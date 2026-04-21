#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]

extern crate alloc;

use core::panic::PanicInfo;
use core::arch::global_asm;
use alloc::vec::Vec;

mod vga_buffer;
mod serial;
mod idt;
mod interrupts;
mod pic;
mod memory;
mod paging;
mod allocator;
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
    loop {
        // Just delay
        for _ in 0..5000000 {}
        serial_println!("RING 0: Background Thread Alive!");
    }
}

#[no_mangle]
pub extern "C" fn user_mode_function() {
    let msg = "Hello from Ring 3 via strictly isolated SYSCALL 0x80!";
    let ptr = msg.as_ptr() as u32;
    let len = msg.len() as u32;

    unsafe {
        core::arch::asm!(
            "int 0x80",
            in("eax") 1,
            in("ebx") ptr,
            in("ecx") len,
            options(nostack, preserves_flags)
        );
    }

    loop {
        // We cannot HLT in User mode safely! So we just do busy-work and get preempted by the timer!
        for _ in 0..10000 {}
    }
}

global_asm!(r#"
.section .text
.global switch_to_user_mode
switch_to_user_mode:
    cli
    mov ax, 0x23
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax

    mov eax, esp
    push 0x23
    push eax
    push 0x202     # EFLAGS with IOPL = 0 (bits 12-13) + Interrupts Enabled (bit 9)
    push 0x1B
    
    mov eax, offset user_mode_function
    push eax
    
    iretd
"#);

extern "C" {
    fn switch_to_user_mode();
}

#[no_mangle]
pub extern "C" fn kernel_main(magic: u32, info_addr: u32) -> ! {
    
    if magic != 0x2BADB002 {
        panic!("Invalid multiboot magic: {:#x}", magic);
    }
    
    // Debug Trace
    
    let mut phys_allocator = memory::BumpAllocator::new(info_addr);
    phys_allocator.allocate_frame(); // Validate allocation
    crate::serial_println!("Kernel Boot: Allocator Ready.");
    
    // 1. Establish Ring Security Descriptors and Task State Segment
    gdt::init();
    crate::serial_println!("Kernel Boot: GDT & TSS Ready.");
    
    // 2. Wrap Memory Virtualization
    paging::init();
    crate::serial_println!("Kernel Boot: Paging Ready.");
    
    // 3. Connect Exception Gates
    interrupts::init_idt();
    pic::PICS.lock().initialize();
    // IMPORTANT: set_interrupt_stack() must be called AFTER init_idt() so the
    // TSS esp0 stack is allocated after the IDT in the heap, preventing collision.
    gdt::set_interrupt_stack();
    crate::serial_println!("Kernel Boot: Interrupts Ready.");
    
    // 4. Wire Multitasking Scheduler
    task::SCHEDULER.lock().register_task(background_thread);
    crate::serial_println!("Kernel Boot: Multitasking Ready. Initiating Ring 3 Jump...");
    
    // 5. Jump natively into Unprivileged Ring 3 execution!
    unsafe {
        switch_to_user_mode();
    }
    
    // We will never reach here!
    loop {}
}


