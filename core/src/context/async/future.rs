pub struct WithFuture<'js, F: 'js> {
    future: Pin<&'js mut F>,
    runtime: &'js mut RawRuntime,
}

impl<'js, F> Future for WithFuture<'js, F>
where
    F: Future,
{
    type Output = F::Output;
    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        unsafe {
            let res = self.future.as_mut().poll(cx);

            loop {
                if let Ok(true) = self.runtime.execute_pending_job() {
                    continue;
                }

                let fut = pin!(self.runtime.get_opaque_mut().spawner().drive());
                if let Poll::Ready(true) = fut.poll(cx) {
                    continue;
                }

                break;
            }
            res
        }
    }
}
