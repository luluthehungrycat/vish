//! exit — exit the shell with an optional exit code.

use alloc::string::String;
use crate::io::WriteStr;
use crate::builtins::BuiltinResult;

pub fn exit(args: &[String], writer: &mut dyn WriteStr) -> BuiltinResult {
    let code = if args.is_empty() {
        0
    } else if args.len() == 1 {
        match args[0].parse::<i32>() {
            Ok(c) => c,
            Err(_) => {
                writer.write_str("exit: numeric argument required: ");
                writer.write_str(&args[0]);
                writer.write_str("\r\n");
                return BuiltinResult::Handled(2);
            }
        }
    } else {
        writer.write_str("exit: too many arguments\r\n");
        return BuiltinResult::Handled(1);
    };

    BuiltinResult::Exit(code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::StringWriter;

    #[test]
    fn test_exit_default() {
        let mut w = StringWriter::new();
        match exit(&[], &mut w) {
            BuiltinResult::Exit(0) => {}
            _ => panic!("expected Exit(0)"),
        }
    }

    #[test]
    fn test_exit_with_code() {
        let mut w = StringWriter::new();
        match exit(&["42".into()], &mut w) {
            BuiltinResult::Exit(42) => {}
            _ => panic!("expected Exit(42)"),
        }
    }

    #[test]
    fn test_exit_bad_arg() {
        let mut w = StringWriter::new();
        match exit(&["abc".into()], &mut w) {
            BuiltinResult::Handled(2) => {}
            _ => panic!("expected Handled(2)"),
        }
    }

    #[test]
    fn test_exit_too_many() {
        let mut w = StringWriter::new();
        match exit(&["1".into(), "2".into()], &mut w) {
            BuiltinResult::Handled(1) => {}
            _ => panic!("expected Handled(1)"),
        }
    }
}
