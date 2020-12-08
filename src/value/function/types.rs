use crate::{AsFunction, AsFunctionMut, Ctx, Function, IntoJs, Result, SendWhenParallel, Value};
use std::{
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

/// The wrapper for method functions
#[repr(transparent)]
pub struct Method<F>(pub F);

impl<F> AsRef<F> for Method<F> {
    fn as_ref(&self) -> &F {
        &self.0
    }
}

impl<F> Deref for Method<F> {
    type Target = F;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// The wrapper to specify `this` argument
#[derive(Clone, Copy, Debug, Default)]
#[repr(transparent)]
pub struct This<T>(pub T);

impl<T> This<T> {
    pub fn into(self) -> T {
        self.0
    }
}

impl<T> From<T> for This<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<T> AsRef<T> for This<T> {
    fn as_ref(&self) -> &T {
        &self.0
    }
}

impl<T> AsMut<T> for This<T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T> Deref for This<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for This<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// The wrapper for variable number of arguments
#[derive(Clone, Default)]
pub struct Args<T>(pub Vec<T>);

impl<T> Args<T> {
    pub fn new() -> Self {
        Self(Vec::new())
    }
}

impl<T> From<Vec<T>> for Args<T> {
    fn from(vec: Vec<T>) -> Self {
        Self(vec)
    }
}

impl<T> Into<Vec<T>> for Args<T> {
    fn into(self) -> Vec<T> {
        self.0
    }
}

impl<T> AsRef<Vec<T>> for Args<T> {
    fn as_ref(&self) -> &Vec<T> {
        &self.0
    }
}

impl<T> AsMut<Vec<T>> for Args<T> {
    fn as_mut(&mut self) -> &mut Vec<T> {
        &mut self.0
    }
}

impl<T> Deref for Args<T> {
    type Target = Vec<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for Args<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// The wrapper for functions which implements `Fn` trait
///
/// ```
/// # use rquickjs::JsFn;
/// let my_func = JsFn::new("my_func", || 42);
/// let print = JsFn::new_unnamed(|m: String| println!("{}", m));
/// ```
pub struct JsFn<F>(pub F);

impl<'js, F, A, R> JsFn<(F, PhantomData<(A, R)>)>
where
    F: AsFunction<'js, A, R> + SendWhenParallel + 'static,
{
    /// Wrap anonymous function
    pub fn new_unnamed(func: F) -> Self {
        Self((func, PhantomData))
    }
}

impl<'js, S, F, A, R> JsFn<(S, F, PhantomData<(A, R)>)>
where
    S: AsRef<str>,
    F: AsFunction<'js, A, R> + SendWhenParallel + 'static,
{
    /// Wrap named function
    pub fn new(name: S, func: F) -> Self {
        Self((name, func, PhantomData))
    }
}

impl<'js, F, A, R> IntoJs<'js> for JsFn<(F, PhantomData<(A, R)>)>
where
    F: AsFunction<'js, A, R> + SendWhenParallel + 'static,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        let (func, _) = self.0;
        Function::new(ctx, "", func).map(Value::from)
    }
}

impl<'js, S, F, A, R> IntoJs<'js> for JsFn<(S, F, PhantomData<(A, R)>)>
where
    S: AsRef<str>,
    F: AsFunction<'js, A, R> + SendWhenParallel + 'static,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        let (name, func, _) = self.0;
        Function::new(ctx, name, func).map(Value::from)
    }
}

/// The wrapper for functions which implements `Fn` trait
///
/// ```
/// # use rquickjs::JsFn;
/// let my_func = JsFn::new("my_func", || 42);
/// let print = JsFn::new_unnamed(|m: String| println!("{}", m));
/// ```
pub struct JsFnMut<F>(pub F);

impl<'js, F, A, R> JsFnMut<(F, PhantomData<(A, R)>)>
where
    F: AsFunctionMut<'js, A, R> + SendWhenParallel + 'static,
{
    /// Wrap anonymous function
    pub fn new_unnamed(func: F) -> Self {
        Self((func, PhantomData))
    }
}

impl<'js, S, F, A, R> JsFnMut<(S, F, PhantomData<(A, R)>)>
where
    S: AsRef<str>,
    F: AsFunctionMut<'js, A, R> + SendWhenParallel + 'static,
{
    /// Wrap named function
    pub fn new(name: S, func: F) -> Self {
        Self((name, func, PhantomData))
    }
}

impl<'js, F, A, R> IntoJs<'js> for JsFnMut<(F, PhantomData<(A, R)>)>
where
    F: AsFunctionMut<'js, A, R> + SendWhenParallel + 'static,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        let (func, _) = self.0;
        Function::new_mut(ctx, "", func).map(Value::from)
    }
}

impl<'js, S, F, A, R> IntoJs<'js> for JsFnMut<(S, F, PhantomData<(A, R)>)>
where
    S: AsRef<str>,
    F: AsFunctionMut<'js, A, R> + SendWhenParallel + 'static,
{
    fn into_js(self, ctx: Ctx<'js>) -> Result<Value<'js>> {
        let (name, func, _) = self.0;
        Function::new_mut(ctx, name, func).map(Value::from)
    }
}
