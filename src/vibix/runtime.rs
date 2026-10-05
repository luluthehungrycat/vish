#[cfg(test)]
use crate::io::{ReadChar, WriteStr};
#[cfg(not(test))]
use vish::io::{ReadChar, WriteStr};

fn syscall_read(fd: usize, buffer: &mut [u8]) -> isize {
    let mut result = 2usize;
    unsafe {
        core::arch::asm!("syscall", inlateout("rax") result, inlateout("rdi") fd => _,
            inlateout("rsi") buffer.as_mut_ptr() => _, inlateout("rdx") buffer.len() => _,
            lateout("r8") _, lateout("r9") _, lateout("r10") _,
            lateout("rcx") _, lateout("r11") _, options(nostack));
    }
    result as isize
}

fn syscall_write(fd: usize, buffer: &[u8]) -> isize {
    let mut result = 1usize;
    unsafe {
        core::arch::asm!("syscall", inlateout("rax") result, inlateout("rdi") fd => _,
            inlateout("rsi") buffer.as_ptr() => _, inlateout("rdx") buffer.len() => _,
            lateout("r8") _, lateout("r9") _, lateout("r10") _,
            lateout("rcx") _, lateout("r11") _, options(nostack));
    }
    result as isize
}

pub fn syscall_exit(code: i32) -> ! {
    unsafe {
        let mut result = 0usize;
        core::arch::asm!("syscall", inlateout("rax") result, in("rdi") code as usize,
            lateout("rcx") _, lateout("r11") _, options(nostack));
    }
    loop { core::hint::spin_loop(); }
}

fn decode_read(result: isize, byte: u8) -> Option<u8> {
    if result == 1 { Some(byte) } else { None }
}

fn read_should_retry(result: isize) -> bool {
    result == 0
}

pub struct VibixReader;
impl ReadChar for VibixReader {
    fn read_char(&mut self) -> Option<u8> {
        loop {
            let mut byte = [0u8; 1];
            let result = syscall_read(0, &mut byte);
            if read_should_retry(result) {
                // VIBIX blocks the task for input, then resumes this syscall
                // with a zero-byte result. Retry to consume the queued byte.
                core::hint::spin_loop();
                continue;
            }
            return decode_read(result, byte[0]);
        }
    }
}

pub struct VibixWriter;
impl WriteStr for VibixWriter {
    fn write_str(&mut self, text: &str) {
        let mut written = 0;
        while written < text.len() {
            let count = syscall_write(1, &text.as_bytes()[written..]);
            if count <= 0 { break; }
            written += count as usize;
        }
    }

    fn write_char(&mut self, character: u8) {
        let _ = syscall_write(1, &[character]);
    }

    fn flush(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::{decode_read, read_should_retry};

    #[test]
    fn read_translates_data_eof_and_error() {
        assert_eq!(decode_read(1, b'x'), Some(b'x'));
        assert_eq!(decode_read(0, 0), None);
        assert_eq!(decode_read(-1, 0), None);
    }

    #[test]
    fn zero_byte_read_is_a_retry_condition_on_vibix() {
        assert!(read_should_retry(0));
        assert!(!read_should_retry(-1));
    }
}
