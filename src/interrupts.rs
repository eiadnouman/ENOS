use crate::idt::InterruptDescriptorTable;
use lazy_static::lazy_static;

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.set_handler(3, breakpoint_handler);
        idt.set_handler_ptr(32, timer_interrupt_wrapper as usize as u32);
        idt.set_handler(33, keyboard_interrupt_handler);
        idt
    };
}

pub fn init_idt() {
    IDT.load();
}

extern "x86-interrupt" fn breakpoint_handler(frame: &mut crate::idt::InterruptStackFrame) {
    crate::println!("EXCEPTION: BREAKPOINT\n{:#?}", frame);
    crate::serial_println!("EXCEPTION: BREAKPOINT\n{:#?}", frame);
}

use core::arch::global_asm;

global_asm!(r#"
.global timer_interrupt_wrapper
timer_interrupt_wrapper:
    pushad
    push esp
    call scheduler_tick
    add esp, 4
    mov esp, eax

    mov al, 0x20
    out 0x20, al

    popad
    iretd
"#);

extern "C" {
    fn timer_interrupt_wrapper();
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_frame: &mut crate::idt::InterruptStackFrame) {
    let mut port = 0x60;
    let mut scancode: u8;
    unsafe {
        core::arch::asm!(
            "in al, dx",
            out("al") scancode,
            in("dx") port,
            options(nomem, nostack, preserves_flags)
        );
    }
    
    crate::shell::process_scancode(scancode);


    crate::pic::ack(33);
}
