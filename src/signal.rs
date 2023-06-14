#![allow(clippy::module_name_repetitions)]
use std::fmt;

use tokio::signal::unix::SignalKind;

#[derive(Clone, Copy, Debug)]
pub enum UnixSignal {
    Terminate,
    Interrupt,
}

impl UnixSignal {
    pub const fn to_signal_kind(self) -> SignalKind {
        match self {
            Self::Terminate => SignalKind::terminate(),
            Self::Interrupt => SignalKind::interrupt(),
        }
    }
}

impl fmt::Display for UnixSignal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Terminate => "SIGTERM",
            Self::Interrupt => "SIGINT",
        };

        f.write_str(s)
    }
}
