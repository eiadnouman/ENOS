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
    next_free_frame: u32,
    current_mmap_offset: u32,
}

impl BumpAllocator {
    pub fn new(info_addr: u32) -> Self {
        let info = unsafe { &*(info_addr as *const MultibootInfo) };
        let mut allocator = BumpAllocator {
            memory_map_addr: 0,
            memory_map_length: 0,
            next_free_frame: 0,
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

    pub fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let mmap_end = self.memory_map_addr + self.memory_map_length;

        while self.current_mmap_offset < self.memory_map_length {
            let mmap = self.memory_map_addr + self.current_mmap_offset;
            let entry = unsafe { &*(mmap as *const MemoryMapEntry) };

            if !entry.is_available() {
                self.current_mmap_offset += entry.size + 4;
                self.next_free_frame = 0;
                continue;
            }

            // Init next_free_frame for this region if it's 0
            if self.next_free_frame == 0 {
                // Find a proper page-aligned starting address
                let base = entry.base_addr_low;
                let remainder = base % PAGE_SIZE;
                self.next_free_frame = if remainder == 0 { base } else { base - remainder + PAGE_SIZE };
            }

            // Check if we hit the kernel bounds
            let kernel_start = Self::kernel_start();
            let kernel_end = Self::kernel_end();

            // Align kernel end to next page boundary
            let kernel_end_aligned = if kernel_end % PAGE_SIZE == 0 {
                kernel_end
            } else {
                kernel_end - (kernel_end % PAGE_SIZE) + PAGE_SIZE
            };

            // If the next_free_frame is overlapping the kernel, jump over the kernel!
            if self.next_free_frame >= kernel_start && self.next_free_frame < kernel_end_aligned {
                self.next_free_frame = kernel_end_aligned;
            }

            let frame_end = self.next_free_frame + PAGE_SIZE;
            
            // Check if this region actually has enough space left for the frame
            let region_end = entry.base_addr_low + entry.length_low;
            if frame_end <= region_end {
                // Beautiful! We have space!
                let frame = PhysFrame {
                    start_address: self.next_free_frame,
                };
                self.next_free_frame += PAGE_SIZE;
                return Some(frame);
            } else {
                // Not enough space in this region. Move to the next region.
                self.current_mmap_offset += entry.size + 4;
                self.next_free_frame = 0;
            }
        }
        None // Out of memory!
    }
}

