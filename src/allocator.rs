use core::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;
use spin::Mutex;

const HEAP_SIZE: usize = 1024 * 1024; // 1 MB heap

#[repr(C, align(4096))]
struct HeapMemory {
    data: [u8; HEAP_SIZE],
}

static mut HEAP_MEM: HeapMemory = HeapMemory { data: [0; HEAP_SIZE] };

pub struct SimpleAllocator {
    offset: Mutex<usize>,
}

#[derive(Debug, Clone, Copy)]
pub struct HeapStats {
    pub used_bytes: usize,
    pub free_bytes: usize,
    pub total_bytes: usize,
    pub used_percent: usize,
}

unsafe impl GlobalAlloc for SimpleAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut offset = self.offset.lock();
        let heap_base = core::ptr::addr_of_mut!(HEAP_MEM.data) as *mut u8 as usize;
        let alloc_start = heap_base + *offset;
        
        // Align up
        let align = layout.align();
        let remainder = alloc_start % align;
        let start_addr = if remainder == 0 {
            alloc_start
        } else {
            alloc_start + align - remainder
        };
        
        let alloc_end = start_addr.checked_add(layout.size());
        
        match alloc_end {
            Some(end) => {
                let heap_end = heap_base + HEAP_SIZE;
                if end > heap_end {
                    null_mut() // Out of heap memory!
                } else {
                    let new_offset = end - heap_base;
                    *offset = new_offset;
                    start_addr as *mut u8
                }
            }
            None => null_mut()
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // Our simple bump allocator doesn't reuse memory.
        // It simply grows until out of memory. 
        // For a true OS heap, a Linked List allocator is required later!
    }
}

#[global_allocator]
pub static ALLOCATOR: SimpleAllocator = SimpleAllocator {
    offset: Mutex::new(0),
};

pub fn heap_stats() -> HeapStats {
    let used = *ALLOCATOR.offset.lock();
    let clamped_used = core::cmp::min(used, HEAP_SIZE);
    let free = HEAP_SIZE.saturating_sub(clamped_used);
    let used_percent = if HEAP_SIZE == 0 {
        0
    } else {
        (clamped_used.saturating_mul(100)) / HEAP_SIZE
    };

    HeapStats {
        used_bytes: clamped_used,
        free_bytes: free,
        total_bytes: HEAP_SIZE,
        used_percent,
    }
}
