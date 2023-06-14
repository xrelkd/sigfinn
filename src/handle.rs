use std::{fmt, future::Future};

use futures::FutureExt;
use tokio::sync::{mpsc, oneshot};

use crate::{event::Event, exit_status::ExitStatus, shutdown::Shutdown, signal::UnixSignal};

#[derive(Debug)]
pub struct Handle<Error> {
    event_sender: mpsc::UnboundedSender<Event<Error>>,
}

impl<ErrorType> Clone for Handle<ErrorType> {
    fn clone(&self) -> Self { Self { event_sender: self.event_sender.clone() } }
}

impl<ErrorType> Handle<ErrorType>
where
    ErrorType: Send + 'static,
{
    pub(crate) const fn new(event_sender: mpsc::UnboundedSender<Event<ErrorType>>) -> Self {
        Self { event_sender }
    }

    #[inline]
    #[must_use]
    pub fn spawn<FutureName, CreateFutureFn, Fut>(
        &self,
        name: FutureName,
        create_future: CreateFutureFn,
    ) -> Self
    where
        FutureName: fmt::Display,
        CreateFutureFn: FnOnce(Shutdown) -> Fut + Send + 'static,
        Fut: Future<Output = ExitStatus<ErrorType>> + Send + 'static,
    {
        let (shutdown_sender, shutdown_rx) = oneshot::channel();
        let name = name.to_string();
        let future = {
            let event_sender = self.event_sender.clone();
            let name = name.clone();
            async move {
                let exit_status = create_future(Shutdown::new(shutdown_rx)).await;
                event_sender.send(Event::FutureCompleted { name, exit_status }).ok();
            }
            .boxed()
        };
        self.event_sender.send(Event::NewFuture { name, shutdown_sender, future }).ok();
        self.clone()
    }

    pub fn shutdown(&self) { self.event_sender.send(Event::Shutdown).ok(); }

    pub(crate) fn on_signal(&self, signal: UnixSignal) {
        self.event_sender.send(Event::Signal(signal)).ok();
    }
}
