#[no_mangle]
pub extern "C" fn syscall_dispatcher(
    syscall_number: u32,
    arg1: u32,
    arg2: u32,
    arg3: u32,
    _arg4: u32,
    _arg5: u32,
) -> u32 {
    crate::serial_println!("[Syscall Debug] Entering Dispatcher. EAX: {}", syscall_number);
    match syscall_number {
        // SYS_PRINT
        1 => {
            // arg1 is a pointer to the string, arg2 is the length
            let ptr = arg1 as *const u8;
            let len = arg2 as usize;
            
            // Validate memory (just primitive check for our Microkernel)
            if ptr as u32 >= 0x08000000 {
                // Return Error: Invalid Memory Address
                return 1;
            }

            let slice = unsafe { core::slice::from_raw_parts(ptr, len) };
            if let Ok(s) = core::str::from_utf8(slice) {
                crate::serial_println!("[Syscall] {}", s);
            } else {
                crate::serial_println!("[Syscall] Invalid UTF-8 Passed!");
            }
            0 // Success
        }
        
        // SYS_YIELD
        2 => {
            crate::serial_println!("[Syscall] Thread Yielding!");
            // Manually forcing scheduling is a complex task without context in Rust directly from syscall, 
            // since we do a scheduler_tick on the timer. For now, just logging.
            0 // Success
        }

        _ => {
            crate::serial_println!("[Syscall] UNKNOWN SYSCALL: {}", syscall_number);
            0xFFFFFFFF // Not Implemented Error
        }
    }
}
