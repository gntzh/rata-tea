use dashmap::DashMap;
use futures::{Stream, StreamExt, stream::BoxStream};
use ratatui::Frame;
use std::{any::TypeId, future, hash::Hash, marker::PhantomData, pin::Pin};
use tokio::sync::mpsc::{self, UnboundedReceiver};

pub use crossterm::event::Event as TerminalEvent;

pub mod runner;
pub use runner::Runner;
pub type Hasher = std::hash::DefaultHasher;

pub trait InitFn<Model, Msg: OwnedSend> {
    fn init(&self) -> (Model, Cmd<Msg>);
}

impl<F, I, Model, Msg> InitFn<Model, Msg> for F
where
    F: Fn() -> I,
    Msg: OwnedSend,
    I: IntoInit<Model, Msg>,
{
    fn init(&self) -> (Model, Cmd<Msg>) {
        self().into_init()
    }
}

pub trait IntoInit<Model, Msg: OwnedSend> {
    /// Turns some type into the initial state of some [`Application`].
    fn into_init(self) -> (Model, Cmd<Msg>);
}

impl<Model, Msg: OwnedSend> IntoInit<Model, Msg> for (Model, Cmd<Msg>) {
    fn into_init(self) -> (Model, Cmd<Msg>) {
        self
    }
}

impl<Model, Msg: OwnedSend> IntoInit<Model, Msg> for Model {
    fn into_init(self) -> (Model, Cmd<Msg>) {
        (self, Cmd::none())
    }
}

pub trait UpdateFn<Model, Msg: OwnedSend> {
    fn update(&self, model: &mut Model, msg: Msg) -> Cmd<Msg>;
}

impl<F, Model, Msg> UpdateFn<Model, Msg> for F
where
    F: Fn(&mut Model, Msg) -> Cmd<Msg>,
    Msg: OwnedSend,
{
    fn update(&self, model: &mut Model, msg: Msg) -> Cmd<Msg> {
        self(model, msg)
    }
}

pub trait ViewFn<Model, Msg: OwnedSend> {
    fn view(&self, model: &mut Model, frame: &mut Frame);
}

impl<F, Model, Msg> ViewFn<Model, Msg> for F
where
    F: Fn(&mut Model, &mut Frame),
    Msg: OwnedSend,
{
    fn view(&self, model: &mut Model, frame: &mut Frame) {
        self(model, frame)
    }
}

pub trait SubFn<Model, Msg: OwnedSend> {
    fn subscriptions(&self, model: &Model) -> Sub<Msg>;
}

impl<F, Model, Msg> SubFn<Model, Msg> for F
where
    F: for<'a> Fn(&'a Model) -> Sub<Msg>,
    Msg: OwnedSend,
{
    fn subscriptions(&self, model: &Model) -> Sub<Msg> {
        self(model)
    }
}

impl<Model, Msg: OwnedSend> SubFn<Model, Msg> for () {
    fn subscriptions(&self, _model: &Model) -> Sub<Msg> {
        Sub::none()
    }
}

pub trait Tea {
    type Model;
    type Msg: OwnedSend;

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

pub struct Application<Model, Msg: OwnedSend, I, U, V, S = ()>
where
    I: InitFn<Model, Msg>,
    U: UpdateFn<Model, Msg>,
    V: ViewFn<Model, Msg>,
    S: SubFn<Model, Msg>,
{
    pub(crate) init: I,
    pub(crate) update: U,
    pub(crate) view: V,
    pub(crate) subscriptions: S,
    _model: PhantomData<Model>,
    _msg: PhantomData<Msg>,
}

impl<Model, Msg: OwnedSend, I, U, V, S> Tea for Application<Model, Msg, I, U, V, S>
where
    I: InitFn<Model, Msg>,
    U: UpdateFn<Model, Msg>,
    V: ViewFn<Model, Msg>,
    S: SubFn<Model, Msg>,
{
    type Model = Model;

    type Msg = Msg;

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

impl<Model, Msg: OwnedSend, I, U, V> Application<Model, Msg, I, U, V, ()>
where
    I: InitFn<Model, Msg>,
    U: UpdateFn<Model, Msg>,
    V: ViewFn<Model, Msg>,
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

impl<Model, Msg: OwnedSend, I, U, V, S> Application<Model, Msg, I, U, V, S>
where
    I: InitFn<Model, Msg>,
    U: UpdateFn<Model, Msg>,
    V: ViewFn<Model, Msg>,
    S: SubFn<Model, Msg>,
{
    pub fn with_subscriptions<S2: Fn(&Model) -> Sub<Msg>>(
        self,
        subscriptions: S2,
    ) -> Application<Model, Msg, I, U, V, S2> {
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

pub trait OwnedSend: Send + 'static {}
impl<T: Send + 'static> OwnedSend for T {}

pub enum Action<Msg> {
    Msg(Msg),
    SendEvent(Box<dyn FnOnce(&EventBus) + Send>),
    Quit,
}

pub struct Cmd<Msg: OwnedSend>(Vec<Command<Action<Msg>>>);
pub type Command<Msg> = Pin<Box<dyn Future<Output = Msg> + Send + 'static>>;

impl<Msg: OwnedSend> Cmd<Msg> {
    pub fn none() -> Self {
        Self(Vec::new())
    }

    pub fn batch(cmds: impl IntoIterator<Item = Self>) -> Self {
        Self(cmds.into_iter().flat_map(|factories| factories.0).collect())
    }
}

impl<Msg: OwnedSend> Cmd<Msg> {
    pub fn size_hint(&self) -> usize {
        self.0.len()
    }

    pub fn effect(action: Action<Msg>) -> Self {
        Self(vec![Box::pin(future::ready(action))])
    }

    pub fn quit() -> Self {
        Self::effect(Action::Quit)
    }
}

pub struct Sub<Msg: OwnedSend>(Vec<Box<dyn SubFactory<Msg>>>);
pub type MsgStream<M> = BoxStream<'static, M>;
pub type EventStream = BoxStream<'static, Event>;

#[derive(Debug, Clone, strum::EnumDiscriminants, strum::EnumIs)]
#[strum_discriminants(derive(Hash))]
#[strum_discriminants(name(EventType))]
pub enum Event {
    Tick(Tick),
    Terminal(TerminalEvent),
}

#[derive(Debug, Clone)]
pub struct Tick;

/// Factory invoked at spawn-time by the runner. Consumed once.
pub trait SubFactory<Msg: OwnedSend>: 'static {
    fn hash(&self, state: &mut Hasher);
    fn stream(self: Box<Self>, event_bus: EventBusSub<'_>) -> MsgStream<Msg>;
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
            fn hash(&self, state: &mut Hasher) {
                TypeId::of::<F>().hash(state);
                self.inner.hash(state);
            }

            fn stream(self: Box<Self>, event_bus: EventBusSub<'_>) -> MsgStream<Msg2> {
                let mapper = self.mapper;
                tokio_stream::StreamExt::map(self.inner.stream(event_bus), mapper).boxed()
            }
        }
        let factories = self
            .0
            .into_iter()
            .map(|raw| {
                Box::new(Map {
                    inner: raw,
                    mapper: mapper.clone(),
                }) as Box<dyn SubFactory<Msg2>>
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
            raw: Box<dyn SubFactory<Msg>>,
            mapper: F,
        }

        impl<Msg, Msg2, F> SubFactory<Msg2> for FilterMap<Msg, Msg2, F>
        where
            Msg: OwnedSend,
            Msg2: OwnedSend,
            F: Fn(Msg) -> Option<Msg2> + Send + 'static,
        {
            fn hash(&self, state: &mut Hasher) {
                TypeId::of::<F>().hash(state);
                self.raw.hash(state);
            }

            fn stream(self: Box<Self>, event_bus: EventBusSub<'_>) -> MsgStream<Msg2> {
                let mapper = self.mapper;
                tokio_stream::StreamExt::filter_map(self.raw.stream(event_bus), mapper).boxed()
            }
        }
        let factories = self
            .0
            .into_iter()
            .map(|raw| {
                Box::new(FilterMap {
                    raw,
                    mapper: mapper.clone(),
                }) as Box<dyn SubFactory<Msg2>>
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
    pub fn make<I, F, S>(input: I, stream_maker: F) -> Self
    where
        I: Hash + 'static,
        F: FnOnce(I, EventBusSub<'_>) -> S + 'static,
        S: Stream<Item = Msg> + Send + 'static,
    {
        struct MakeSub<I, F, S, Msg>
        where
            F: FnOnce(I, EventBusSub<'_>) -> S,
            S: Stream<Item = Msg>,
        {
            input: I,
            stream_maker: F,
        }
        impl<I, F, S, Msg> SubFactory<Msg> for MakeSub<I, F, S, Msg>
        where
            I: Hash + 'static,
            F: FnOnce(I, EventBusSub<'_>) -> S + 'static,
            S: Stream<Item = Msg> + Send + 'static,
            Msg: OwnedSend,
        {
            fn hash(&self, state: &mut Hasher) {
                TypeId::of::<I>().hash(state);
                self.input.hash(state);
                TypeId::of::<F>().hash(state);
            }

            fn stream(self: Box<Self>, event_bus: EventBusSub<'_>) -> MsgStream<Msg> {
                (self.stream_maker)(self.input, event_bus).boxed()
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

    use crate::{Sub, TerminalEvent};

    pub fn on_term_event() -> Sub<TerminalEvent> {
        Sub::make((), |(), event_bus| {
            tokio_stream::wrappers::UnboundedReceiverStream::new(
                event_bus.subscribe::<TerminalEvent>(),
            )
            .boxed()
        })
    }
    pub fn on_key_event() -> Sub<KeyEvent> {
        Sub::make((), |(), event_bus| {
            tokio_stream::StreamExt::filter_map(
                tokio_stream::wrappers::UnboundedReceiverStream::new(
                    event_bus.subscribe::<TerminalEvent>(),
                ),
                move |event| match event {
                    TerminalEvent::Key(event) => Some(event),
                    _ => None,
                },
            )
            .boxed()
        })
    }
    pub fn on_key_press() -> Sub<KeyEvent> {
        Sub::make((), |(), event_bus| {
            tokio_stream::StreamExt::filter_map(
                tokio_stream::wrappers::UnboundedReceiverStream::new(
                    event_bus.subscribe::<TerminalEvent>(),
                ),
                move |event| match event {
                    TerminalEvent::Key(
                        event @ KeyEvent {
                            kind: KeyEventKind::Press,
                            ..
                        },
                    ) => Some(event),
                    _ => None,
                },
            )
            .boxed()
        })
    }
}

/// Typed EventBus
#[derive(Default)]
pub struct EventBus {
    map: DashMap<TypeId, Box<dyn std::any::Any + Send>>,
}

#[derive(Default)]
struct Bus<E: OwnedSend> {
    listeners: Vec<mpsc::UnboundedSender<E>>,
}

impl EventBus {
    pub fn new() -> Self {
        Default::default()
    }

    /// Interior mutability
    pub fn subscribe<E: OwnedSend>(&self) -> UnboundedReceiver<E> {
        let (tx, rx) = mpsc::unbounded_channel();
        self.map
            .entry(TypeId::of::<E>())
            .or_insert_with(|| Box::new(Bus::<E> { listeners: vec![] }))
            .downcast_mut::<Bus<E>>()
            .expect("got by TypeId")
            .listeners
            .push(tx);
        rx
    }

    pub fn publish<E: OwnedSend + Clone>(&self, event: E) {
        if let Some(bus) = self
            .map
            .get(&TypeId::of::<E>())
            .as_ref()
            .and_then(|entry| entry.downcast_ref::<Bus<E>>())
        {
            for tx in &bus.listeners {
                let _ = tx.send(event.clone());
            }
        }
    }

    pub fn sub_cap(&self) -> EventBusSub<'_> {
        EventBusSub(self)
    }
    pub fn pub_cap(&self) -> EventBusPub<'_> {
        EventBusPub(self)
    }
}

/// Capability to subscribe only
pub struct EventBusSub<'a>(&'a EventBus);
impl<'a> EventBusSub<'a> {
    pub fn subscribe<E: OwnedSend>(&self) -> UnboundedReceiver<E> {
        self.0.subscribe()
    }
}

/// Capability to publish only
pub struct EventBusPub<'a>(&'a EventBus);
impl<'a> EventBusPub<'a> {
    pub fn publish<E: OwnedSend + Clone>(&self, event: E) {
        self.0.publish(event)
    }
}
