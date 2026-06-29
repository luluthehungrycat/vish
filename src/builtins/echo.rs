//! echo — write arguments to stdout.
//!
//! Flags: -n (no trailing newline), -e (enable escapes), -E (disable escapes).
//! Default is -E (POSIX).

use alloc::string::String;
use crate::io::WriteStr;

pub fn echo(args: &[String], writer: &mut dyn WriteStr) -> i32 {
    let mut no_newline = false;
    let mut parse_escapes = false;
    let mut start = 0usize;

    // Parse flags before first non-flag argument
    for (i, arg) in args.iter().enumerate() {
        if arg == "--" {
            start = i + 1;
            break;
        }
        if arg.starts_with('-') && arg.len() > 1 {
            let mut all_valid = true;
            for ch in arg[1..].chars() {
                match ch {
                    'n' => no_newline = true,
                    'e' => parse_escapes = true,
                    'E' => parse_escapes = false,
                    _ => {
                        all_valid = false;
                        break;
                    }
                }
            }
            if all_valid {
                start = i + 1;
            } else {
                break;
            }
        } else {
            break;
        }
    }

    // Print arguments
    for (i, arg) in args[start..].iter().enumerate() {
        if i > 0 {
            writer.write_char(b' ');
        }
        if parse_escapes {
            write_escaped(writer, arg);
        } else {
            writer.write_str(arg);
        }
    }

    if !no_newline {
        writer.write_str("\r\n");
    }

    0
}

/// Write a string with escape sequences decoded.
fn write_escaped(writer: &mut dyn WriteStr, s: &str) {
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                None => writer.write_char(b'\\'),
                Some('a') => writer.write_char(0x07),
                Some('b') => writer.write_char(0x08),
                Some('c') => return, // stop output
                Some('e') => writer.write_char(0x1b),
                Some('E') => writer.write_char(0x1b),
                Some('f') => writer.write_char(0x0c),
                Some('n') => writer.write_str("\r\n"),
                Some('r') => writer.write_char(0x0d),
                Some('t') => writer.write_char(0x09),
                Some('v') => writer.write_char(0x0b),
                Some('\\') => writer.write_char(b'\\'),
                Some('\'') => writer.write_char(b'\''),
                Some('\"') => writer.write_char(b'"'),
                Some('x') => {
                    // hex escape \xNN
                    let mut hex = String::new();
                    for _ in 0..2 {
                        match chars.next() {
                            Some(c) if c.is_ascii_hexdigit() => hex.push(c),
                            Some(c) => {
                                // invalid hex — emit literal
                                writer.write_str("\\x");
                                writer.write_str(&hex);
                                writer.write_char(c as u8);
                                break;
                            }
                            None => {
                                writer.write_str("\\x");
                                writer.write_str(&hex);
                                return;
                            }
                        }
                    }
                    if hex.len() == 2 {
                        let val = u8::from_str_radix(&hex, 16).unwrap_or(0);
                        writer.write_char(val);
                    }
                }
                Some(c) if c.is_ascii_digit() && c <= '7' => {
                    // octal escape \0NNN
                    let mut oct = String::new();
                    oct.push(c);
                    for _ in 0..2 {
                        match chars.next() {
                            Some(c) if c >= '0' && c <= '7' => oct.push(c),
                            Some(c) => {
                                writer.write_char(
                                    u8::from_str_radix(&oct, 8).unwrap_or(0),
                                );
                                // Push back the non-octal char via an
                                // unfortunate allocation ...
                                let rest: String = [c].iter().collect();
                                writer.write_str(&rest);
                                return;
                            }
                            None => break,
                        }
                    }
                    let val = u8::from_str_radix(&oct, 8).unwrap_or(0);
                    writer.write_char(val);
                }
                Some(c) => {
                    // unknown escape — literal
                    writer.write_char(b'\\');
                    writer.write_char(c as u8);
                }
            }
        } else {
            let mut buf = [0u8; 4];
            let s = c.encode_utf8(&mut buf);
            writer.write_str(s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::StringWriter;

    #[test]
    fn test_echo_default() {
        let mut w = StringWriter::new();
        echo(&["hello".into(), "world".into()], &mut w);
        assert_eq!(w.buf, "hello world\r\n");
    }

    #[test]
    fn test_echo_n() {
        let mut w = StringWriter::new();
        echo(&["-n".into(), "hi".into()], &mut w);
        assert_eq!(w.buf, "hi");
    }

    #[test]
    fn test_echo_e_tab() {
        let mut w = StringWriter::new();
        echo(&["-e".into(), "tab\\there".into()], &mut w);
        assert_eq!(w.buf, "tab\there\r\n");
    }

    #[test]
    fn test_no_args() {
        let mut w = StringWriter::new();
        echo(&[], &mut w);
        assert_eq!(w.buf, "\r\n");
    }

    #[test]
    fn test_echo_dash_dash() {
        let mut w = StringWriter::new();
        echo(&["--".into(), "-n".into()], &mut w);
        assert_eq!(w.buf, "-n\r\n");
    }
}
