use core::arch::asm;
use core::mem::size_of;

#[derive(Clone, Copy)]
#[repr(C, packed)]
pub struct IdtEntry {
    offset_low: u16,
    selector: u16,
    zero: u8,
    type_attr: u8,
    offset_high: u16,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct InterruptStackFrame {
    pub instruction_pointer: u32,
    pub code_segment: u32,
    pub cpu_flags: u32,
    pub stack_pointer: u32,
    pub stack_segment: u32,
}

pub type HandlerFunc = extern "x86-interrupt" fn(&mut InterruptStackFrame);

impl IdtEntry {
    pub const fn missing() -> Self {
        IdtEntry {
            offset_low: 0,
            selector: 0,
            zero: 0,
            type_attr: 0,
            offset_high: 0,
        }
    }

    pub fn set_handler_fn(&mut self, handler: HandlerFunc) {
        self.set_handler_raw(handler as u32);
    }

    pub fn set_handler_raw(&mut self, pointer: u32) {
        self.offset_low = pointer as u16;
        
        let mut cs: u16;
        unsafe { core::arch::asm!("mov {0:x}, cs", out(reg) cs) };
        self.selector = cs;
        
        self.zero = 0;
        self.type_attr = 0x8E; // Present (1) | DPL (00) | Storage (0) | 32-bit Interrupt Gate (1110)
        self.offset_high = (pointer >> 16) as u16;
    }
}

pub struct InterruptDescriptorTable {
    entries: [IdtEntry; 256],
}

impl InterruptDescriptorTable {
    pub const fn new() -> Self {
        InterruptDescriptorTable {
            entries: [IdtEntry::missing(); 256],
        }
    }

    pub fn set_handler(&mut self, index: usize, handler: HandlerFunc) {
        self.entries[index].set_handler_fn(handler);
    }

    pub fn set_handler_ptr(&mut self, index: usize, pointer: u32) {
        self.entries[index].set_handler_raw(pointer);
    }

    pub fn load(&'static self) {
        let ptr = IdtPtr {
            limit: (size_of::<InterruptDescriptorTable>() - 1) as u16,
            base: self as *const _ as u32,
        };

        unsafe {
            asm!("lidt [{}]", in(reg) &ptr, options(readonly, nostack, preserves_flags));
        }
    }
}

#[repr(C, packed)]
struct IdtPtr {
    limit: u16,
    base: u32,
}
