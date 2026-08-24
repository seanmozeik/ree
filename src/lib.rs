//! A fast terminal reset for macOS and Linux.
//!
//! `ree` repairs the kernel TTY state and resets the attached terminal emulator
//! without linking to ncurses.

#![cfg_attr(
    not(any(target_os = "linux", target_os = "macos")),
    allow(unused_imports)
)]

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
compile_error!("ree supports macOS and Linux");

mod display;
mod error;
mod reset;
mod terminfo;
mod terminfo_db;
mod tty;

pub use error::Error;
pub use reset::reset;
