use core::mem::size_of;
use alloc::boxed::Box;

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct GdtEntry {
    limit_low: u16,
    base_low: u16,
    base_middle: u8,
    access: u8,
    granularity: u8,
    base_high: u8,
}

impl GdtEntry {
    pub const fn empty() -> Self {
        GdtEntry {
            limit_low: 0,
            base_low: 0,
            base_middle: 0,
            access: 0,
            granularity: 0,
            base_high: 0,
        }
    }

    pub const fn new(base: u32, limit: u32, access: u8, gran: u8) -> Self {
        GdtEntry {
            limit_low: (limit & 0xFFFF) as u16,
            base_low: (base & 0xFFFF) as u16,
            base_middle: ((base >> 16) & 0xFF) as u8,
            access,
            granularity: ((limit >> 16) & 0x0F) as u8 | (gran & 0xF0),
            base_high: ((base >> 24) & 0xFF) as u8,
        }
    }
}

#[repr(C, packed)]
pub struct GdtPtr {
    limit: u16,
    base: u32,
}

#[repr(C, packed)]
pub struct TaskStateSegment {
    link: u16, res0: u16,
    pub esp0: u32,
    pub ss0: u16, res1: u16,
    esp1: u32, ss1: u16, res2: u16,
    esp2: u32, ss2: u16, res3: u16,
    cr3: u32, eip: u32, eflags: u32,
    eax: u32, ecx: u32, edx: u32, ebx: u32,
    esp: u32, ebp: u32, esi: u32, edi: u32,
    es: u16, res4: u16, cs: u16, res5: u16,
    ss: u16, res6: u16, ds: u16, res7: u16,
    fs: u16, res8: u16, gs: u16, res9: u16,
    ldtr: u16, res10: u16,
    iopb: u16, _pad: u16,
}

impl TaskStateSegment {
    pub fn new() -> Self {
        TaskStateSegment {
            link: 0, res0: 0,
            esp0: 0, ss0: 0x10, res1: 0, // SS0 = Kernel Data Segment
            esp1: 0, ss1: 0, res2: 0,
            esp2: 0, ss2: 0, res3: 0,
            cr3: 0, eip: 0, eflags: 0,
            eax: 0, ecx: 0, edx: 0, ebx: 0,
            esp: 0, ebp: 0, esi: 0, edi: 0,
            es: 0, res4: 0, cs: 0, res5: 0,
            ss: 0, res6: 0, ds: 0, res7: 0,
            fs: 0, res8: 0, gs: 0, res9: 0,
            ldtr: 0, res10: 0,
            iopb: 104, _pad: 0,
        }
    }
}

pub struct GlobalDescriptorTable {
    entries: [GdtEntry; 6],
}

// Global TSS pointer set during init — used by set_interrupt_stack() called after IDT init.
static mut TSS_PTR: *mut TaskStateSegment = core::ptr::null_mut();

pub fn init() {
    unsafe {
        // Allocate dynamically on the Global Kernel Heap to bypass static BSS compiler limits!
        let tss_box = Box::new(TaskStateSegment::new());
        let gdt_box = Box::new(GlobalDescriptorTable {
            entries: [GdtEntry::empty(); 6]
        });

        let tss_ref = Box::leak(tss_box);
        let gdt_ref = Box::leak(gdt_box);

        // Save global TSS pointer for later esp0 setup
        TSS_PTR = tss_ref as *mut TaskStateSegment;

        gdt_ref.entries[1] = GdtEntry::new(0, 0xFFFFF, 0x9A, 0xCF); // Kernel Code 0x08
        gdt_ref.entries[2] = GdtEntry::new(0, 0xFFFFF, 0x92, 0xCF); // Kernel Data 0x10
        gdt_ref.entries[3] = GdtEntry::new(0, 0xFFFFF, 0xFA, 0xCF); // User Code 0x1B
        gdt_ref.entries[4] = GdtEntry::new(0, 0xFFFFF, 0xF2, 0xCF); // User Data 0x23

        let tss_base = tss_ref as *const _ as u32;
        let tss_limit = size_of::<TaskStateSegment>() as u32 - 1;
        gdt_ref.entries[5] = GdtEntry::new(tss_base, tss_limit, 0x89, 0x40);

        // Debug: dump GDT[2] (Kernel Data 0x10) bytes
        let gdt2_ptr = &gdt_ref.entries[2] as *const GdtEntry as *const u8;
        let b = core::slice::from_raw_parts(gdt2_ptr, 8);
        crate::serial_println!("GDT[2] bytes: {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x} {:02x}",
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]);
        crate::serial_println!("GDT base: {:#x}, TSS base: {:#x}", gdt_ref as *const _ as u32, tss_base);

        let gdt_ptr = GdtPtr {
            limit: (size_of::<GlobalDescriptorTable>() - 1) as u16,
            base: gdt_ref as *const _ as u32,
        };

        // Load GDT
        core::arch::asm!("lgdt [{}]", in(reg) &gdt_ptr, options(readonly, nostack, preserves_flags));

        // Flush data segment registers to our new GDT (Kernel Data = 0x10)
        core::arch::asm!(
            "mov ax, 0x10",
            "mov ds, ax",
            "mov es, ax",
            "mov fs, ax",
            "mov gs, ax",
            "mov ss, ax",
            options(nostack, preserves_flags)
        );

        // Reload CS to 0x08 (Kernel Code) using a far return.
        // This flushes the hidden CS descriptor cache with our new GDT's entry.
        core::arch::asm!(
            "push 0x08",       // Push new CS selector
            "lea eax, [2f]",   // Push return address (next instruction after retf)
            "push eax",
            "retf",            // Far return: pops EIP then CS, reloading CS to 0x08
            "2:",
            options(nostack)
        );

        // Load TSS (Offset 0x28)
        core::arch::asm!("ltr ax", in("ax") 0x28u16, options(nostack, preserves_flags));
    }
}

/// Called AFTER the IDT is heap-allocated so the interrupt stack doesn't collide with it.
/// Sets TSS.esp0 to the top of a freshly allocated 8KB Ring 0 stack.
pub fn set_interrupt_stack() {
    unsafe {
        assert!(!TSS_PTR.is_null(), "gdt::init() must be called before set_interrupt_stack()");
        // Allocate the dedicated Ring 0 interrupt stack (8KB) AFTER IDT is allocated
        let int_stack: &'static mut [u8; 8192] = Box::leak(Box::new([0u8; 8192]));
        // Stack grows downward — esp0 points to the TOP (end of the slice)
        (*TSS_PTR).esp0 = (int_stack.as_ptr() as usize + 8192) as u32;
        let esp0 = (*TSS_PTR).esp0;
        crate::serial_println!("GDT: TSS.esp0 set to {:#x} (Ring 0 interrupt stack top)", esp0);
    }
}
