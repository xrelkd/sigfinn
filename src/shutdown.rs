use std::{future::Future, pin::Pin, task::Poll};

use tokio::sync::oneshot;

pub struct Shutdown(oneshot::Receiver<()>);

impl Shutdown {
    pub(crate) fn new(r: oneshot::Receiver<()>) -> Self { Self(r) }
}

impl Future for Shutdown {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.get_mut().0).poll(cx) {
            Poll::Ready(_) => Poll::Ready(()),
            Poll::Pending => Poll::Pending,
        }
    }
}
