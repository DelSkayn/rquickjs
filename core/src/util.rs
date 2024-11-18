//! Module with some util types.

/// A trait for preventing implementing traits which should not be implemented outside of rquickjs.
pub(crate) trait Sealed {}

#[cfg(feature = "futures")]
pub use self::futures::*;

#[cfg(feature = "futures")]
mod futures {
    use std::{
        future::Future,
        marker::PhantomData,
        mem::ManuallyDrop,
        ops::{Deref, DerefMut},
        pin::Pin,
        task::{Context, Poll},
    };

    /// Future which allows one to bail out of a async context, back to manually calling poll.
    pub struct ManualPoll<F, R> {
        f: F,
        _marker: PhantomData<R>,
    }

    impl<F, R> ManualPoll<F, R>
    where
        F: FnMut(&mut Context) -> Poll<R>,
    {
        pub fn new(f: F) -> Self {
            ManualPoll {
                f,
                _marker: PhantomData,
            }
        }
    }

    impl<F: Unpin, R> Unpin for ManualPoll<F, R> {}

    impl<F, R> Future for ManualPoll<F, R>
    where
        F: FnMut(&mut Context) -> Poll<R>,
    {
        type Output = R;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let p = unsafe { self.get_unchecked_mut() };
            (p.f)(cx)
        }
    }

    #[cfg(feature = "parallel")]
    pub use self::parallel::*;

    #[cfg(feature = "parallel")]
    mod parallel {
        use super::*;

        pub struct AssertSyncFuture<F>(F);

        impl<F> AssertSyncFuture<F> {
            pub unsafe fn assert(f: F) -> Self {
                Self(f)
            }
        }

        unsafe impl<F> Sync for AssertSyncFuture<F> {}
        unsafe impl<F: Send> Send for AssertSyncFuture<F> {}

        impl<F> Future for AssertSyncFuture<F>
        where
            F: Future,
        {
            type Output = F::Output;

            fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                let f = unsafe { self.map_unchecked_mut(|x| &mut x.0) };
                f.poll(cx)
            }
        }

        pub struct AssertSendFuture<F>(F);

        impl<F> AssertSendFuture<F> {
            pub unsafe fn assert(f: F) -> Self {
                Self(f)
            }
        }

        unsafe impl<F> Send for AssertSendFuture<F> {}
        unsafe impl<F: Sync> Sync for AssertSendFuture<F> {}

        impl<F> Future for AssertSendFuture<F>
        where
            F: Future,
        {
            type Output = F::Output;

            fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                let f = unsafe { self.map_unchecked_mut(|x| &mut x.0) };
                f.poll(cx)
            }
        }
    }

    pub struct Defer<T, F: FnOnce(T)> {
        value: ManuallyDrop<T>,
        f: Option<F>,
    }

    impl<T, F: FnOnce(T)> Defer<T, F> {
        pub fn new(value: T, func: F) -> Self {
            Defer {
                value: ManuallyDrop::new(value),
                f: Some(func),
            }
        }

        pub fn take(mut self) -> T {
            self.f = None;
            unsafe { ManuallyDrop::take(&mut self.value) }
        }
    }

    impl<T, F: FnOnce(T)> Deref for Defer<T, F> {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            &self.value
        }
    }

    impl<T, F: FnOnce(T)> DerefMut for Defer<T, F> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.value
        }
    }

    impl<T, F> Drop for Defer<T, F>
    where
        F: FnOnce(T),
    {
        fn drop(&mut self) {
            if let Some(x) = self.f.take() {
                unsafe { (x)(ManuallyDrop::take(&mut self.value)) };
            }
        }
    }
}
