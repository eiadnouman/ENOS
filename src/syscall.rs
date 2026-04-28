const USER_PTR_MIN: u32 = 0x0040_0000;
const USER_PTR_MAX_EXCLUSIVE: u32 = 0x0080_0000;
const MAX_PRINT_LEN: usize = 1024;

const ERR_INVALID_POINTER: u32 = 1;
const ERR_INVALID_LENGTH: u32 = 2;
const ERR_INVALID_UTF8: u32 = 3;
const ERR_INVALID_CONTEXT: u32 = 4;
const ERR_NOT_IMPLEMENTED: u32 = 0xFFFF_FFFF;

fn validate_user_buffer(ptr: u32, len: usize) -> Result<(*const u8, usize), u32> {
    if len == 0 || len > MAX_PRINT_LEN {
        return Err(ERR_INVALID_LENGTH);
    }

    if ptr < USER_PTR_MIN {
        return Err(ERR_INVALID_POINTER);
    }

    let end = match ptr.checked_add(len as u32) {
        Some(v) => v,
        None => return Err(ERR_INVALID_POINTER),
    };

    if end > USER_PTR_MAX_EXCLUSIVE || end <= ptr {
        return Err(ERR_INVALID_POINTER);
    }

    Ok((ptr as *const u8, len))
}

#[no_mangle]
pub extern "C" fn syscall_dispatcher(
    syscall_number: u32,
    arg1: u32,
    arg2: u32,
    _arg3: u32,
    _arg4: u32,
    _arg5: u32,
    current_esp: u32,
) -> u32 {
    match syscall_number {
        // SYS_PRINT
        1 => {
            let (ptr, len) = match validate_user_buffer(arg1, arg2 as usize) {
                Ok(v) => v,
                Err(code) => return code,
            };

            let slice = unsafe { core::slice::from_raw_parts(ptr, len) };
            if let Ok(s) = core::str::from_utf8(slice) {
                crate::serial_println!("[Syscall] {}", s);
                0 // Success
            } else {
                ERR_INVALID_UTF8
            }
        }

        // SYS_YIELD
        2 => {
            if crate::task::sleep_current_task(current_esp, 1) {
                0
            } else {
                ERR_INVALID_CONTEXT
            }
        }

        // SYS_SLEEP (arg1 = ticks)
        3 => {
            if crate::task::sleep_current_task(current_esp, arg1) {
                0
            } else {
                ERR_INVALID_CONTEXT
            }
        }

        _ => {
            crate::serial_println!("[Syscall] UNKNOWN SYSCALL: {}", syscall_number);
            ERR_NOT_IMPLEMENTED
        }
    }
}
