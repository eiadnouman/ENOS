use core::fmt;
use spin::Mutex;
use lazy_static::lazy_static;

pub struct SerialPort {
    port: u16,
}

impl SerialPort {
    pub const unsafe fn new(port: u16) -> Self {
        Self { port }
    }

    pub fn init(&mut self) {
        unsafe {
            self.outb(1, 0x00);    // Disable all interrupts
            self.outb(3, 0x80);    // Enable DLAB (set baud rate divisor)
            self.outb(0, 0x03);    // Set divisor to 3 (lo byte) 38400 baud
            self.outb(1, 0x00);    //                  (hi byte)
            self.outb(3, 0x03);    // 8 bits, no parity, one stop bit
            self.outb(2, 0xC7);    // Enable FIFO, clear them, with 14-byte threshold
            self.outb(4, 0x0B);    // IRQs enabled, RTS/DSR set
        }
    }

    unsafe fn outb(&mut self, offset: u16, value: u8) {
        core::arch::asm!(
            "out dx, al",
            in("dx") self.port + offset,
            in("al") value,
            options(nomem, nostack, preserves_flags)
        );
    }
    
    unsafe fn inb(&mut self, offset: u16) -> u8 {
        let mut ret: u8;
        core::arch::asm!(
            "in al, dx",
            out("al") ret,
            in("dx") self.port + offset,
            options(nomem, nostack, preserves_flags)
        );
        ret
    }
    
    fn is_transmit_empty(&mut self) -> bool {
        unsafe { self.inb(5) & 0x20 != 0 }
    }
    
    pub fn write_byte(&mut self, byte: u8) {
        while !self.is_transmit_empty() {
            core::hint::spin_loop();
        }
        unsafe {
            self.outb(0, byte);
        }
    }
}

impl fmt::Write for SerialPort {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            match byte {
                b'\n' => {
                    self.write_byte(b'\r');
                    self.write_byte(b'\n');
                }
                _ => self.write_byte(byte),
            }
        }
        Ok(())
    }
}

lazy_static! {
    pub static ref SERIAL1: Mutex<SerialPort> = {
        let mut serial_port = unsafe { SerialPort::new(0x3F8) };
        serial_port.init();
        Mutex::new(serial_port)
    };
}

#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;
    SERIAL1.lock().write_fmt(args).expect("Printing to serial failed");
}

#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::serial::_print(format_args!($($arg)*));
    };
}

#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($fmt:expr) => ($crate::serial_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serial_print!(
        concat!($fmt, "\n"), $($arg)*));
}
