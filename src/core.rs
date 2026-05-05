use std::{any::TypeId, hash::Hasher as _, marker::PhantomData, pin::Pin};

use futures::FutureExt as _;
pub type Hasher = std::hash::DefaultHasher;

pub trait OwnedSend: Send + 'static {}
impl<T: Send + 'static> OwnedSend for T {}

pub trait Dispatch<Msg>: Fn(Msg) + Send + 'static {}
impl<F, Msg> Dispatch<Msg> for F where F: Fn(Msg) + Send + 'static {}

pub type AbortHandle = futures::future::AbortHandle;

pub struct Cmd<Msg: OwnedSend>(pub Vec<Command<Msg>>);
pub type Command<Msg> = Pin<Box<dyn Future<Output = Msg> + Send + 'static>>;

impl<Msg: OwnedSend> Cmd<Msg> {
    pub fn none() -> Self {
        Self(Vec::new())
    }

    pub fn batch(cmds: impl IntoIterator<Item = Self>) -> Self {
        Self(cmds.into_iter().flat_map(|factories| factories.0).collect())
    }

    pub fn map<F, Msg2>(self, mapper: F) -> Cmd<Msg2>
    where
        F: FnMut(Msg) -> Msg2 + Send + Clone + 'static,
        Msg2: OwnedSend,
    {
        let commands = self
            .0
            .into_iter()
            .map(move |fut| {
                let mut mapper = mapper.clone();

                let fut = async move {
                    let msg = fut.await;
                    mapper(msg)
                };
                fut.boxed()
            })
            .collect::<Vec<_>>();
        Cmd(commands)
    }
}

impl<Msg: OwnedSend> Cmd<Msg> {
    pub fn size_hint(&self) -> usize {
        self.0.len()
    }

    pub fn future(fut: impl Future<Output = Msg> + Send + 'static) -> Self {
        Self(vec![Box::pin(fut)])
    }
}

pub struct Sub<Msg: OwnedSend>(pub Vec<(SubId, Box<dyn SubFactory<Msg>>)>);
#[derive(Debug, Hash, PartialEq, Eq)]
pub enum SubId {
    Str(&'static str),
    String(String),
    TypeId(TypeId),
    Hash(u64),
    Batch(Vec<SubId>),
}

impl From<&'static str> for SubId {
    fn from(value: &'static str) -> Self {
        Self::Str(value)
    }
}

impl From<String> for SubId {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<TypeId> for SubId {
    fn from(value: TypeId) -> Self {
        Self::TypeId(value)
    }
}

impl From<u64> for SubId {
    fn from(value: u64) -> Self {
        Self::Hash(value)
    }
}

use std::hash::Hash;

impl SubId {
    fn with(self, id: impl Into<SubId>) -> Self {
        let id = id.into();
        match (self, id) {
            (
                this @ (SubId::Str(_) | SubId::String(_) | SubId::TypeId(_) | SubId::Hash(_)),
                new @ (SubId::Str(_) | SubId::String(_) | SubId::TypeId(_) | SubId::Hash(_)),
            ) => Self::Batch(vec![this, new]),
            (
                SubId::Batch(mut sub_ids),
                new @ (SubId::Str(_) | SubId::String(_) | SubId::TypeId(_) | SubId::Hash(_)),
            ) => {
                sub_ids.push(new);
                Self::Batch(sub_ids)
            }
            (
                this @ (SubId::Str(_) | SubId::String(_) | SubId::TypeId(_) | SubId::Hash(_)),
                SubId::Batch(new),
            ) => {
                let mut sub_ids = vec![this];
                sub_ids.extend(new);
                Self::Batch(sub_ids)
            }
            (SubId::Batch(mut sub_ids), SubId::Batch(new)) => {
                sub_ids.extend(new);
                Self::Batch(sub_ids)
            }
        }
    }

    fn with_hash<T: Hash + 'static>(self, value: &T) -> Self {
        let mut state = Hasher::new();
        value.hash(&mut state);
        let hash = state.finish();
        self.with(TypeId::of::<T>()).with(hash)
    }
}

pub trait SubFactory<Msg: OwnedSend> {
    fn create(self: Box<Self>, dispatch: Box<dyn Dispatch<Msg>>) -> AbortHandle;
}

impl<Msg: OwnedSend> Sub<Msg> {
    pub fn none() -> Self {
        Self(Vec::new())
    }

    pub fn batch(subs: impl IntoIterator<Item = Self>) -> Self {
        Self(subs.into_iter().flat_map(|factories| factories.0).collect())
    }

    pub fn map<F, Msg2>(self, mapper: F) -> Sub<Msg2>
    where
        F: Fn(Msg) -> Msg2 + Send + Clone + 'static,
        Msg2: OwnedSend,
    {
        struct Map<M, M2, F: Fn(M) -> M2> {
            inner: Box<dyn SubFactory<M>>,
            mapper: F,
        }

        impl<Msg, Msg2, F> SubFactory<Msg2> for Map<Msg, Msg2, F>
        where
            Msg: OwnedSend,
            Msg2: OwnedSend,
            F: Fn(Msg) -> Msg2 + Send + 'static,
        {
            fn create(self: Box<Self>, dispatch: Box<dyn Dispatch<Msg2>>) -> AbortHandle {
                let mapper = self.mapper;
                
                self.inner.create(Box::new(move |msg| {
                    let msg2 = mapper(msg);
                    dispatch(msg2)
                }))
            }
        }
        let factories = self
            .0
            .into_iter()
            .map(|raw| {
                (
                    raw.0.with(TypeId::of::<F>()),
                    Box::new(Map {
                        inner: raw.1,
                        mapper: mapper.clone(),
                    }) as Box<dyn SubFactory<Msg2>>,
                )
            })
            .collect();
        Sub(factories)
    }

    pub fn filter_map<F, Msg2>(self, mapper: F) -> Sub<Msg2>
    where
        F: Fn(Msg) -> Option<Msg2> + Send + Clone + 'static,
        Msg2: OwnedSend,
    {
        struct FilterMap<Msg, Msg2, F>
        where
            F: Fn(Msg) -> Option<Msg2> + Send,
        {
            inner: Box<dyn SubFactory<Msg>>,
            mapper: F,
        }

        impl<Msg, Msg2, F> SubFactory<Msg2> for FilterMap<Msg, Msg2, F>
        where
            Msg: OwnedSend,
            Msg2: OwnedSend,
            F: Fn(Msg) -> Option<Msg2> + Send + 'static,
        {
            fn create(self: Box<Self>, dispatch: Box<dyn Dispatch<Msg2>>) -> AbortHandle {
                let mapper = self.mapper;
                
                self.inner.create(Box::new(move |msg| {
                    if let Some(msg2) = mapper(msg) {
                        dispatch(msg2)
                    }
                }))
            }
        }
        let factories = self
            .0
            .into_iter()
            .map(|raw| {
                (
                    raw.0.with(TypeId::of::<F>()),
                    (Box::new(FilterMap {
                        inner: raw.1,
                        mapper: mapper.clone(),
                    }) as Box<dyn SubFactory<Msg2>>),
                )
            })
            .collect();
        Sub(factories)
    }
}

impl<Msg: OwnedSend> Sub<Msg> {
    pub fn size_hint(&self) -> usize {
        self.0.len()
    }
    /// `input` and `TypeId::of<F>` will be used as the sub identifier
    pub fn make<I, F>(input: I, stream_maker: F) -> Self
    where
        I: Hash + 'static,
        F: FnOnce(I, Box<dyn Dispatch<Msg>>) -> AbortHandle + 'static,
    {
        struct MakeSub<I, F, Msg>
        where
            F: FnOnce(I, Box<dyn Dispatch<Msg>>) -> AbortHandle + 'static,
        {
            input: I,
            stream_maker: F,
            _msg: PhantomData<Msg>,
        }
        impl<I, F, Msg> SubFactory<Msg> for MakeSub<I, F, Msg>
        where
            I: Hash + 'static,
            F: FnOnce(I, Box<dyn Dispatch<Msg>>) -> AbortHandle + 'static,
            Msg: OwnedSend,
        {
            fn create(self: Box<Self>, dispatch: Box<dyn Dispatch<Msg>>) -> AbortHandle {
                (self.stream_maker)(self.input, dispatch)
            }
        }

        let id = SubId::from(TypeId::of::<F>()).with_hash(&input);
        let sub = MakeSub {
            input,
            stream_maker: |input, event_stream| stream_maker(input, event_stream),
            _msg: PhantomData,
        };
        Self(vec![(id, Box::new(sub))])
    }
}
