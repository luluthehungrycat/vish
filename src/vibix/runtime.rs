#[cfg(test)]
use crate::io::{ReadChar, WriteStr};
#[cfg(not(test))]
use vish::io::{ReadChar, WriteStr};

#[cfg(not(test))]
core::arch::global_asm!(
    ".section .text.vibix_syscall_wrappers,\"ax\"",
    ".global vibix_syscall_read",
    ".type vibix_syscall_read,@function",
    "vibix_syscall_read:",
    "push rbx",
    "push rbp",
    "push r12",
    "push r13",
    "push r14",
    "push r15",
    "mov rax, 2",
    "syscall",
    "pop r15",
    "pop r14",
    "pop r13",
    "pop r12",
    "pop rbp",
    "pop rbx",
    "ret",
    ".size vibix_syscall_read, .-vibix_syscall_read",
    ".global vibix_syscall_write",
    ".type vibix_syscall_write,@function",
    "vibix_syscall_write:",
    "push rbx",
    "push rbp",
    "push r12",
    "push r13",
    "push r14",
    "push r15",
    "mov rax, 1",
    "syscall",
    "pop r15",
    "pop r14",
    "pop r13",
    "pop r12",
    "pop rbp",
    "pop rbx",
    "ret",
    ".size vibix_syscall_write, .-vibix_syscall_write",
    ".global vibix_syscall_brk",
    ".type vibix_syscall_brk,@function",
    "vibix_syscall_brk:",
    "push rbx",
    "push rbp",
    "push r12",
    "push r13",
    "push r14",
    "push r15",
    "mov rax, 4",
    "syscall",
    "pop r15",
    "pop r14",
    "pop r13",
    "pop r12",
    "pop rbp",
    "pop rbx",
    "ret",
    ".size vibix_syscall_brk, .-vibix_syscall_brk",
);

#[cfg(not(test))]
extern "C" {
    fn vibix_syscall_read(fd: usize, buffer: *mut u8, len: usize) -> usize;
    fn vibix_syscall_write(fd: usize, buffer: *const u8, len: usize) -> usize;
}

#[cfg(not(test))]
fn syscall_read(fd: usize, buffer: &mut [u8]) -> isize {
    unsafe { vibix_syscall_read(fd, buffer.as_mut_ptr(), buffer.len()) as isize }
}

#[cfg(test)]
fn syscall_read(_fd: usize, _buffer: &mut [u8]) -> isize {
    0
}

#[cfg(not(test))]
fn syscall_write(fd: usize, buffer: &[u8]) -> isize {
    unsafe { vibix_syscall_write(fd, buffer.as_ptr(), buffer.len()) as isize }
}

#[cfg(test)]
fn syscall_write(_fd: usize, _buffer: &[u8]) -> isize {
    0
}

pub fn syscall_exit(code: i32) -> ! {
    unsafe {
        core::arch::asm!("syscall", in("rax") 0usize, in("rdi") code as usize,
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
