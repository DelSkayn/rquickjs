#[cfg(all(feature = "async-std", feature = "tokio"))]
compile_error!("Both `async-std` and `tokio` features cannot be enabled simultaneously.");

#[cfg(feature = "async-std")]
pub use async_std_rs::task::{spawn, spawn_local, yield_now, JoinHandle};

#[cfg(all(test, feature = "async-std"))]
pub use async_std_rs::task::block_on;

#[cfg(feature = "tokio")]
pub use tokio_rs::task::{spawn, spawn_local, yield_now, JoinHandle};

#[cfg(all(test, feature = "tokio"))]
pub fn block_on<F>(future: F) -> F::Output
where
    F: std::future::Future,
{
    tokio_rs::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(tokio_rs::task::LocalSet::new().run_until(future))
}

#[cfg(any(feature = "async-std", feature = "tokio"))]
#[cfg(not(feature = "parallel"))]
pub use spawn_local as spawn_parallel;

#[cfg(any(feature = "async-std", feature = "tokio"))]
#[cfg(feature = "parallel")]
pub use spawn as spawn_parallel;
