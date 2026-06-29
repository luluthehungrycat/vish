//! cd — change the current working directory.
//!
//! `cd` with no arguments: no-op (or go to HOME in the future).
//! `cd DIR`: change to DIR.

use alloc::vec::Vec;
use alloc::string::String;
use crate::io::WriteStr;

pub fn cd(args: &[String], env: &mut Vec<String>, writer: &mut dyn WriteStr) -> i32 {
    let target = if args.is_empty() {
        // No argument: try $HOME (for now, just return)
        // In a full shell, we'd look up HOME from env
        writer.write_str("cd: no directory argument\r\n");
        return 1;
    } else {
        &args[0]
    };

    // Perform the directory change
    match change_dir(target, env) {
        Ok(()) => {
            // Update PWD in environment
            update_pwd(env);
            0
        }
        Err(msg) => {
            writer.write_str("cd: ");
            writer.write_str(target);
            writer.write_str(": ");
            writer.write_str(&msg);
            writer.write_str("\r\n");
            1
        }
    }
}

/// Change directory using the platform backend.
fn change_dir(path: &str, _env: &mut Vec<String>) -> Result<(), String> {
    #[cfg(feature = "std")]
    {
        std::env::set_current_dir(path).map_err(|e| e.to_string())
    }

    #[cfg(not(feature = "std"))]
    {
        let _ = path;
        Err("cd not yet implemented for this platform".into())
    }
}

/// Update PWD in the environment after a successful cd.
fn update_pwd(env: &mut Vec<String>) {
    #[cfg(feature = "std")]
    {
        if let Ok(cwd) = std::env::current_dir() {
            let new_entry = alloc::format!("PWD={}", cwd.display());
            // Replace existing PWD or add
            let mut found = false;
            for entry in env.iter_mut() {
                if entry.starts_with("PWD=") {
                    *entry = new_entry.clone();
                    found = true;
                    break;
                }
            }
            if !found {
                env.push(new_entry);
            }
        }
    }

    #[cfg(not(feature = "std"))]
    {
        let _ = env;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::StringWriter;

    #[test]
    fn test_cd_no_args() {
        let mut w = StringWriter::new();
        let mut env: Vec<String> = vec!["PWD=/tmp".into()];
        let code = cd(&[], &mut env, &mut w);
        assert_ne!(code, 0);
        assert!(w.buf.contains("no directory"));
    }

    #[test]
    fn test_cd_nonexistent() {
        let mut w = StringWriter::new();
        let mut env: Vec<String> = vec!["PWD=/tmp".into()];
        let code = cd(&["/nonexistent_dir_xyz_12345".into()], &mut env, &mut w);
        assert_ne!(code, 0);
        assert!(w.buf.contains("No such file") || w.buf.contains("cd:"));
    }

    #[test]
    fn test_cd_valid() {
        let mut w = StringWriter::new();
        let mut env: Vec<String> = vec!["PWD=/".into()];
        let code = cd(&["/tmp".into()], &mut env, &mut w);
        assert_eq!(code, 0);
        // PWD should be updated
        assert!(env.iter().any(|e| e.starts_with("PWD=")));
    }
}
