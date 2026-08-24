use rustix::io::Errno;
use thiserror::Error as ThisError;

/// An error that stopped `ree` from resetting the terminal.
#[derive(Debug, ThisError)]
#[non_exhaustive]
pub enum Error {
    /// No usable terminal was attached to standard input, output, error, or `/dev/tty`.
    #[error("no terminal found")]
    NoTerminal,
    /// The process is in a background job and must not change another job's terminal.
    #[error("the process is not in the terminal's foreground process group")]
    NotForegroundProcess,
    /// The process could not prove that it owns the terminal foreground.
    #[error("failed to read the terminal's foreground process group: {0}")]
    GetForegroundProcess(#[source] Errno),
    /// The terminal output queue could not be resumed.
    #[error("failed to resume terminal output: {0}")]
    ResumeOutput(#[source] Errno),
    /// The current terminal driver state could not be read.
    #[error("failed to read terminal state: {0}")]
    GetTerminalState(#[source] Errno),
    /// The repaired terminal driver state could not be applied.
    #[error("failed to set terminal state: {0}")]
    SetTerminalState(#[source] Errno),
    /// `TERM` is absent from the process environment.
    #[error("TERM is not set")]
    TermNotSet,
    /// `TERM` contains characters that cannot name a terminfo entry safely.
    #[error("TERM is not a valid terminal name")]
    InvalidTermName,
    /// No compiled terminfo entry was found for `TERM`.
    #[error("no terminfo entry was found for TERM")]
    TerminfoNotFound,
    /// A compiled terminfo entry could not be read.
    #[error("the terminfo entry for TERM could not be read")]
    TerminfoUnreadable,
    /// A compiled terminfo entry was structurally invalid.
    #[error("the terminfo entry for TERM is invalid")]
    InvalidTerminfo,
    /// A write to the terminal failed.
    #[error("failed to write terminal reset data: {0}")]
    TerminalWrite(#[source] Errno),
    /// A terminal write returned zero bytes before all reset data was sent.
    #[error("a terminal write made no progress")]
    TerminalWriteMadeNoProgress,
}

impl Error {
    /// Return whether writing a diagnostic could violate terminal job control.
    #[must_use]
    pub const fn requires_silent_exit(&self) -> bool {
        matches!(
            self,
            Self::NotForegroundProcess | Self::GetForegroundProcess(_)
        )
    }
}
