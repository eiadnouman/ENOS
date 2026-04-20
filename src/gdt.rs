use core::arch::asm;
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

pub fn init() {
    unsafe {
        // Allocate dynamically on the Global Kernel Heap to bypass static BSS compiler limits!
        let mut tss_box = Box::new(TaskStateSegment::new());
        let mut gdt_box = Box::new(GlobalDescriptorTable {
            entries: [GdtEntry::empty(); 6]
        });

        gdt_box.entries[1] = GdtEntry::new(0, 0xFFFFF, 0x9A, 0xCF); // Kernel Code 0x08
        gdt_box.entries[2] = GdtEntry::new(0, 0xFFFFF, 0x92, 0xCF); // Kernel Data 0x10
        gdt_box.entries[3] = GdtEntry::new(0, 0xFFFFF, 0xFA, 0xCF); // User Code 0x1B
        gdt_box.entries[4] = GdtEntry::new(0, 0xFFFFF, 0xF2, 0xCF); // User Data 0x23

        // Prepare the TSS Descriptor
        let tss_ref = Box::leak(tss_box);
        let gdt_ref = Box::leak(gdt_box);

        let tss_base = tss_ref as *const _ as u32;
        let tss_limit = size_of::<TaskStateSegment>() as u32 - 1;
        gdt_ref.entries[5] = GdtEntry::new(tss_base, tss_limit, 0x89, 0x40);

        // Set TSS.esp0 to the top of our pre-allocated kernel stack
        // Extern link to the assembly stack top
        extern "C" {
            static kernel_stack_top: u8;
        }
        tss_ref.esp0 = &raw const kernel_stack_top as usize as u32;

        let gdt_ptr = GdtPtr {
            limit: (size_of::<GlobalDescriptorTable>() - 1) as u16,
            base: gdt_ref as *const _ as u32,
        };

        // Load GDT
        core::arch::asm!("lgdt [{}]", in(reg) &gdt_ptr, options(readonly, nostack, preserves_flags));

        // Flush Segments (Kernel Code = 0x08, Kernel Data = 0x10)
        // Note: Assuming CS is already appropriately set by MultiBoot/GRUB
        core::arch::asm!(
            "mov ax, 0x10",
            "mov ds, ax",
            "mov es, ax",
            "mov fs, ax",
            "mov gs, ax",
            "mov ss, ax",
            options(nostack, preserves_flags)
        );

        // Load TSS (Offset 0x28)
        core::arch::asm!("ltr ax", in("ax") 0x28u16, options(nostack, preserves_flags));
    }
}
