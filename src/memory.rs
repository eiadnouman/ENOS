#[repr(C)]
#[derive(Debug)]
pub struct MultibootInfo {
    pub flags: u32,
    pub mem_lower: u32,
    pub mem_upper: u32,
    pub boot_device: u32,
    pub cmdline: u32,
    pub mods_count: u32,
    pub mods_addr: u32,
    pub syms: [u32; 4],
    pub mmap_length: u32,
    pub mmap_addr: u32,
    pub drives_length: u32,
    pub drives_addr: u32,
    pub config_table: u32,
    pub boot_loader_name: u32,
    pub apm_table: u32,
    pub vbe_control_info: u32,
    pub vbe_mode_info: u32,
    pub vbe_mode: u16,
    pub vbe_interface_seg: u16,
    pub vbe_interface_off: u16,
    pub vbe_interface_len: u16,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct MemoryMapEntry {
    pub size: u32,
    pub base_addr_low: u32,
    pub base_addr_high: u32,
    pub length_low: u32,
    pub length_high: u32,
    pub buffer_type: u32,
}

impl MemoryMapEntry {
    pub fn is_available(&self) -> bool {
        self.buffer_type == 1
    }
}

pub fn print_memory_map(info_addr: u32) {
    let info = unsafe { &*(info_addr as *const MultibootInfo) };
    
    // Check if the memory map flag is set (bit 6)
    if (info.flags & (1 << 6)) != 0 {
        crate::serial_println!("Multiboot Memory Map detected!");
        
        let mut mmap = info.mmap_addr;
        let mmap_end = info.mmap_addr + info.mmap_length;
        
        let mut total_memory_kb = 0;

        while mmap < mmap_end {
            let entry = unsafe { &*(mmap as *const MemoryMapEntry) };
            
            let base = entry.base_addr_low;
            let p_length = entry.length_low;
            let btype = entry.buffer_type;
            
            crate::serial_println!(
                "Base: {:#010X}, Length: {:#010X}, Type: {}",
                base,
                p_length,
                btype
            );
            
            if entry.is_available() {
                total_memory_kb += p_length / 1024;
            }
            
            // Increment by the size of the entry + 4 (because `size` doesn't include itself)
            mmap += entry.size + 4;
        }
        
        crate::println!("Total Available RAM: {} MB", total_memory_kb / 1024);
        crate::serial_println!("Total Available RAM: {} MB", total_memory_kb / 1024);
    } else {
        crate::println!("Warning: No memory map provided by Grub.");
        crate::serial_println!("Warning: No memory map provided by Grub.");
    }
}

pub const PAGE_SIZE: u32 = 4096;
pub const LOW_MEMORY_RESERVED_END: u32 = 0x0010_0000;

#[derive(Debug, Clone, Copy)]
pub struct PhysFrame {
    pub start_address: u32,
}

extern "C" {
    static _kernel_start: u8;
    static _kernel_end: u8;
}

pub struct BumpAllocator {
    memory_map_addr: u32,
    memory_map_length: u32,
    // Option<u32> so we can distinguish "not yet initialised for this
    // region" from the valid physical address 0x0.
    next_free_frame: Option<u32>,
    current_mmap_offset: u32,
}

impl BumpAllocator {
    pub fn new(info_addr: u32) -> Self {
        let info = unsafe { &*(info_addr as *const MultibootInfo) };
        let mut allocator = BumpAllocator {
            memory_map_addr: 0,
            memory_map_length: 0,
            next_free_frame: None,
            current_mmap_offset: 0,
        };

        if (info.flags & (1 << 6)) != 0 {
            allocator.memory_map_addr = info.mmap_addr;
            allocator.memory_map_length = info.mmap_length;
        }
        
        allocator
    }

    fn kernel_start() -> u32 {
        unsafe { &_kernel_start as *const _ as u32 }
    }

    fn kernel_end() -> u32 {
        unsafe { &_kernel_end as *const _ as u32 }
    }

    fn align_up(value: u64, align: u64) -> u64 {
        let remainder = value % align;
        if remainder == 0 {
            value
        } else {
            value + align - remainder
        }
    }

    pub fn allocate_frame(&mut self) -> Option<PhysFrame> {
        while self.current_mmap_offset < self.memory_map_length {
            let mmap = self.memory_map_addr + self.current_mmap_offset;
            let entry = unsafe { &*(mmap as *const MemoryMapEntry) };

            if !entry.is_available() {
                self.current_mmap_offset += entry.size + 4;
                self.next_free_frame = None;
                continue;
            }

            let region_start =
                (entry.base_addr_low as u64) | ((entry.base_addr_high as u64) << 32);
            let region_length = (entry.length_low as u64) | ((entry.length_high as u64) << 32);
            let region_end = region_start.saturating_add(region_length);
            let allocation_floor =
                core::cmp::max(region_start, LOW_MEMORY_RESERVED_END as u64);

            if allocation_floor >= region_end || allocation_floor > u32::MAX as u64 {
                self.current_mmap_offset += entry.size + 4;
                self.next_free_frame = None;
                continue;
            }

            // Initialise next_free_frame for this region on first visit.
            if self.next_free_frame.is_none() {
                self.next_free_frame = Some(
                    Self::align_up(allocation_floor, PAGE_SIZE as u64) as u32,
                );
            }

            let mut candidate = self.next_free_frame.unwrap();

            // Skip over the kernel image if it sits in this region.
            let kernel_start = Self::kernel_start();
            let kernel_end   = Self::kernel_end();
            let kernel_end_aligned = if kernel_end % PAGE_SIZE == 0 {
                kernel_end
            } else {
                kernel_end - (kernel_end % PAGE_SIZE) + PAGE_SIZE
            };

            let candidate_end = candidate.saturating_add(PAGE_SIZE);
            if candidate < kernel_end_aligned && candidate_end > kernel_start {
                candidate = kernel_end_aligned;
                self.next_free_frame = Some(candidate);
            }

            let frame_end = candidate.saturating_add(PAGE_SIZE);

            if (frame_end as u64) <= region_end && frame_end > candidate {
                let frame = PhysFrame { start_address: candidate };
                self.next_free_frame = Some(candidate + PAGE_SIZE);
                return Some(frame);
            } else {
                // Region exhausted — move on.
                self.current_mmap_offset += entry.size + 4;
                self.next_free_frame = None;
            }
        }
        None // Out of memory!
    }
}
