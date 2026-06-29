//! printenv — print all or part of the environment.
//!
//! `printenv` with no arguments prints all environment variables.
//! `printenv VAR` prints only the value of VAR (if set).

use alloc::vec::Vec;
use alloc::string::String;
use crate::io::WriteStr;

pub fn printenv(args: &[String], env: &mut Vec<String>, writer: &mut dyn WriteStr) -> i32 {
    if args.is_empty() {
        // Print all environment variables
        for entry in env.iter() {
            writer.write_str(entry);
            writer.write_str("\r\n");
        }
        0
    } else {
        // Look up each named variable
        let mut found = false;
        for name in args {
            let prefix = alloc::format!("{}=", name);
            let mut matched = false;
            for entry in env.iter() {
                if entry.starts_with(&prefix) {
                    writer.write_str(&entry[prefix.len()..]);
                    writer.write_str("\r\n");
                    matched = true;
                    found = true;
                    break;
                }
            }
            if !matched {
                // Not found — print nothing (exit code is still 0 for
                // individual missing vars per POSIX? Actually 0.)
            }
        }
        if found { 0 } else { 1 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::StringWriter;

    fn test_env() -> Vec<String> {
        vec!["HOME=/home/user".into(), "PATH=/usr/bin:/bin".into(), "SHELL=/bin/bash".into()]
    }

    #[test]
    fn test_printenv_all() {
        let mut w = StringWriter::new();
        let mut env = test_env();
        let code = printenv(&[], &mut env, &mut w);
        assert_eq!(code, 0);
        assert!(w.buf.contains("HOME=/home/user"));
        assert!(w.buf.contains("PATH=/usr/bin:/bin"));
    }

    #[test]
    fn test_printenv_one() {
        let mut w = StringWriter::new();
        let mut env = test_env();
        let code = printenv(&["HOME".into()], &mut env, &mut w);
        assert_eq!(code, 0);
        assert_eq!(w.buf, "/home/user\r\n");
    }

    #[test]
    fn test_printenv_missing() {
        let mut w = StringWriter::new();
        let mut env = test_env();
        let code = printenv(&["NOTSET".into()], &mut env, &mut w);
        assert_eq!(code, 1);
        assert!(w.buf.is_empty());
    }
}
