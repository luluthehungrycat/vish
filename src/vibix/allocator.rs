use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
use core::ptr;
use core::sync::atomic::{AtomicBool, Ordering};

const ALIGN: usize = 16;
const PAGE: usize = 4096;

#[repr(C, align(16))]
struct Block {
    span: usize,
    next: *mut Block,
    free: bool,
}

pub struct Heap {
    head: *mut Block,
}

impl Heap {
    pub const fn new() -> Self {
        Self { head: ptr::null_mut() }
    }

    pub unsafe fn add_region(&mut self, start: *mut u8, size: usize) {
        let aligned = align_up(start as usize, ALIGN) as *mut u8;
        let skipped = aligned as usize - start as usize;
        let size = (size - skipped) & !(ALIGN - 1);
        if size < core::mem::size_of::<Block>() + ALIGN { return; }
        let block = aligned.cast::<Block>();
        block.write(Block { span: size, next: ptr::null_mut(), free: true });
        if self.head.is_null() {
            self.head = block;
        } else {
            let mut tail = self.head;
            while !(*tail).next.is_null() { tail = (*tail).next; }
            (*tail).next = block;
            self.coalesce(tail);
        }
    }

    pub unsafe fn allocate(&mut self, layout: Layout) -> *mut u8 {
        let alignment = layout.align().max(ALIGN);
        let mut block = self.head;
        while !block.is_null() {
            if (*block).free {
                let base = block as usize;
                let payload = align_up(base + core::mem::size_of::<Block>() + core::mem::size_of::<usize>(), alignment);
                let required = align_up(payload.saturating_add(layout.size()).saturating_sub(base), ALIGN);
                if required <= (*block).span {
                    let remaining = (*block).span - required;
                    if remaining >= core::mem::size_of::<Block>() + ALIGN {
                        let tail = (base + required) as *mut Block;
                        tail.write(Block { span: remaining, next: (*block).next, free: true });
                        (*block).span = required;
                        (*block).next = tail;
                    }
                    (*block).free = false;
                    ((payload - core::mem::size_of::<usize>()) as *mut usize).write(block as usize);
                    return payload as *mut u8;
                }
            }
            block = (*block).next;
        }
        ptr::null_mut()
    }

    pub unsafe fn deallocate(&mut self, pointer: *mut u8) {
        if pointer.is_null() { return; }
        let block = ((pointer as *mut usize).sub(1).read()) as *mut Block;
        (*block).free = true;
        self.coalesce(block);
        let mut previous: *mut Block = ptr::null_mut();
        let mut current = self.head;
        while current != block && !current.is_null() {
            previous = current;
            current = (*current).next;
        }
        if !previous.is_null() { self.coalesce(previous); }
    }

    unsafe fn coalesce(&mut self, block: *mut Block) {
        if block.is_null() || !(*block).free { return; }
        let next = (*block).next;
        if !next.is_null() && (*next).free && (block as usize + (*block).span) == next as usize {
            (*block).span += (*next).span;
            (*block).next = (*next).next;
        }
    }
}

const fn align_up(value: usize, alignment: usize) -> usize {
    (value + alignment - 1) & !(alignment - 1)
}

struct SharedHeap(UnsafeCell<Heap>);
unsafe impl Sync for SharedHeap {}

pub struct BrkAllocator;
static HEAP: SharedHeap = SharedHeap(UnsafeCell::new(Heap::new()));
static LOCK: AtomicBool = AtomicBool::new(false);

struct Guard;
impl Guard {
    fn lock() -> Self {
        while LOCK.compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed).is_err() {
            core::hint::spin_loop();
        }
        Self
    }
}
impl Drop for Guard {
    fn drop(&mut self) { LOCK.store(false, Ordering::Release); }
}

unsafe fn syscall_brk(address: usize) -> usize {
    let mut result = 4usize;
    core::arch::asm!("syscall", inlateout("rax") result, inlateout("rdi") address => _,
        lateout("rsi") _, lateout("rdx") _, lateout("r8") _,
        lateout("r9") _, lateout("r10") _,
        lateout("rcx") _, lateout("r11") _, options(nostack));
    result
}

unsafe fn grow(heap: &mut Heap, layout: Layout) -> bool {
    let current = syscall_brk(0);
    let payload_need = layout.size().saturating_add(layout.align()).saturating_add(core::mem::size_of::<Block>() + core::mem::size_of::<usize>());
    let bytes = align_up(payload_need.max(PAGE), PAGE);
    let Some(new_break) = current.checked_add(bytes) else { return false; };
    let actual = syscall_brk(new_break);
    if actual != new_break {
        return false;
    }
    heap.add_region(current as *mut u8, bytes);
    true
}

unsafe impl GlobalAlloc for BrkAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _guard = Guard::lock();
        let heap = &mut *HEAP.0.get();
        let first = heap.allocate(layout);
        if !first.is_null() { return first; }
        if !grow(heap, layout) { return ptr::null_mut(); }
        heap.allocate(layout)
    }

    unsafe fn dealloc(&self, pointer: *mut u8, _layout: Layout) {
        let _guard = Guard::lock();
        (&mut *HEAP.0.get()).deallocate(pointer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[repr(align(4096))]
    struct Region([u8; 16 * 1024]);

    #[test]
    fn aligned_allocations_honor_requested_alignment() {
        let mut region = Region([0; 16 * 1024]);
        let mut heap = Heap::new();
        unsafe {
            heap.add_region(region.0.as_mut_ptr(), region.0.len());
            let value = heap.allocate(Layout::from_size_align(17, 64).unwrap());
            assert!(!value.is_null());
            assert_eq!(value as usize % 64, 0);
        }
    }

    #[test]
    fn deallocation_reuses_freed_block() {
        let mut region = Region([0; 16 * 1024]);
        let mut heap = Heap::new();
        unsafe {
            heap.add_region(region.0.as_mut_ptr(), region.0.len());
            let layout = Layout::from_size_align(64, 16).unwrap();
            let first = heap.allocate(layout);
            heap.deallocate(first);
            assert_eq!(heap.allocate(layout), first);
        }
    }

    #[test]
    fn adjacent_free_blocks_coalesce() {
        let mut region = Region([0; 16 * 1024]);
        let mut heap = Heap::new();
        unsafe {
            heap.add_region(region.0.as_mut_ptr(), region.0.len());
            let small = Layout::from_size_align(256, 16).unwrap();
            let first = heap.allocate(small);
            let second = heap.allocate(small);
            heap.deallocate(first);
            heap.deallocate(second);
            let large = heap.allocate(Layout::from_size_align(600, 16).unwrap());
            assert!(!large.is_null());
        }
    }

    #[test]
    fn allocation_fails_without_a_growable_region() {
        let mut heap = Heap::new();
        unsafe { assert!(heap.allocate(Layout::from_size_align(64, 16).unwrap()).is_null()); }
    }
}
