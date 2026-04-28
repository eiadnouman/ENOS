use core::arch::asm;

// Flags for Page Tables/Directories
const PAGE_PRESENT: u32 = 0x01;
const PAGE_RW: u32 = 0x02;
const PAGE_USER: u32 = 0x04;

#[repr(C, align(4096))]
pub struct PageDirectory {
    entries: [u32; 1024],
}

#[repr(C, align(4096))]
pub struct PageTable {
    entries: [u32; 1024],
}

static mut PAGE_DIRECTORY: PageDirectory = PageDirectory { entries: [0; 1024] };
static mut PAGE_TABLE_0: PageTable = PageTable { entries: [0; 1024] };
static mut PAGE_TABLE_1: PageTable = PageTable { entries: [0; 1024] };

pub fn init() {
    unsafe {
        // Prepare Page Table 0 (maps 0x0 to 0x3FFFFF - first 4MB)
        for i in 0..1024 {
            let phys_addr = (i * 4096) as u32;
            // Kernel space: supervisor-only mappings.
            PAGE_TABLE_0.entries[i] = phys_addr | PAGE_PRESENT | PAGE_RW;
        }

        // Prepare Page Table 1 (maps 0x400000 to 0x7FFFFF - second 4MB)
        for i in 0..1024 {
            let phys_addr = 0x400000 + (i * 4096) as u32;
            // User space: explicitly user-accessible.
            PAGE_TABLE_1.entries[i] = phys_addr | PAGE_PRESENT | PAGE_RW | PAGE_USER;
        }

        // Attach Page Tables to Page Directory
        let pt0_phys = (&raw const PAGE_TABLE_0 as *const _ as u32) - super::memory::PAGE_SIZE * 0; // It's all physical for now
        let pt1_phys = (&raw const PAGE_TABLE_1 as *const _ as u32) - super::memory::PAGE_SIZE * 0;

        PAGE_DIRECTORY.entries[0] = pt0_phys | PAGE_PRESENT | PAGE_RW;
        PAGE_DIRECTORY.entries[1] = pt1_phys | PAGE_PRESENT | PAGE_RW | PAGE_USER;

        // Load Page Directory to CR3
        let pd_addr = &raw const PAGE_DIRECTORY as *const _ as u32;
        asm!("mov cr3, {}", in(reg) pd_addr, options(nostack, preserves_flags));

        // Enable Paging in CR0 (Bit 31)
        let mut cr0: u32;
        asm!("mov {}, cr0", out(reg) cr0, options(nomem, nostack, preserves_flags));
        cr0 |= 0x80000000;
        asm!("mov cr0, {}", in(reg) cr0, options(nostack, preserves_flags));
    }
}
