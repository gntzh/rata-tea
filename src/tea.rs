use ratatui::Frame;

use std::marker::PhantomData;

use crate::core::*;

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
    pub fn subscriptions<S2: Fn(&Model) -> Sub<Msg>>(
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
