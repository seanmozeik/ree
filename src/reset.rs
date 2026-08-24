use rustix::{process, termios};

use crate::{Error, display, tty};

/// Repair the kernel TTY state and reset the attached terminal emulator.
///
/// # Errors
///
/// Returns [`Error`] when no terminal is available, the process is a background
/// job, terminal state cannot be changed, or the terminfo entry cannot be read.
///
/// # Examples
///
/// ```no_run
/// ree::reset()?;
/// # Ok::<(), ree::Error>(())
/// ```
pub fn reset() -> Result<(), Error> {
    let terminal = tty::find().ok_or(Error::NoTerminal)?;
    let foreground = tty::retry_on_interrupt(|| termios::tcgetpgrp(&terminal))
        .map_err(Error::GetForegroundProcess)?;
    if foreground != process::getpgrp() {
        return Err(Error::NotForegroundProcess);
    }

    tty::retry_on_interrupt(|| termios::tcflow(&terminal, termios::Action::OOn))
        .map_err(Error::ResumeOutput)?;

    let mut mode = tty::retry_on_interrupt(|| termios::tcgetattr(&terminal))
        .map_err(Error::GetTerminalState)?;
    tty::repair_terminal_mode(&mut mode);
    tty::retry_on_interrupt(|| {
        termios::tcsetattr(&terminal, termios::OptionalActions::Flush, &mode)
    })
    .map_err(Error::SetTerminalState)?;

    display::reset(&terminal)
}
