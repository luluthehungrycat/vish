//! help — display information about builtin commands.

use crate::io::WriteStr;
use crate::builtins::BUILTIN_NAMES;

const HELP_TEXT: &str = "\
vish — VIBIX SHell

Built-in commands:\r\n";

pub fn help(writer: &mut dyn WriteStr) -> i32 {
    writer.write_str(HELP_TEXT);

    for name in BUILTIN_NAMES {
        writer.write_str("  ");
        writer.write_str(name);
        writer.write_str("\r\n");
    }

    writer.write_str("\r\n");
    writer.write_str("Use 'man' or external commands for detailed help.\r\n");

    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::StringWriter;

    #[test]
    fn test_help() {
        let mut w = StringWriter::new();
        help(&mut w);
        assert!(w.buf.contains("echo"));
        assert!(w.buf.contains("exit"));
        assert!(w.buf.contains("cat"));
    }
}
