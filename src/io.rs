//! Platform-independent I/O traits.
//!
//! The core shell logic operates on these traits. Each platform backend
//! (Linux, VIBIX) provides its own implementation.

use alloc::string::String;

/// Read individual characters from an input source.
pub trait ReadChar {
    /// Read one byte. Returns `None` on EOF / closed.
    fn read_char(&mut self) -> Option<u8>;
}

/// Write text to an output sink.
pub trait WriteStr {
    fn write_str(&mut self, s: &str);
    fn write_char(&mut self, c: u8);
    fn flush(&mut self);
}

// ── Null implementations (useful for testing, /dev/null) ────────────────

pub struct NullReader;
impl ReadChar for NullReader {
    fn read_char(&mut self) -> Option<u8> {
        None // immediate EOF
    }
}

pub struct NullWriter;
impl WriteStr for NullWriter {
    fn write_str(&mut self, _s: &str) {}
    fn write_char(&mut self, _c: u8) {}
    fn flush(&mut self) {}
}

// ── String-backed writer for tests ──────────────────────────────────────

pub struct StringWriter {
    pub buf: String,
}
impl StringWriter {
    pub fn new() -> Self {
        StringWriter {
            buf: String::new(),
        }
    }
}
impl WriteStr for StringWriter {
    fn write_str(&mut self, s: &str) {
        self.buf.push_str(s);
    }
    fn write_char(&mut self, c: u8) {
        self.buf.push(c as char);
    }
    fn flush(&mut self) {}
}
