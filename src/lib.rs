mod error;
mod event;
mod exit_status;
mod handle;
mod shutdown;
mod signal;

use std::fmt;

use futures::{
    future,
    future::{Either, FutureExt},
    Future,
};
use snafu::ResultExt;
use tokio::{
    signal::unix::signal,
    sync::{mpsc, oneshot},
    task::JoinSet,
};

pub use self::{
    error::{Error, Result},
    exit_status::ExitStatus,
    handle::Handle,
    shutdown::Shutdown,
};
use crate::{event::Event, signal::UnixSignal};

pub struct LifecycleManager<ErrorType = ()> {
    handle: Handle<ErrorType>,
    event_receiver: mpsc::UnboundedReceiver<Event<ErrorType>>,
}

impl<ErrorType> Default for LifecycleManager<ErrorType>
where
    ErrorType: Send + 'static,
{
    fn default() -> Self { Self::new() }
}

impl<ErrorType> LifecycleManager<ErrorType>
where
    ErrorType: Send + 'static,
{
    #[must_use]
    pub fn new() -> Self {
        let (event_sender, event_receiver) = mpsc::unbounded_channel();
        let handle = Handle::new(event_sender);

        Self { handle, event_receiver }
    }

    #[inline]
    #[must_use]
    pub fn handle(&self) -> Handle<ErrorType> { self.handle.clone() }

    #[inline]
    pub fn spawn<FutureName, CreateFutureFn, Fut>(
        &self,
        name: FutureName,
        create_future: CreateFutureFn,
    ) -> Handle<ErrorType>
    where
        FutureName: fmt::Display,
        CreateFutureFn: FnOnce(Shutdown) -> Fut + Send + 'static,
        Fut: Future<Output = ExitStatus<ErrorType>> + Send + 'static,
    {
        self.handle.spawn(name, create_future)
    }

    fn init_signal_watcher(&self, sig: UnixSignal) -> Result<()> {
        tracing::debug!("Create UNIX signal listener for `{sig}`");
        let mut signal =
            signal(sig.to_signal_kind()).context(error::CreateUnixSignalListenerSnafu)?;

        let handle = self.handle();

        self.spawn(format!("UNIX signal listener ({sig})"), move |internal_signal| async move {
            tracing::debug!("Wait for signal `{sig}`");

            match future::select(internal_signal, signal.recv().boxed()).await {
                Either::Left(_) => {}
                Either::Right(_) => {
                    tracing::info!("`{sig}` received, starting graceful shutdown");
                    handle.on_signal(sig);
                }
            }

            ExitStatus::Success
        });

        Ok(())
    }

    /// # Errors
    ///
    /// - returns error while failed to join task
    pub async fn serve(mut self) -> Result<std::result::Result<(), ErrorType>> {
        let signals = [UnixSignal::Interrupt, UnixSignal::Terminate];
        for sig in signals {
            self.init_signal_watcher(sig)?;
        }

        let mut join_set = JoinSet::<()>::new();
        let mut shutdown_senders: Vec<(String, oneshot::Sender<()>)> = Vec::new();
        let mut maybe_error = None;

        while let Some(event) = self.event_receiver.recv().await {
            match event {
                Event::NewFuture { name, shutdown_sender, future } => {
                    shutdown_senders.push((name, shutdown_sender));
                    join_set.spawn(future);
                }
                Event::Signal(signal) => {
                    tracing::debug!("Receive signal `{signal}`");
                    break;
                }
                Event::Shutdown => {
                    tracing::debug!("Receive shutdown signal from internal");
                    break;
                }
                Event::FutureCompleted { name, exit_status } => {
                    match join_set.join_next().await {
                        Some(Ok(())) => {}
                        Some(Err(err)) => {
                            tracing::error!("Error while joining tokio `Task`, error: {err}");
                        }
                        None => {
                            tracing::debug!("All futures are completed");
                            break;
                        }
                    };

                    match exit_status {
                        ExitStatus::Success => {
                            tracing::debug!("Future `{name}` completed");
                            if join_set.len() <= signals.len() {
                                break;
                            }
                        }
                        ExitStatus::Failure(error) => {
                            tracing::error!("Future `{name}` failed, starting graceful shutdown");
                            maybe_error = Some(error);
                            break;
                        }
                    }
                }
            }
        }

        for (name, sender) in shutdown_senders {
            tracing::info!("Shut down `{name}`");
            drop(sender);
        }

        while let Some(result) = join_set.join_next().await {
            result.context(error::JoinTaskHandleSnafu)?;
        }

        maybe_error.map_or_else(|| Ok(Ok(())), |err| Ok(Err(err)))
    }
}
