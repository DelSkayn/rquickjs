use std::ops::{Deref, DerefMut};

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

/*
/// The wrapper for function results
#[repr(transparent)]
pub struct ResultJs<T, E>(pub StdResult<T, E>);

impl<T, E> From<StdResult<T, E>> for ResultJs<T, E> {
    fn from(result: StdResult<T, E>) -> Self {
        Self(result)
    }
}

impl<T, E> AsRef<StdResult<T, E>> for ResultJs<T, E> {
    fn as_ref(&self) -> &StdResult<T, E> {
        &self.0
    }
}

impl<T, E> Deref for ResultJs<T, E> {
    type Target = StdResult<T, E>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
*/

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
