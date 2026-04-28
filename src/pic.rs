use spin::Mutex;
use lazy_static::lazy_static;

const PIC1_CMD: u16 = 0x20;
const PIC1_DATA: u16 = 0x21;
const PIC2_CMD: u16 = 0xA0;
const PIC2_DATA: u16 = 0xA1;

const PIC_EOI: u8 = 0x20;

unsafe fn outb(port: u16, value: u8) {
    core::arch::asm!(
        "out dx, al",
        in("dx") port,
        in("al") value,
        options(nomem, nostack, preserves_flags)
    );
}

unsafe fn inb(port: u16) -> u8 {
    let mut ret: u8;
    core::arch::asm!(
        "in al, dx",
        out("al") ret,
        in("dx") port,
        options(nomem, nostack, preserves_flags)
    );
    ret
}

unsafe fn iowait() {
    outb(0x80, 0);
}

pub struct ChainedPics {
    offset1: u8,
    offset2: u8,
}

impl ChainedPics {
    pub const fn new(offset1: u8, offset2: u8) -> Self {
        ChainedPics { offset1, offset2 }
    }

    pub fn initialize(&mut self) {
        unsafe {
            let a1 = inb(PIC1_DATA);
            let a2 = inb(PIC2_DATA);

            outb(PIC1_CMD, 0x11); // start init sequence
            iowait();
            outb(PIC2_CMD, 0x11);
            iowait();

            outb(PIC1_DATA, self.offset1); // ICW2: Vector offset
            iowait();
            outb(PIC2_DATA, self.offset2);
            iowait();

            outb(PIC1_DATA, 4); // ICW3: tell Primary there is a slave PIC at IRQ2
            iowait();
            outb(PIC2_DATA, 2); // ICW3: tell Secondary its cascade identity
            iowait();

            outb(PIC1_DATA, 0x01); // ICW4: 8086 mode
            iowait();
            outb(PIC2_DATA, 0x01);
            iowait();

            outb(PIC1_DATA, 0x00); // Unmask all
            outb(PIC2_DATA, 0x00);
        }
    }

    pub fn handles_interrupt(&self, interrupt_id: u8) -> bool {
        self.offset1 <= interrupt_id && interrupt_id < self.offset2 + 8
    }

    pub fn notify_end_of_interrupt(&mut self, interrupt_id: u8) {
        if self.handles_interrupt(interrupt_id) {
            unsafe {
                if interrupt_id >= self.offset2 {
                    outb(PIC2_CMD, PIC_EOI);
                }
                outb(PIC1_CMD, PIC_EOI);
            }
        }
    }
}

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

lazy_static! {
    pub static ref PICS: Mutex<ChainedPics> =
        Mutex::new(ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET));
}

pub fn ack(irq_idx: u8) {
    PICS.lock().notify_end_of_interrupt(irq_idx);
}
