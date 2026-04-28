#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]

extern crate alloc;

use core::panic::PanicInfo;
use core::arch::global_asm;

mod vga_buffer;
mod serial;
mod idt;
mod interrupts;
mod pic;
mod pit;
mod memory;
mod paging;
mod allocator;
mod fs;
mod shell;
mod task;
mod gdt;
mod syscall;

// The raw boot sequence. Sets up a strict 16KB stack, pushes the multiboot registers to the C-stack, and jumps to rust.
global_asm!(r#"
.section .text
.global _start
_start:
    mov esp, offset kernel_stack_top
    push ebx
    push eax
    call kernel_main
    cli
1:  hlt
    jmp 1b

.section .bss
.align 16
.global kernel_stack_bottom
kernel_stack_bottom:
.skip 16384 # 16KB boot stack
.global kernel_stack_top
kernel_stack_top:
"#);

#[repr(C, packed)]
pub struct MultibootHeader {
    magic: u32,
    flags: u32,
    checksum: u32,
}

#[used]
#[no_mangle]
#[link_section = ".multiboot"]
pub static MULTIBOOT_HEADER: MultibootHeader = {
    let magic: u32 = 0x1BADB002;
    let flags: u32 = 0x0;
    MultibootHeader {
        magic,
        flags,
        checksum: (0_u32.wrapping_sub(magic).wrapping_sub(flags)),
    }
};

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    serial_println!("{}", info);
    loop {}
}

#[alloc_error_handler]
fn alloc_error_handler(layout: core::alloc::Layout) -> ! {
    panic!("Out of Memory Exception: {:?}", layout)
}

fn background_thread() {
    let mut heartbeat: u32 = 0;
    loop {
        heartbeat = heartbeat.wrapping_add(1);

        // Log occasionally without burning CPU on a busy loop.
        if heartbeat % 400 == 0 {
            serial_println!("RING 0: Background Thread Alive!");
        }

        unsafe {
            core::arch::asm!("hlt", options(nomem, nostack, preserves_flags));
        }
    }
}

const USER_CODE_ADDR: u32 = 0x0040_0000;
const USER_MSG_ADDR: u32 = 0x0040_0100;
const USER_STACK_TOP: u32 = 0x0080_0000;

fn write_u32_le(buf: &mut [u8], offset: usize, value: u32) {
    buf[offset] = (value & 0xFF) as u8;
    buf[offset + 1] = ((value >> 8) & 0xFF) as u8;
    buf[offset + 2] = ((value >> 16) & 0xFF) as u8;
    buf[offset + 3] = ((value >> 24) & 0xFF) as u8;
}

fn write_i32_le(buf: &mut [u8], offset: usize, value: i32) {
    write_u32_le(buf, offset, value as u32);
}

fn emit_mov_imm32(code: &mut [u8], cursor: &mut usize, opcode: u8, imm: u32) {
    code[*cursor] = opcode;
    *cursor += 1;
    write_u32_le(code, *cursor, imm);
    *cursor += 4;
}

fn install_user_program() -> u32 {
    let msg = b"Ring 3 online via isolated user pages + sys_sleep.";

    unsafe {
        core::ptr::write_bytes(USER_CODE_ADDR as *mut u8, 0, 512);
        core::ptr::write_bytes(USER_MSG_ADDR as *mut u8, 0, 128);
        core::ptr::copy_nonoverlapping(msg.as_ptr(), USER_MSG_ADDR as *mut u8, msg.len());
    }

    let mut code = [0u8; 64];
    let mut i = 0usize;

    // SYS_PRINT(ptr=USER_MSG_ADDR, len=msg.len())
    emit_mov_imm32(&mut code, &mut i, 0xB8, 1); // mov eax, 1
    emit_mov_imm32(&mut code, &mut i, 0xBB, USER_MSG_ADDR); // mov ebx, msg_ptr
    emit_mov_imm32(&mut code, &mut i, 0xB9, msg.len() as u32); // mov ecx, msg_len
    code[i] = 0xCD; // int 0x80
    code[i + 1] = 0x80;
    i += 2;

    let loop_start = i;

    // SYS_SLEEP(ticks=20)
    emit_mov_imm32(&mut code, &mut i, 0xB8, 3); // mov eax, 3
    emit_mov_imm32(&mut code, &mut i, 0xBB, 20); // mov ebx, 20
    code[i] = 0xCD; // int 0x80
    code[i + 1] = 0x80;
    i += 2;

    // jmp loop_start
    code[i] = 0xE9;
    let next_ip = USER_CODE_ADDR as i64 + i as i64 + 5;
    let target_ip = USER_CODE_ADDR as i64 + loop_start as i64;
    let rel = (target_ip - next_ip) as i32;
    write_i32_le(&mut code, i + 1, rel);
    i += 5;

    unsafe {
        core::ptr::copy_nonoverlapping(code.as_ptr(), USER_CODE_ADDR as *mut u8, i);
    }

    USER_CODE_ADDR
}

global_asm!(r#"
.section .text
.global switch_to_user_mode
switch_to_user_mode:
    cli
    mov edx, [esp + 4]   # user_entry
    mov ecx, [esp + 8]   # user_stack_top

    mov ax, 0x23
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax

    push 0x23
    push ecx
    push 0x202     # EFLAGS with IOPL = 0 (bits 12-13) + Interrupts Enabled (bit 9)
    push 0x1B
    push edx
    
    iretd
"#);

extern "C" {
    fn switch_to_user_mode(user_entry: u32, user_stack_top: u32);
}

#[no_mangle]
pub extern "C" fn kernel_main(magic: u32, info_addr: u32) -> ! {
    if magic != 0x2BADB002 {
        panic!("Invalid multiboot magic: {:#x}", magic);
    }

    crate::println!("Booting ENOS...");
    memory::print_memory_map(info_addr);

    let mut phys_allocator = memory::BumpAllocator::new(info_addr);
    if let Some(frame) = phys_allocator.allocate_frame() {
        crate::serial_println!(
            "Kernel Boot: Allocator Ready. First frame at {:#x}.",
            frame.start_address
        );
    } else {
        crate::serial_println!("Kernel Boot: Allocator Ready but no frame available.");
    }
    
    // 1. Establish Ring Security Descriptors and Task State Segment
    gdt::init();
    crate::serial_println!("Kernel Boot: GDT & TSS Ready.");
    
    // 2. Wrap Memory Virtualization
    paging::init();
    crate::serial_println!("Kernel Boot: Paging Ready.");
    
    // 3. Connect Exception Gates
    interrupts::init_idt();
    pic::PICS.lock().initialize();
    pit::init_default();
    // IMPORTANT: set_interrupt_stack() must be called AFTER init_idt() so the
    // TSS esp0 stack is allocated after the IDT in the heap, preventing collision.
    gdt::set_interrupt_stack();
    crate::serial_println!("Kernel Boot: Interrupts Ready.");

    shell::init();

    // 4. Wire Multitasking Scheduler
    task::SCHEDULER
        .lock()
        .register_named_task("background_thread", background_thread);
    crate::serial_println!("Kernel Boot: Multitasking Ready. Initiating Ring 3 Jump...");

    // 5. Jump natively into Unprivileged Ring 3 execution!
    let user_entry = install_user_program();
    crate::serial_println!("Kernel Boot: User program installed at {:#x}.", user_entry);
    unsafe {
        switch_to_user_mode(user_entry, USER_STACK_TOP);
    }

    // We will never reach here!
    loop {}
}
