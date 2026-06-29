//! cat — concatenate files and print to stdout.
//!
//! With no arguments or `-`, reads from stdin.
//! With file arguments, reads and outputs each file in order.

use alloc::string::String;
use crate::io::WriteStr;

pub fn cat(args: &[String], writer: &mut dyn WriteStr) -> i32 {
    let mut exit_code = 0;

    if args.is_empty() {
        // Read from stdin — for interactive mode, this doesn't
        // make much sense. We'll just output a brief message.
        // In a real shell with external commands, this would be handled
        // by piping or redirecting stdin.
        writer.write_str("cat: reading from stdin (not yet supported)\r\n");
        return 1;
    }

    for arg in args {
        if arg == "-" {
            writer.write_str("cat: reading from stdin (not yet supported)\r\n");
            continue;
        }
        match read_file(arg) {
            Some(contents) => {
                writer.write_str(&contents);
            }
            None => {
                writer.write_str("cat: ");
                writer.write_str(arg);
                writer.write_str(": No such file or directory\r\n");
                exit_code = 1;
            }
        }
    }

    exit_code
}

/// Read a file's contents into a string. Returns `None` if the file
/// can't be read.
fn read_file(path: &str) -> Option<String> {
    // On no_std targets this will need a syscall-based implementation.
    // For now, use std::fs when available.
    #[cfg(feature = "std")]
    {
        std::fs::read_to_string(path).ok()
    }

    #[cfg(not(feature = "std"))]
    {
        // TODO: implement VIBIX file reading via syscall
        let _ = path;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::StringWriter;

    #[test]
    fn test_cat_no_args() {
        let mut w = StringWriter::new();
        let code = cat(&[], &mut w);
        assert_ne!(code, 0);
    }

    #[test]
    fn test_cat_nonexistent() {
        let mut w = StringWriter::new();
        let code = cat(&["/nonexistent_file_xyz".into()], &mut w);
        assert_eq!(code, 1);
        assert!(w.buf.contains("No such file"));
    }
}
