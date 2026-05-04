use crossterm::event::Event as TerminalEvent;
use futures::{Stream, StreamExt, stream::BoxStream};
use ratatui::Frame;
use std::{any::TypeId, future, hash::Hash, marker::PhantomData, pin::Pin};
pub mod runner;
pub use runner::Runner;

pub type Hasher = std::hash::DefaultHasher;

pub trait InitFn<Mo, M: Msg> {
    fn init(&self) -> (Mo, Cmd<M>);
}

impl<F, I, Mo, M> InitFn<Mo, M> for F
where
    F: Fn() -> I,
    M: Msg,
    I: IntoInit<Mo, M>,
{
    fn init(&self) -> (Mo, Cmd<M>) {
        self().into_init()
    }
}

pub trait IntoInit<Mo, M: Msg> {
    /// Turns some type into the initial state of some [`Application`].
    fn into_init(self) -> (Mo, Cmd<M>);
}

impl<Mo, M: Msg> IntoInit<Mo, M> for (Mo, Cmd<M>) {
    fn into_init(self) -> (Mo, Cmd<M>) {
        self
    }
}

impl<Mo, M: Msg> IntoInit<Mo, M> for Mo {
    fn into_init(self) -> (Mo, Cmd<M>) {
        (self, Cmd::none())
    }
}

pub trait UpdateFn<Mo, M: Msg> {
    fn update(&self, model: &mut Mo, msg: M) -> Cmd<M>;
}

impl<F, Mo, M> UpdateFn<Mo, M> for F
where
    F: Fn(&mut Mo, M) -> Cmd<M>,
    M: Msg,
{
    fn update(&self, model: &mut Mo, msg: M) -> Cmd<M> {
        self(model, msg)
    }
}

pub trait ViewFn<Mo, M: Msg> {
    fn view(&self, model: &mut Mo, frame: &mut Frame);
}

impl<F, Mo, M> ViewFn<Mo, M> for F
where
    F: Fn(&mut Mo, &mut Frame),
    M: Msg,
{
    fn view(&self, model: &mut Mo, frame: &mut Frame) {
        self(model, frame)
    }
}

pub trait SubFn<Mo, M: Msg> {
    fn subscriptions(&self, model: &Mo) -> Sub<M>;
}

impl<F, Mo, M> SubFn<Mo, M> for F
where
    F: for<'a> Fn(&'a Mo) -> Sub<M>,
    M: Msg,
{
    fn subscriptions(&self, model: &Mo) -> Sub<M> {
        self(model)
    }
}

impl<Mo, M: Msg> SubFn<Mo, M> for () {
    fn subscriptions(&self, _model: &Mo) -> Sub<M> {
        Sub::none()
    }
}

pub trait Tea {
    type Model;
    type Msg: Msg;

    fn init(&self) -> (Self::Model, Cmd<Self::Msg>);

    /// Match each possible message and decide how the model should change
    ///
    /// Modify existing model reflecting those changes
    ///
    /// can also return another Msg
    fn update(&self, model: &mut Self::Model, msg: Self::Msg) -> Cmd<Self::Msg>;

    /// render model to the terminal.
    ///
    /// In ratatui, there are [`ratatui::widgets::StatefulWidget`]s which require a mutable reference to state during render.
    fn view(&self, model: &mut Self::Model, frame: &mut Frame);

    fn subscriptions(&self, _model: &Self::Model) -> Sub<Self::Msg> {
        Sub::none()
    }
}

pub struct Application<Mo, M: Msg, I, U, V, S = ()>
where
    I: InitFn<Mo, M>,
    U: UpdateFn<Mo, M>,
    V: ViewFn<Mo, M>,
    S: SubFn<Mo, M>,
{
    pub(crate) init: I,
    pub(crate) update: U,
    pub(crate) view: V,
    pub(crate) subscriptions: S,
    _model: PhantomData<Mo>,
    _msg: PhantomData<M>,
}

impl<Mo, M: Msg, I, U, V, S> Tea for Application<Mo, M, I, U, V, S>
where
    I: InitFn<Mo, M>,
    U: UpdateFn<Mo, M>,
    V: ViewFn<Mo, M>,
    S: SubFn<Mo, M>,
{
    type Model = Mo;

    type Msg = M;

    fn init(&self) -> (Self::Model, Cmd<Self::Msg>) {
        self.init.init()
    }

    fn update(&self, model: &mut Self::Model, msg: Self::Msg) -> Cmd<Self::Msg> {
        self.update.update(model, msg)
    }

    fn view(&self, model: &mut Self::Model, frame: &mut Frame) {
        self.view.view(model, frame);
    }

    fn subscriptions(&self, model: &Self::Model) -> Sub<Self::Msg> {
        self.subscriptions.subscriptions(model)
    }
}

impl<Mo, M: Msg, I, U, V> Application<Mo, M, I, U, V, ()>
where
    I: InitFn<Mo, M>,
    U: UpdateFn<Mo, M>,
    V: ViewFn<Mo, M>,
{
    pub fn new(init: I, update: U, view: V) -> Self {
        Self {
            init,
            update,
            view,
            subscriptions: (),
            _model: PhantomData,
            _msg: PhantomData,
        }
    }
}

impl<Mo, M: Msg, I, U, V, S> Application<Mo, M, I, U, V, S>
where
    I: InitFn<Mo, M>,
    U: UpdateFn<Mo, M>,
    V: ViewFn<Mo, M>,
    S: SubFn<Mo, M>,
{
    pub fn with_subscriptions<S2: Fn(&Mo) -> Sub<M>>(
        self,
        subscriptions: S2,
    ) -> Application<Mo, M, I, U, V, S2> {
        Application {
            init: self.init,
            update: self.update,
            view: self.view,
            subscriptions,
            _model: PhantomData,
            _msg: PhantomData,
        }
    }
}

pub trait Msg: Send + 'static {}
impl<T: Send + 'static> Msg for T {}

pub type Command<M> = Pin<Box<dyn Future<Output = M> + Send + 'static>>;

pub enum Action<M> {
    Msg(M),
    Quit,
}

pub struct Cmd<M: Msg>(Vec<Command<Action<M>>>);

impl<M: Msg> Cmd<M> {
    pub fn none() -> Self {
        Self(Vec::new())
    }

    pub fn batch(cmds: Vec<Command<Action<M>>>) -> Self {
        Self(cmds)
    }

    pub fn quit() -> Self {
        Self(vec![Box::pin(future::ready(Action::Quit))])
    }
}

pub struct Sub<M: Msg>(Vec<Box<dyn SubFactory<M>>>);
pub type MsgStream<M> = BoxStream<'static, M>;
pub type EventStream = BoxStream<'static, Event>;

#[derive(Debug, Clone, strum::EnumDiscriminants)]
#[strum_discriminants(derive(Hash))]
#[strum_discriminants(name(EventType))]
pub enum Event {
    Frame,
    Terminal(TerminalEvent),
}

pub trait SubFactory<M: Msg>: 'static {
    fn hash(&self, state: &mut Hasher);
    fn stream(self: Box<Self>, event_stream: EventStream) -> MsgStream<M>;
}

impl<M: Msg> Sub<M> {
    pub fn none() -> Self {
        Self(Vec::new())
    }

    pub fn batch(subs: impl IntoIterator<Item = Self>) -> Self {
        Self(subs.into_iter().flat_map(|factories| factories.0).collect())
    }

    pub fn map<F, M2>(self, mapper: F) -> Sub<M2>
    where
        F: Fn(M) -> M2 + Send + Clone + 'static,
        M2: Msg,
    {
        struct Map<M, M2, F: Fn(M) -> M2> {
            inner: Box<dyn SubFactory<M>>,
            mapper: F,
        }

        impl<M, M2, F> SubFactory<M2> for Map<M, M2, F>
        where
            M: Msg,
            M2: Msg,
            F: Fn(M) -> M2 + Send + 'static,
        {
            fn hash(&self, state: &mut Hasher) {
                TypeId::of::<F>().hash(state);
                self.inner.hash(state);
            }

            fn stream(self: Box<Self>, event_stream: EventStream) -> MsgStream<M2> {
                let mapper = self.mapper;
                tokio_stream::StreamExt::map(self.inner.stream(event_stream), mapper).boxed()
            }
        }
        let factories = self
            .0
            .into_iter()
            .map(|raw| {
                Box::new(Map {
                    inner: raw,
                    mapper: mapper.clone(),
                }) as Box<dyn SubFactory<M2>>
            })
            .collect();
        Sub(factories)
    }

    pub fn filter_map<F, M2>(self, mapper: F) -> Sub<M2>
    where
        F: Fn(M) -> Option<M2> + Send + Clone + 'static,
        M2: Msg,
    {
        struct FilterMap<M, M2, F>
        where
            F: Fn(M) -> Option<M2> + Send,
        {
            raw: Box<dyn SubFactory<M>>,
            mapper: F,
        }

        impl<M, M2, F> SubFactory<M2> for FilterMap<M, M2, F>
        where
            M: Msg,
            M2: Msg,
            F: Fn(M) -> Option<M2> + Send + 'static,
        {
            fn hash(&self, state: &mut Hasher) {
                TypeId::of::<F>().hash(state);
                self.raw.hash(state);
            }

            fn stream(self: Box<Self>, event_rx: EventStream) -> MsgStream<M2> {
                let mapper = self.mapper;
                tokio_stream::StreamExt::filter_map(self.raw.stream(event_rx), mapper).boxed()
            }
        }
        let factories = self
            .0
            .into_iter()
            .map(|raw| {
                Box::new(FilterMap {
                    raw,
                    mapper: mapper.clone(),
                }) as Box<dyn SubFactory<M2>>
            })
            .collect();
        Sub(factories)
    }
}

impl<M: Msg> Sub<M> {
    /// `input` and `TypeId::of<F>` will be used as the sub identifier
    pub fn make<I, F, S>(input: I, stream_maker: F) -> Self
    where
        I: Hash + 'static,
        F: FnOnce(I, EventStream) -> S + 'static,
        S: Stream<Item = M> + Send + 'static,
    {
        struct MakeSub<I, F, S, M>
        where
            F: FnOnce(I, EventStream) -> S,
            S: Stream<Item = M>,
        {
            input: I,
            stream_maker: F,
        }
        impl<I, F, S, M> SubFactory<M> for MakeSub<I, F, S, M>
        where
            I: Hash + 'static,
            F: FnOnce(I, EventStream) -> S + 'static,
            S: Stream<Item = M> + Send + 'static,
            M: Msg,
        {
            fn hash(&self, state: &mut Hasher) {
                TypeId::of::<I>().hash(state);
                self.input.hash(state);
                TypeId::of::<F>().hash(state);
            }

            fn stream(self: Box<Self>, event_stream: EventStream) -> MsgStream<M> {
                (self.stream_maker)(self.input, event_stream).boxed()
            }
        }

        let sub = MakeSub {
            input,
            stream_maker: |input, event_stream| stream_maker(input, event_stream),
        };
        Self(vec![Box::new(sub)])
    }
}

pub mod time {
    use futures::StreamExt;
    use std::time::{Duration, Instant};

    use crate::Sub;
    pub fn every(interval: Duration) -> Sub<Instant> {
        #[derive(Hash)]
        struct Every {
            interval: Duration,
        }
        Sub::make(Every { interval }, move |Every { interval }, _| {
            tokio_stream::wrappers::IntervalStream::new(tokio::time::interval(interval))
                .map(tokio::time::Instant::into_std)
                .boxed()
        })
    }
}

pub mod term {
    use crossterm::event::{KeyEvent, KeyEventKind};
    use futures::StreamExt as _;

    use crate::{Event, Sub, TerminalEvent};

    pub fn on_term_event() -> Sub<TerminalEvent> {
        Sub::make((), |(), event_stream| {
            tokio_stream::StreamExt::filter_map(event_stream, move |event| match event {
                Event::Terminal(event) => Some(event),
                _ => None,
            })
            .boxed()
        })
    }
    pub fn on_key_event() -> Sub<KeyEvent> {
        Sub::make((), |(), event_stream| {
            tokio_stream::StreamExt::filter_map(event_stream, move |event| match event {
                Event::Terminal(TerminalEvent::Key(event)) => Some(event),
                _ => None,
            })
            .boxed()
        })
    }
    pub fn on_key_press() -> Sub<KeyEvent> {
        Sub::make((), |(), event_stream| {
            tokio_stream::StreamExt::filter_map(event_stream, move |event| match event {
                Event::Terminal(TerminalEvent::Key(
                    event @ KeyEvent {
                        kind: KeyEventKind::Press,
                        ..
                    },
                )) => Some(event),
                _ => None,
            })
            .boxed()
        })
    }
}
