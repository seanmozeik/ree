use std::env;
use std::os::fd::OwnedFd;

use rustix::io::{self, Errno};

use crate::Error;
use crate::terminfo::{self, ResetStrings};

const VT_CLEANUP_SEQUENCE: &[u8] = concat!(
    "\x1b]\x1b\\", // terminate a stray OSC, DCS, APC, or PM string
    "\x1b[?2026l", // synchronized output off before other visible changes
    "\x1b[?9;1000;1002;1003;1004;1005;1006;1015;1016;2004;2031;2033;2048;5522l",
    "\x1b[<8u",   // clear Ghostty's Kitty keyboard stack
    "\x1b[=0u",   // disable Kitty keyboard flags in the current stack entry
    "\x1b[>4;0m", // disable xterm modifyOtherKeys
)
.as_bytes();

const VT_FALLBACK_SEQUENCE: &[u8] = concat!(
    "\x1b]\x1b\\", // leave a stray terminal string before RIS
    "\x1bc",       // RIS: reset terminal state
    "\x1b[!p",     // DECSTR: soft terminal reset
    "\x1b[?3;4l",  // 80 columns, smooth scrolling off
    "\x1b[4l",     // insert mode off
    "\x1b>",       // normal numeric keypad
    "\x1b(B",      // ASCII in the G0 character set
    "\x1b[?7h",    // wraparound on
    "\x1b[0m",     // default rendition
    "\x1b[?25h",   // cursor visible
)
.as_bytes();

pub fn reset(terminal: &OwnedFd) -> Result<(), Error> {
    let term = env::var_os("TERM").ok_or(Error::TermNotSet)?;
    let vt_compatible = terminfo::is_vt_compatible(&term);

    if vt_compatible {
        write_all(terminal, VT_CLEANUP_SEQUENCE)?;
    }

    let data = match terminfo::load(&term) {
        Ok(data) => data,
        Err(terminfo::Error::NotFound) if vt_compatible => {
            return write_all(terminal, VT_FALLBACK_SEQUENCE);
        }
        Err(error) => return Err(map_terminfo_error(error)),
    };

    let strings = terminfo::reset_strings(&data).map_err(map_terminfo_error)?;
    emit_reset_strings(terminal, &strings, vt_compatible)
}

fn emit_reset_strings(
    terminal: &OwnedFd,
    strings: &ResetStrings<'_>,
    vt_compatible: bool,
) -> Result<(), Error> {
    let mut sent = false;
    sent |= emit(terminal, strings.rs1.as_deref(), strings.is1.as_deref())?;
    sent |= emit(terminal, strings.rs2.as_deref(), strings.is2.as_deref())?;

    if let Some(clear_margins) = strings.clear_margins.as_deref() {
        write_all(terminal, clear_margins)?;
        sent = true;
    }

    sent |= emit(terminal, strings.rs3.as_deref(), strings.is3.as_deref())?;

    if !sent && vt_compatible {
        write_all(terminal, VT_FALLBACK_SEQUENCE)?;
    }

    Ok(())
}

fn emit(terminal: &OwnedFd, reset: Option<&[u8]>, init: Option<&[u8]>) -> Result<bool, Error> {
    let Some(bytes) = reset.or(init) else {
        return Ok(false);
    };

    write_all(terminal, bytes)?;
    Ok(true)
}

fn write_all(terminal: &OwnedFd, mut bytes: &[u8]) -> Result<(), Error> {
    while !bytes.is_empty() {
        match io::write(terminal, bytes) {
            Ok(0) => return Err(Error::TerminalWriteMadeNoProgress),
            Ok(written) => bytes = &bytes[written..],
            Err(Errno::INTR) => {}
            Err(error) => return Err(Error::TerminalWrite(error)),
        }
    }

    Ok(())
}

const fn map_terminfo_error(error: terminfo::Error) -> Error {
    match error {
        terminfo::Error::InvalidTermName => Error::InvalidTermName,
        terminfo::Error::NotFound => Error::TerminfoNotFound,
        terminfo::Error::Unreadable => Error::TerminfoUnreadable,
        terminfo::Error::TooSmall
        | terminfo::Error::BadMagic
        | terminfo::Error::Truncated
        | terminfo::Error::BadStringOffset
        | terminfo::Error::UnterminatedString
        | terminfo::Error::TooLarge => Error::InvalidTerminfo,
    }
}

#[cfg(test)]
#[path = "display_tests.rs"]
mod tests;
