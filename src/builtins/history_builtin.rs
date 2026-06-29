//! history — display the command history list.

use crate::io::WriteStr;
use crate::readline::History;

/// Print numbered command history.
/// Note: this must be called from exec with history.entries().
/// Kept as a separate function for clarity.
pub fn history(writer: &mut dyn WriteStr) -> i32 {
    // This builtin is special: it relies on the caller having passed
    // in the history entries via a side channel (the exec function).
    // We use a global-style dispatch for simplicity.
    //
    // The actual implementation is in the exec module, which has access
    // to the History struct.
    //
    // This function is a placeholder that's overridden by exec's handling
    // of the "history" command.
    writer.write_str("history: internal error — use shell's history function\r\n");
    1
}

/// The real implementation, called from exec::execute when the command
/// is "history".
pub fn show_history(history: &History, writer: &mut dyn WriteStr) -> i32 {
    let entries = history.entries();
    if entries.is_empty() {
        writer.write_str("no history\r\n");
        return 0;
    }
    for (i, entry) in entries.iter().enumerate() {
        // Format: "  N  command"
        let line = alloc::format!("  {}  {}\r\n", i + 1, entry);
        writer.write_str(&line);
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::StringWriter;
    use crate::readline::History;

    #[test]
    fn test_show_history() {
        let mut h = History::new(4);
        h.add("echo hello".into());
        h.add("ls -la".into());

        let mut w = StringWriter::new();
        show_history(&h, &mut w);
        assert!(w.buf.contains("echo hello"));
        assert!(w.buf.contains("ls -la"));
        assert!(w.buf.contains("  1"));
        assert!(w.buf.contains("  2"));
    }

    #[test]
    fn test_empty_history() {
        let h = History::new(4);
        let mut w = StringWriter::new();
        show_history(&h, &mut w);
        assert_eq!(w.buf, "no history\r\n");
    }
}
