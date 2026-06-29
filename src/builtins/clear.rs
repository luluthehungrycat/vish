//! clear — clear the terminal screen.

use crate::io::WriteStr;

/// ANSI escape sequence: clear screen and home cursor.
const CLEAR_SCREEN: &str = "\x1b[2J\x1b[H";

pub fn clear(writer: &mut dyn WriteStr) -> i32 {
    writer.write_str(CLEAR_SCREEN);
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::StringWriter;

    #[test]
    fn test_clear() {
        let mut w = StringWriter::new();
        clear(&mut w);
        assert_eq!(w.buf, CLEAR_SCREEN);
    }
}
