//! Line editor with history support.
//!
//! Provides `read_line()` which reads a line of input character-by-character,
//! with backspace, Ctrl+D (EOF), and arrow-key history navigation.
//!
//! History is a bounded ring buffer with deduplication.

use alloc::vec::Vec;
use alloc::string::String;
use crate::io::{ReadChar, WriteStr};

// ── Constants ────────────────────────────────────────────────────────────

/// Default shell prompt.
pub const PROMPT: &str = "vish$ ";

/// Maximum length of an input line.
pub const LINE_MAX: usize = 4096;

// ── History ──────────────────────────────────────────────────────────────

/// Bounded history ring buffer.
pub struct History {
    entries: Vec<String>,
    capacity: usize,
    /// Browse position (index into entries from the end).
    /// `None` means not browsing (at current input).
    browse: Option<usize>,
    /// Saved pending input while browsing.
    pending: String,
}

impl History {
    pub fn new(capacity: usize) -> Self {
        History {
            entries: Vec::with_capacity(capacity),
            capacity,
            browse: None,
            pending: String::new(),
        }
    }

    /// Add a new command to history. Deduplicates against last entry.
    pub fn add(&mut self, entry: String) {
        if entry.is_empty() {
            return;
        }
        // Skip if identical to the most recent entry
        if self.entries.last().map(|s| s == &entry).unwrap_or(false) {
            return;
        }
        if self.entries.len() >= self.capacity {
            self.entries.remove(0);
        }
        self.entries.push(entry);
        self.browse = None;
    }

    /// Navigate to an older entry. Returns the loaded command or `None`.
    pub fn navigate_up(&mut self, current: &str) -> Option<String> {
        if self.entries.is_empty() {
            return None;
        }

        match self.browse {
            None => {
                // First up-arrow: save current input, load newest entry
                self.pending = current.to_string();
                self.browse = Some(self.entries.len() - 1);
                Some(self.entries[self.entries.len() - 1].clone())
            }
            Some(pos) if pos > 0 => {
                let new_pos = pos - 1;
                self.browse = Some(new_pos);
                Some(self.entries[new_pos].clone())
            }
            Some(_) => {
                // Already at the oldest entry
                None
            }
        }
    }

    /// Navigate to a newer entry. Returns the loaded command, or the
    /// saved pending input when returning to the bottom.
    pub fn navigate_down(&mut self) -> Option<String> {
        match self.browse {
            Some(pos) if pos + 1 < self.entries.len() => {
                self.browse = Some(pos + 1);
                Some(self.entries[pos + 1].clone())
            }
            Some(_) => {
                // Back to the bottom — show pending input
                self.browse = None;
                let result = self.pending.clone();
                self.pending.clear();
                if result.is_empty() {
                    None
                } else {
                    Some(result)
                }
            }
            None => None,
        }
    }

    /// Reset browse state (call after committing a line).
    pub fn reset_browse(&mut self) {
        self.browse = None;
        self.pending.clear();
    }

    /// Access all history entries (for the `history` builtin).
    pub fn entries(&self) -> &[String] {
        &self.entries
    }
}

// ── Readline ─────────────────────────────────────────────────────────────

/// Clear from cursor to end of line.
const ANSI_CLEAR_LINE: &str = "\r\x1b[K";

/// Read one line of input from the user.
///
/// Returns the line (without trailing newline), or `Err(())` on EOF/Ctrl+D
/// when the line is empty.
pub fn read_line<R: ReadChar, W: WriteStr>(
    reader: &mut R,
    writer: &mut W,
    history: &mut History,
) -> Result<String, ()> {
    let mut buf = String::with_capacity(128);

    loop {
        match reader.read_char() {
            None => {
                // EOF / Ctrl+D
                if buf.is_empty() {
                    writer.write_str("\r\n");
                    return Err(());
                }
                // Non-empty line: treat as enter
                writer.write_str("\r\n");
                return Ok(buf);
            }
            Some(b) => match b {
                // Enter
                b'\n' | b'\r' => {
                    writer.write_str("\r\n");
                    return Ok(buf);
                }

                // Backspace (0x7F DEL or 0x08 BS)
                0x7F | 0x08 => {
                    if buf.pop().is_some() {
                        // Move cursor back, write space, move back again
                        writer.write_str("\x08 \x08");
                    }
                    history.reset_browse();
                }

                // Tab (ignore for now)
                0x09 => {}

                // Ctrl+C — clear line
                0x03 => {
                    if !buf.is_empty() {
                        buf.clear();
                        writer.write_str("^C");
                        writer.write_str(ANSI_CLEAR_LINE);
                        writer.write_str(PROMPT);
                    }
                    history.reset_browse();
                }

                // Ctrl+U — kill line
                0x15 => {
                    if !buf.is_empty() {
                        // Clear display with spaces
                        writer.write_str("\r");
                        writer.write_str(ANSI_CLEAR_LINE);
                        writer.write_str(PROMPT);
                        buf.clear();
                    }
                    history.reset_browse();
                }

                // Ctrl+D on empty line → EOF
                0x04 => {
                    if buf.is_empty() {
                        writer.write_str("\r\n");
                        return Err(());
                    }
                    // Otherwise ignore (future: delete character)
                }

                // ESC sequence: arrow keys
                0x1b => {
                    let next1 = reader.read_char();
                    if next1 == Some(0x5b) {
                        match reader.read_char() {
                            Some(0x41) => {
                                // Up arrow — history older
                                if let Some(entry) = history.navigate_up(&buf) {
                                    replace_line(writer, &entry, &buf);
                                    buf = entry;
                                }
                            }
                            Some(0x42) => {
                                // Down arrow — history newer
                                if let Some(entry) = history.navigate_down() {
                                    replace_line(writer, &entry, &buf);
                                    buf = entry;
                                } else {
                                    // Back to empty pending
                                    replace_line(writer, "", &buf);
                                    buf.clear();
                                }
                            }
                            _ => {}
                        }
                    }
                }

                // Printable character
                _ => {
                    if buf.len() < LINE_MAX {
                        buf.push(b as char);
                        writer.write_char(b);
                    }
                    history.reset_browse();
                }
            },
        }
    }
}

/// Replace the current displayed line with new content.
fn replace_line(writer: &mut impl WriteStr, new: &str, old: &str) {
    let old_len = old.len();
    let new_len = new.len();

    writer.write_str("\r");
    writer.write_str(ANSI_CLEAR_LINE);
    writer.write_str(PROMPT);
    writer.write_str(new);
    // ANSI_CLEAR_LINE already handles erasing trailing characters
    // since we CR + clear the whole line before redrawing.
    let _ = (old_len, new_len);
}

// ── Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::{StringWriter, NullReader};

    #[test]
    fn test_history_add() {
        let mut h = History::new(4);
        h.add("echo hi".into());
        h.add("ls -la".into());
        assert_eq!(h.entries().len(), 2);
    }

    #[test]
    fn test_history_dedup() {
        let mut h = History::new(4);
        h.add("echo hi".into());
        h.add("echo hi".into()); // duplicate
        assert_eq!(h.entries().len(), 1);
    }

    #[test]
    fn test_history_capacity() {
        let mut h = History::new(2);
        h.add("a".into());
        h.add("b".into());
        h.add("c".into()); // evicts "a"
        assert_eq!(h.entries().len(), 2);
        assert_eq!(h.entries()[0], "b");
        assert_eq!(h.entries()[1], "c");
    }

    #[test]
    fn test_history_navigate() {
        let mut h = History::new(4);
        h.add("echo a".into());
        h.add("echo b".into());
        h.add("echo c".into());

        assert_eq!(h.navigate_up("").as_deref(), Some("echo c"));
        assert_eq!(h.navigate_up("").as_deref(), Some("echo b"));
        assert_eq!(h.navigate_up("").as_deref(), Some("echo a"));
        assert_eq!(h.navigate_up(""), None); // already at oldest

        assert_eq!(h.navigate_down().as_deref(), Some("echo b"));
        assert_eq!(h.navigate_down().as_deref(), Some("echo c"));
        assert_eq!(h.navigate_down(), None); // back to current
    }

    #[test]
    fn test_empty_line_eof() {
        let mut r = NullReader;
        let mut w = StringWriter::new();
        let mut h = History::new(4);
        let result = read_line(&mut r, &mut w, &mut h);
        assert!(result.is_err());
    }
}
