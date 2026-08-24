use std::os::fd::{BorrowedFd, OwnedFd};

use rustix::fs::{self, Mode, OFlags};
use rustix::io::{self, Errno};
use rustix::stdio;
use rustix::termios::{
    self, ControlModes, InputModes, LocalModes, OutputModes, SpecialCodeIndex, Termios,
};

#[cfg(target_os = "macos")]
const POSIX_VDISABLE: u8 = 0xff;

#[cfg(target_os = "linux")]
const POSIX_VDISABLE: u8 = 0;

pub fn find() -> Option<OwnedFd> {
    for fd in [stdio::stderr(), stdio::stdout()] {
        if termios::isatty(fd)
            && is_writable(fd)
            && let Ok(owned) = io::dup(fd)
        {
            return Some(owned);
        }
    }

    let input = stdio::stdin();
    if termios::isatty(input)
        && let Ok(path) = termios::ttyname(input, Vec::new())
        && let Ok(owned) = fs::open(
            path.as_c_str(),
            OFlags::RDWR | OFlags::NOCTTY | OFlags::CLOEXEC,
            Mode::empty(),
        )
    {
        return Some(owned);
    }

    fs::open(
        "/dev/tty",
        OFlags::RDWR | OFlags::NOCTTY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .ok()
}

fn is_writable(fd: BorrowedFd<'_>) -> bool {
    fs::fcntl_getfl(fd).is_ok_and(|flags| flags & OFlags::ACCMODE != OFlags::RDONLY)
}

pub fn retry_on_interrupt<T>(mut operation: impl FnMut() -> io::Result<T>) -> io::Result<T> {
    loop {
        match operation() {
            Err(Errno::INTR) => {}
            result => return result,
        }
    }
}

pub fn repair_terminal_mode(mode: &mut Termios) {
    repair_input_modes(&mut mode.input_modes);
    repair_output_modes(&mut mode.output_modes);
    repair_control_modes(&mut mode.control_modes);
    repair_local_modes(&mut mode.local_modes);

    repair_control_character(mode, SpecialCodeIndex::VEOF, 0o4);
    repair_control_character(mode, SpecialCodeIndex::VERASE, 0o177);
    repair_control_character(mode, SpecialCodeIndex::VWERASE, 0o27);
    repair_control_character(mode, SpecialCodeIndex::VKILL, 0o25);
    repair_control_character(mode, SpecialCodeIndex::VREPRINT, 0o22);
    repair_control_character(mode, SpecialCodeIndex::VINTR, 0o3);
    repair_control_character(mode, SpecialCodeIndex::VQUIT, 0o34);
    repair_control_character(mode, SpecialCodeIndex::VSUSP, 0o32);
    repair_control_character(mode, SpecialCodeIndex::VSTART, 0o21);
    repair_control_character(mode, SpecialCodeIndex::VSTOP, 0o23);
    repair_control_character(mode, SpecialCodeIndex::VLNEXT, 0o26);
    repair_control_character(mode, SpecialCodeIndex::VDISCARD, 0o17);
    repair_darwin_control_characters(mode);
}

fn repair_input_modes(modes: &mut InputModes) {
    modes.remove(
        InputModes::IGNBRK
            | InputModes::PARMRK
            | InputModes::INPCK
            | InputModes::ISTRIP
            | InputModes::INLCR
            | InputModes::IGNCR
            | InputModes::IXOFF
            | InputModes::IXANY,
    );
    modes.insert(
        InputModes::BRKINT
            | InputModes::IGNPAR
            | InputModes::ICRNL
            | InputModes::IXON
            | InputModes::IMAXBEL,
    );
}

fn repair_output_modes(modes: &mut OutputModes) {
    modes.remove(OutputModes::OCRNL | OutputModes::ONOCR | OutputModes::ONLRET);
    remove_output_delay_modes(modes);
    modes.insert(OutputModes::OPOST | OutputModes::ONLCR);
}

fn repair_control_modes(modes: &mut ControlModes) {
    modes.remove(
        ControlModes::CSIZE
            | ControlModes::PARENB
            | ControlModes::PARODD
            | ControlModes::CSTOPB
            | ControlModes::CLOCAL,
    );
    modes.insert(ControlModes::CS8 | ControlModes::CREAD);
}

fn repair_local_modes(modes: &mut LocalModes) {
    modes.remove(
        LocalModes::ECHONL
            | LocalModes::NOFLSH
            | LocalModes::TOSTOP
            | LocalModes::ECHOPRT
            | LocalModes::FLUSHO
            | LocalModes::PENDIN
            | LocalModes::EXTPROC,
    );
    modes.insert(
        LocalModes::ISIG
            | LocalModes::ICANON
            | LocalModes::IEXTEN
            | LocalModes::ECHO
            | LocalModes::ECHOE
            | LocalModes::ECHOK
            | LocalModes::ECHOKE
            | LocalModes::ECHOCTL,
    );
}

#[cfg(target_os = "linux")]
fn remove_output_delay_modes(modes: &mut OutputModes) {
    modes.remove(
        OutputModes::OFILL
            | OutputModes::OFDEL
            | OutputModes::NLDLY
            | OutputModes::CRDLY
            | OutputModes::TABDLY
            | OutputModes::BSDLY
            | OutputModes::VTDLY
            | OutputModes::FFDLY,
    );
}

#[cfg(target_os = "macos")]
fn remove_output_delay_modes(modes: &mut OutputModes) {
    modes.remove(OutputModes::from_bits_retain(
        libc::OFILL
            | libc::OFDEL
            | libc::NLDLY
            | libc::CRDLY
            | libc::TABDLY
            | libc::BSDLY
            | libc::VTDLY
            | libc::FFDLY,
    ));
}

#[cfg(target_os = "macos")]
fn repair_darwin_control_characters(mode: &mut Termios) {
    repair_control_character(mode, SpecialCodeIndex::VDSUSP, 0o31);
    repair_control_character(mode, SpecialCodeIndex::VSTATUS, 0o24);
}

#[cfg(target_os = "linux")]
const fn repair_darwin_control_characters(_mode: &mut Termios) {}

fn repair_control_character(mode: &mut Termios, index: SpecialCodeIndex, default: u8) {
    repair_control_character_value(&mut mode.special_codes[index], default);
}

const fn repair_control_character_value(current: &mut u8, default: u8) {
    if *current == 0 || *current == POSIX_VDISABLE {
        *current = default;
    }
}

#[cfg(test)]
#[path = "tty_tests.rs"]
mod tests;
