use super::{Executor, Idle, Inner, Opaque, Spawner};
use crate::{Runtime, SendWhenParallel};
use std::future::Future;

/// The trait to spawn execution of pending jobs on async runtime
#[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
pub trait ExecutorSpawner: Sized {
    /// The type of join handle which returns `()`
    type JoinHandle;

    /// Spawn pending jobs using async runtime spawn function
    fn spawn_executor(task: Executor) -> Self::JoinHandle;
}

macro_rules! async_rt_impl {
    ($($(#[$meta:meta])* $type:ident { $join_handle:ty, $spawn_local:path, $spawn:path })*) => {
        $(
            $(#[$meta])*
            impl ExecutorSpawner for crate::$type {
                type JoinHandle = $join_handle;

                fn spawn_executor(
                    task: Executor,
                ) -> Self::JoinHandle {
                    #[cfg(not(feature = "parallel"))]
                    use $spawn_local as spawn_parallel;
                    #[cfg(feature = "parallel")]
                    use $spawn as spawn_parallel;

                    spawn_parallel(task)
                }
            }
        )*
    };
}

async_rt_impl! {
    #[cfg(feature = "tokio")]
    Tokio { tokio::task::JoinHandle<()>, tokio::task::spawn_local, tokio::task::spawn }
    #[cfg(feature = "async-std")]
    AsyncStd { async_std::task::JoinHandle<()>, async_std::task::spawn_local, async_std::task::spawn }
}

impl Inner {
    pub fn has_spawner(&self) -> bool {
        unsafe { self.get_opaque() }.spawner.is_some()
    }
}

impl Opaque {
    pub fn get_spawner(&self) -> &Spawner {
        self.spawner
            .as_ref()
            .expect("Async executor is not initialized for the Runtime. Possibly missing call `Runtime::run_executor()` or `Runtime::spawn_executor()`")
    }
}

impl Runtime {
    fn get_spawner(&self) -> &Spawner {
        let inner = self.inner.lock();
        let opaque = unsafe { &*(inner.get_opaque() as *const Opaque) };
        opaque.get_spawner()
    }

    /// Await until all pending jobs and spawned futures will be done
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
    #[inline(always)]
    pub fn idle(&self) -> Idle {
        self.get_spawner().idle()
    }

    /// Run pending jobs and futures executor
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
    #[inline(always)]
    pub fn run_executor(&self) -> Executor {
        let mut inner = self.inner.lock();
        let opaque = unsafe { &mut *(inner.get_opaque_mut() as *mut Opaque) };
        if opaque.spawner.is_some() {
            panic!("Async executor already initialized for the Runtime.");
        }
        let (executor, spawner) = Executor::new();
        opaque.spawner = Some(spawner);
        executor
    }

    /// Spawn pending jobs and futures executor
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
    #[inline(always)]
    pub fn spawn_executor<A: ExecutorSpawner>(&self) -> A::JoinHandle {
        A::spawn_executor(self.run_executor())
    }

    pub(crate) fn spawn_pending_jobs(&self) {
        let runtime = self.clone();
        self.spawn(async move { runtime.execute_pending_jobs().await });
    }

    async fn execute_pending_jobs(&self) {
        loop {
            match self.execute_pending_job() {
                // No tasks in queue
                Ok(false) => break,
                // Task was executed successfully
                Ok(true) => (),
                // Task was failed with exception
                Err(error) => {
                    eprintln!("Error when pending job executing: {}", error);
                }
            }
            futures_lite::future::yield_now().await;
        }
    }

    /// Spawn future using runtime
    #[cfg_attr(feature = "doc-cfg", doc(cfg(feature = "futures")))]
    pub fn spawn<F, T>(&self, future: F)
    where
        F: Future<Output = T> + SendWhenParallel + 'static,
        T: SendWhenParallel + 'static,
    {
        self.get_spawner().spawn(future);
    }
}
