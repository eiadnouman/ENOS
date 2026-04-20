use alloc::string::String;
use spin::Mutex;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref INPUT_BUFFER: Mutex<String> = Mutex::new(String::new());
}

pub fn init() {
    crate::println!("ENOS Shell initialized.");
    crate::print!("> ");
}

pub fn push_char(c: char) {
    if c == '\u{08}' { // Backspace
        let mut buffer = INPUT_BUFFER.lock();
        if !buffer.is_empty() {
            buffer.pop();
            crate::print!("{}", c);
        }
    } else if c == '\n' { // Enter
        crate::print!("{}", c);
        evaluate_command();
    } else { // Normal character
        INPUT_BUFFER.lock().push(c);
        crate::print!("{}", c);
    }
}

pub fn process_scancode(scancode: u8) {
    let ch = match scancode {
        0x02..=0x0B => {
            if scancode == 0x0B { Some('0') } else { Some(('1' as u8 + (scancode - 0x02)) as char) }
        }
        0x10 => Some('q'),
        0x11 => Some('w'),
        0x12 => Some('e'),
        0x13 => Some('r'),
        0x14 => Some('t'),
        0x15 => Some('y'),
        0x16 => Some('u'),
        0x17 => Some('i'),
        0x18 => Some('o'),
        0x19 => Some('p'),
        0x1E => Some('a'),
        0x1F => Some('s'),
        0x20 => Some('d'),
        0x21 => Some('f'),
        0x22 => Some('g'),
        0x23 => Some('h'),
        0x24 => Some('j'),
        0x25 => Some('k'),
        0x26 => Some('l'),
        0x2C => Some('z'),
        0x2D => Some('x'),
        0x2E => Some('c'),
        0x2F => Some('v'),
        0x30 => Some('b'),
        0x31 => Some('n'),
        0x32 => Some('m'),
        0x39 => Some(' '),
        0x1C => Some('\n'),
        0x0E => Some('\u{08}'), // Backspace
        _ => None,
    };

    if let Some(c) = ch {
        push_char(c);
    }
}

pub fn evaluate_command() {
    let mut buffer = INPUT_BUFFER.lock();
    let command = buffer.trim();

    if command.is_empty() {
        crate::print!("> ");
        return;
    }

    if command == "clear" {
        for _ in 0..25 {
            crate::println!();
        }
    } else if command.starts_with("echo ") {
        let text = &command[5..];
        crate::println!("{}", text);
    } else if command == "help" {
        crate::println!("ENOS Built-in Commands:");
        crate::println!("  echo <msg>  - Prints the message");
        crate::println!("  clear       - Clears the screen");
        crate::println!("  help        - Shows this message");
    } else {
        crate::println!("Error: command not found: {}", command);
    }

    buffer.clear();
    crate::print!("> ");
}
