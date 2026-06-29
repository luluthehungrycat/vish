//! Raw terminal mode management for Linux.
//!
//! Uses `libc::tcgetattr`/`tcsetattr` to switch stdin between
//! cooked (line-buffered) and raw (character-by-character) modes.
//! The `RawMode` guard automatically restores the terminal on drop.

use std::io;

pub struct RawMode {
    orig: libc::termios,
}

impl RawMode {
    /// Enter raw mode. The terminal is restored when the returned
    /// guard is dropped.
    pub fn enter() -> io::Result<Self> {
        let fd = libc::STDIN_FILENO;
        let mut orig = unsafe { std::mem::zeroed::<libc::termios>() };

        if unsafe { libc::tcgetattr(fd, &mut orig) } != 0 {
            return Err(io::Error::last_os_error());
        }

        let mut raw = orig;
        unsafe {
            libc::cfmakeraw(&mut raw);
        }

        if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &raw) } != 0 {
            return Err(io::Error::last_os_error());
        }

        Ok(RawMode { orig })
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        unsafe {
            libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &self.orig);
        }
    }
}
