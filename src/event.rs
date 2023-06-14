use std::{future::Future, pin::Pin};

use tokio::sync::oneshot;

use crate::{exit_status::ExitStatus, signal::UnixSignal};

pub enum Event<Error> {
    NewFuture {
        name: String,
        shutdown_sender: oneshot::Sender<()>,
        future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
    },
    FutureCompleted {
        name: String,
        exit_status: ExitStatus<Error>,
    },
    Signal(UnixSignal),
    Shutdown,
}
