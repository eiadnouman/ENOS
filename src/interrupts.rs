use crate::idt::InterruptDescriptorTable;
use alloc::boxed::Box;

pub fn init_idt() {
    let cs: u16;
    unsafe { core::arch::asm!("mov {0:x}, cs", out(reg) cs); }
    crate::serial_println!("IDT init: CS = {:#x}", cs);

    // Heap-allocate the IDT so it doesn't conflict with BSS symbols like kernel_stack_top.
    // lazy_static! uses a spin::Once in BSS which was ending up at the same address as
    // our kernel stack top, causing the CPU to corrupt the IDT on every privilege transition.
    let idt = Box::leak(Box::new({
        let mut idt = InterruptDescriptorTable::new();
        idt.set_handler(3, breakpoint_handler);
        idt.set_handler_ptr(32, timer_interrupt_wrapper as *const () as usize as u32);
        idt.set_handler(33, keyboard_interrupt_handler);
        idt.set_handler_ptr(14, page_fault_wrapper as *const () as usize as u32);
        idt.set_handler_user(0x80, syscall_wrapper as *const () as usize as u32);
        idt.set_handler_with_code(13, gpf_handler);
        idt.set_handler_with_code(8, double_fault_handler);
        idt
    }));
    idt.load();
}

extern "x86-interrupt" fn breakpoint_handler(frame: &mut crate::idt::InterruptStackFrame) {
    crate::serial_println!("EXCEPTION: BREAKPOINT\n{:#?}", frame);
}

extern "x86-interrupt" fn gpf_handler(frame: &mut crate::idt::InterruptStackFrame, error_code: u32) {
    crate::serial_println!("EXCEPTION: General Protection Fault! ErrCode={:#x}\n{:#?}", error_code, frame);
    loop {}
}

extern "x86-interrupt" fn double_fault_handler(frame: &mut crate::idt::InterruptStackFrame, error_code: u32) {
    crate::serial_println!("EXCEPTION: Double Fault! ErrCode={:#x}\n{:#?}", error_code, frame);
    loop {}
}

#[no_mangle]
pub extern "C" fn page_fault_dispatcher(current_esp: u32, fault_addr: u32, error_code: u32) -> u32 {
    let user_fault = (error_code & 0x4) != 0;
    let write_fault = (error_code & 0x2) != 0;
    let protection_violation = (error_code & 0x1) != 0;

    if user_fault {
        crate::serial_println!(
            "EXCEPTION: USER PAGE FAULT addr={:#x} err={:#x} kind={} access={}",
            fault_addr,
            error_code,
            if protection_violation { "protection" } else { "not-present" },
            if write_fault { "write" } else { "read" },
        );
        return crate::task::terminate_current_user_task_from_fault(current_esp);
    }

    panic!(
        "KERNEL PAGE FAULT addr={:#x} err={:#x} kind={} access={}",
        fault_addr,
        error_code,
        if protection_violation { "protection" } else { "not-present" },
        if write_fault { "write" } else { "read" },
    );
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

    popad
    iretd

.global page_fault_wrapper
page_fault_wrapper:
    pushad
    mov ebx, esp
    mov eax, cr2
    mov ecx, [ebx + 32]
    push ecx
    push eax
    push ebx
    call page_fault_dispatcher
    add esp, 12
    mov esp, eax
    popad
    iretd

.global syscall_wrapper
syscall_wrapper:
    pushad
    mov ebp, esp

    # cdecl: push args right-to-left: fn(eax, ebx, ecx, edx, esi, edi, current_esp)
    push ebp
    push edi
    push esi
    push edx
    push ecx
    push ebx
    push eax

    call syscall_dispatcher

    # Cleanup 7 x 4 bytes = 28 bytes of args
    add esp, 28

    # Save return value (eax) into the pushad-saved eax slot
    # pushad layout (top of stack = lowest address): edi esi ebp esp ebx edx ecx eax
    # eax is at [esp+28]
    mov [esp + 28], eax

    popad
    iretd
"#);

extern "C" {
    fn timer_interrupt_wrapper();
    fn page_fault_wrapper();
    fn syscall_wrapper();
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_frame: &mut crate::idt::InterruptStackFrame) {
    let port: u16 = 0x60;
    let scancode: u8;
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
