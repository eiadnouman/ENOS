const PIT_CHANNEL0: u16 = 0x40;
const PIT_COMMAND: u16 = 0x43;
const PIT_INPUT_HZ: u32 = 1_193_182;
const DEFAULT_HZ: u32 = 100;
pub const TICKS_PER_SECOND: u64 = DEFAULT_HZ as u64;

unsafe fn outb(port: u16, value: u8) {
    core::arch::asm!(
        "out dx, al",
        in("dx") port,
        in("al") value,
        options(nomem, nostack, preserves_flags)
    );
}

pub fn init_default() {
    init(DEFAULT_HZ);
}

pub fn init(requested_hz: u32) {
    let hz = if requested_hz == 0 { DEFAULT_HZ } else { requested_hz };
    let divisor_u32 = (PIT_INPUT_HZ / hz).clamp(1, 65535);
    let divisor = divisor_u32 as u16;
    let actual_hz = PIT_INPUT_HZ / divisor_u32;

    unsafe {
        // Channel 0, lobyte/hibyte, mode 3 (square wave), binary counter
        outb(PIT_COMMAND, 0x36);
        outb(PIT_CHANNEL0, (divisor & 0x00FF) as u8);
        outb(PIT_CHANNEL0, (divisor >> 8) as u8);
    }

    crate::serial_println!(
        "PIT: configured to {} Hz (requested {} Hz, divisor {}).",
        actual_hz,
        hz,
        divisor
    );
}

pub fn ticks_to_millis(ticks: u64) -> u64 {
    ticks.saturating_mul(1000) / TICKS_PER_SECOND
}
