use std::marker::PhantomData;

use crate::core::*;

pub trait Tea {
    type Model;
    type Msg: Send + 'static;
    type View<'a>
    where
        Self::Model: 'a;

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
    fn view<'a>(&self, model: &'a mut Self::Model) -> Self::View<'a>;

    fn subscriptions(&self, _model: &Self::Model) -> Sub<Self::Msg> {
        Sub::none()
    }
}

pub trait InitFn<Model, Msg: Send + 'static> {
    fn init(&self) -> (Model, Cmd<Msg>);
}

impl<F, I, Model, Msg> InitFn<Model, Msg> for F
where
    F: Fn() -> I,
    Msg: Send + 'static,
    I: IntoInit<Model, Msg>,
{
    fn init(&self) -> (Model, Cmd<Msg>) {
        self().into_init()
    }
}

pub trait IntoInit<Model, Msg: Send + 'static> {
    /// Turns some type into the initial state of some [`Application`].
    fn into_init(self) -> (Model, Cmd<Msg>);
}

impl<Model, Msg: Send + 'static> IntoInit<Model, Msg> for (Model, Cmd<Msg>) {
    fn into_init(self) -> (Model, Cmd<Msg>) {
        self
    }
}

impl<Model, Msg: Send + 'static> IntoInit<Model, Msg> for Model {
    fn into_init(self) -> (Model, Cmd<Msg>) {
        (self, Cmd::none())
    }
}

pub trait UpdateFn<Model, Msg: Send + 'static> {
    fn update(&self, model: &mut Model, msg: Msg) -> Cmd<Msg>;
}

impl<F, Model, Msg> UpdateFn<Model, Msg> for F
where
    F: Fn(&mut Model, Msg) -> Cmd<Msg>,
    Msg: Send + 'static,
{
    fn update(&self, model: &mut Model, msg: Msg) -> Cmd<Msg> {
        self(model, msg)
    }
}

pub trait ViewFn<'a, Model, Msg: Send + 'static> {
    type View;

    fn view(&self, model: &'a mut Model) -> Self::View;
}

impl<'a, F, Model, View, Msg> ViewFn<'a, Model, Msg> for F
where
    Model: 'a,
    F: Fn(&'a mut Model) -> View,
    Msg: Send + 'static,
{
    type View = View;

    fn view(&self, model: &'a mut Model) -> Self::View {
        self(model)
    }
}

pub trait SubFn<Model, Msg: Send + 'static> {
    fn subscriptions(&self, model: &Model) -> Sub<Msg>;
}

impl<F, Model, Msg> SubFn<Model, Msg> for F
where
    F: for<'a> Fn(&'a Model) -> Sub<Msg>,
    Msg: Send + 'static,
{
    fn subscriptions(&self, model: &Model) -> Sub<Msg> {
        self(model)
    }
}

impl<Model, Msg: Send + 'static> SubFn<Model, Msg> for () {
    fn subscriptions(&self, _model: &Model) -> Sub<Msg> {
        Sub::none()
    }
}

pub struct Application<Model, Msg: Send + 'static, I, U, V, S = ()>
where
    I: InitFn<Model, Msg>,
    U: UpdateFn<Model, Msg>,
    for<'a> V: ViewFn<'a, Model, Msg>,
    S: SubFn<Model, Msg>,
{
    pub(crate) init: I,
    pub(crate) update: U,
    pub(crate) view: V,
    pub(crate) subscriptions: S,
    _model: PhantomData<Model>,
    _msg: PhantomData<Msg>,
}

impl<Model, Msg: Send + 'static, I, U, V, S> Tea for Application<Model, Msg, I, U, V, S>
where
    I: InitFn<Model, Msg>,
    U: UpdateFn<Model, Msg>,
    for<'a> V: ViewFn<'a, Model, Msg>,
    S: SubFn<Model, Msg>,
{
    type Model = Model;
    type Msg = Msg;
    type View<'a>
        = <V as ViewFn<'a, Model, Msg>>::View
    where
        Model: 'a;

    fn init(&self) -> (Self::Model, Cmd<Self::Msg>) {
        self.init.init()
    }

    fn update(&self, model: &mut Self::Model, msg: Self::Msg) -> Cmd<Self::Msg> {
        self.update.update(model, msg)
    }

    fn view<'a>(&self, model: &'a mut Self::Model) -> Self::View<'a> {
        self.view.view(model)
    }

    fn subscriptions(&self, model: &Self::Model) -> Sub<Self::Msg> {
        self.subscriptions.subscriptions(model)
    }
}

impl<Model, Msg: Send + 'static, I, U, V> Application<Model, Msg, I, U, V, ()>
where
    I: InitFn<Model, Msg>,
    U: UpdateFn<Model, Msg>,
    for<'a> V: ViewFn<'a, Model, Msg>,
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

impl<Model, Msg: Send + 'static, I, U, V, S> Application<Model, Msg, I, U, V, S>
where
    I: InitFn<Model, Msg>,
    U: UpdateFn<Model, Msg>,
    for<'a> V: ViewFn<'a, Model, Msg>,
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
