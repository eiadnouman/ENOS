use alloc::string::{String, ToString};
use alloc::vec::Vec;
use lazy_static::lazy_static;
use spin::Mutex;

const MAX_INPUT_LEN: usize = 256;
const MAX_HISTORY_ENTRIES: usize = 64;

#[derive(Clone, Copy)]
struct KeyboardState {
    shift_pressed: bool,
    caps_lock: bool,
    extended_prefix: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SessionUser {
    Enos,
    Guest,
}

impl SessionUser {
    fn name(self) -> &'static str {
        match self {
            SessionUser::Enos => "enos",
            SessionUser::Guest => "guest",
        }
    }

    fn uid(self) -> u32 {
        match self {
            SessionUser::Enos => 0,
            SessionUser::Guest => 1000,
        }
    }

    fn is_root(self) -> bool {
        matches!(self, SessionUser::Enos)
    }

    fn prompt_suffix(self) -> &'static str {
        if self.is_root() {
            "#"
        } else {
            "$"
        }
    }
}

#[derive(Clone, Copy)]
struct SessionState {
    user: SessionUser,
}

impl SessionState {
    const fn new() -> Self {
        SessionState {
            user: SessionUser::Enos,
        }
    }
}

struct HistoryState {
    entries: Vec<String>,
    index: Option<usize>,
    draft: String,
}

impl HistoryState {
    fn new() -> Self {
        HistoryState {
            entries: Vec::new(),
            index: None,
            draft: String::new(),
        }
    }

    fn reset_navigation(&mut self) {
        self.index = None;
        self.draft.clear();
    }

    fn record(&mut self, command: &str) {
        if command.is_empty() {
            self.reset_navigation();
            return;
        }

        let should_push = self
            .entries
            .last()
            .map(|last| last.as_str() != command)
            .unwrap_or(true);
        if should_push {
            if self.entries.len() >= MAX_HISTORY_ENTRIES {
                self.entries.remove(0);
            }
            self.entries.push(command.to_string());
        }

        self.reset_navigation();
    }

    fn previous(&mut self, current_line: &str) -> Option<String> {
        if self.entries.is_empty() {
            return None;
        }

        match self.index {
            None => {
                self.draft.clear();
                self.draft.push_str(current_line);
                self.index = Some(self.entries.len() - 1);
            }
            Some(0) => {}
            Some(i) => {
                self.index = Some(i - 1);
            }
        }

        self.index.map(|i| self.entries[i].clone())
    }

    fn next(&mut self) -> Option<String> {
        let index = self.index?;
        if index + 1 < self.entries.len() {
            self.index = Some(index + 1);
            return Some(self.entries[index + 1].clone());
        }

        self.index = None;
        Some(self.draft.clone())
    }
}

#[derive(Clone, Copy)]
enum InputAction {
    Backspace,
    Enter,
    Tab,
    HistoryUp,
    HistoryDown,
    Char(char),
}

lazy_static! {
    pub static ref INPUT_BUFFER: Mutex<String> = Mutex::new(String::new());
    static ref KEYBOARD_STATE: Mutex<KeyboardState> = Mutex::new(KeyboardState {
        shift_pressed: false,
        caps_lock: false,
        extended_prefix: false
    });
    static ref HISTORY: Mutex<HistoryState> = Mutex::new(HistoryState::new());
    static ref SESSION: Mutex<SessionState> = Mutex::new(SessionState::new());
}

pub fn init() {
    crate::println!("ENOS shell ready. Type `help` for commands.");
    print_prompt();
}

fn print_prompt() {
    let session = *SESSION.lock();
    crate::print!("{}{} ", session.user.name(), session.user.prompt_suffix());
}

fn is_root() -> bool {
    SESSION.lock().user.is_root()
}

fn require_root(action: &str) -> bool {
    if is_root() {
        true
    } else {
        crate::println!("Permission denied: `{}` requires enos root.", action);
        false
    }
}

fn switch_user(user: SessionUser) {
    SESSION.lock().user = user;
}

fn clear_visible_input(count: usize) {
    for _ in 0..count {
        crate::print!("\u{08}");
    }
}

fn replace_input_line(text: &str) {
    let rendered = {
        let mut buffer = INPUT_BUFFER.lock();
        clear_visible_input(buffer.len());
        buffer.clear();
        for ch in text.chars() {
            if buffer.len() >= MAX_INPUT_LEN {
                break;
            }
            buffer.push(ch);
        }
        buffer.clone()
    };

    crate::print!("{}", rendered);
}

fn history_up() {
    let current = INPUT_BUFFER.lock().clone();
    let previous = HISTORY.lock().previous(&current);
    if let Some(entry) = previous {
        replace_input_line(&entry);
    }
}

fn history_down() {
    let next = HISTORY.lock().next();
    if let Some(entry) = next {
        replace_input_line(&entry);
    }
}

pub fn push_char(c: char) {
    if c == '\u{08}' {
        // Backspace
        HISTORY.lock().reset_navigation();
        let mut buffer = INPUT_BUFFER.lock();
        if !buffer.is_empty() {
            buffer.pop();
            crate::print!("{}", c);
        }
        return;
    }

    if c == '\n' {
        crate::print!("{}", c);
        evaluate_command();
        return;
    }

    let mut buffer = INPUT_BUFFER.lock();
    if buffer.len() < MAX_INPUT_LEN {
        HISTORY.lock().reset_navigation();
        buffer.push(c);
        crate::print!("{}", c);
    }
}

fn map_letter_base(scancode: u8) -> Option<char> {
    match scancode {
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
        _ => None,
    }
}

fn map_non_letter(scancode: u8, shifted: bool) -> Option<char> {
    match scancode {
        0x02 => Some(if shifted { '!' } else { '1' }),
        0x03 => Some(if shifted { '@' } else { '2' }),
        0x04 => Some(if shifted { '#' } else { '3' }),
        0x05 => Some(if shifted { '$' } else { '4' }),
        0x06 => Some(if shifted { '%' } else { '5' }),
        0x07 => Some(if shifted { '^' } else { '6' }),
        0x08 => Some(if shifted { '&' } else { '7' }),
        0x09 => Some(if shifted { '*' } else { '8' }),
        0x0A => Some(if shifted { '(' } else { '9' }),
        0x0B => Some(if shifted { ')' } else { '0' }),
        0x0C => Some(if shifted { '_' } else { '-' }),
        0x0D => Some(if shifted { '+' } else { '=' }),
        0x1A => Some(if shifted { '{' } else { '[' }),
        0x1B => Some(if shifted { '}' } else { ']' }),
        0x27 => Some(if shifted { ':' } else { ';' }),
        0x28 => Some(if shifted { '"' } else { '\'' }),
        0x29 => Some(if shifted { '~' } else { '`' }),
        0x2B => Some(if shifted { '|' } else { '\\' }),
        0x33 => Some(if shifted { '<' } else { ',' }),
        0x34 => Some(if shifted { '>' } else { '.' }),
        0x35 => Some(if shifted { '?' } else { '/' }),
        0x39 => Some(' '),
        // Keypad / numpad common scan codes (when NumLock is active).
        0x37 => Some('*'),
        0x47 => Some('7'),
        0x48 => Some('8'),
        0x49 => Some('9'),
        0x4A => Some('-'),
        0x4B => Some('4'),
        0x4C => Some('5'),
        0x4D => Some('6'),
        0x4E => Some('+'),
        0x4F => Some('1'),
        0x50 => Some('2'),
        0x51 => Some('3'),
        0x52 => Some('0'),
        0x53 => Some('.'),
        _ => None,
    }
}

fn map_key(scancode: u8, shifted: bool, caps_lock: bool) -> Option<char> {
    if let Some(mut c) = map_letter_base(scancode) {
        if caps_lock ^ shifted {
            c = c.to_ascii_uppercase();
        }
        return Some(c);
    }

    map_non_letter(scancode, shifted)
}

pub fn process_scancode(scancode: u8) {
    if scancode == 0xE0 {
        KEYBOARD_STATE.lock().extended_prefix = true;
        return;
    }

    let mut action = None;

    {
        let mut keyboard = KEYBOARD_STATE.lock();

        if keyboard.extended_prefix {
            keyboard.extended_prefix = false;
            let is_release = (scancode & 0x80) != 0;
            let code = scancode & 0x7F;
            if !is_release {
                action = match code {
                    0x48 => Some(InputAction::HistoryUp),
                    0x50 => Some(InputAction::HistoryDown),
                    // Left/Right arrows are consumed here so they don't print keypad digits.
                    0x4B | 0x4D => None,
                    _ => None,
                };
            }
        } else {
            let is_release = (scancode & 0x80) != 0;
            let code = scancode & 0x7F;

            if is_release {
                if code == 0x2A || code == 0x36 {
                    keyboard.shift_pressed = false;
                }
                return;
            }

            action = match code {
                0x2A | 0x36 => {
                    keyboard.shift_pressed = true;
                    None
                }
                0x3A => {
                    keyboard.caps_lock = !keyboard.caps_lock;
                    None
                }
                0x0E => Some(InputAction::Backspace),
                0x1C => Some(InputAction::Enter),
                0x0F => Some(InputAction::Tab),
                _ => map_key(code, keyboard.shift_pressed, keyboard.caps_lock).map(InputAction::Char),
            };
        }
    }

    match action {
        Some(InputAction::Backspace) => push_char('\u{08}'),
        Some(InputAction::Enter) => push_char('\n'),
        Some(InputAction::Tab) => {
            for _ in 0..4 {
                push_char(' ');
            }
        }
        Some(InputAction::HistoryUp) => history_up(),
        Some(InputAction::HistoryDown) => history_down(),
        Some(InputAction::Char(c)) => push_char(c),
        None => {}
    }
}

fn ticks_to_ms(ticks: u64) -> u64 {
    crate::pit::ticks_to_millis(ticks)
}

fn format_duration_from_ticks(ticks: u64) -> (u64, u64, u64, u64) {
    let total_ms = ticks_to_ms(ticks);
    let total_seconds = total_ms / 1000;
    let ms = total_ms % 1000;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let seconds = total_seconds % 60;
    (hours, minutes, seconds, ms)
}

fn format_bytes(bytes: usize) -> String {
    if bytes >= 1024 * 1024 {
        let mb = bytes / (1024 * 1024);
        let rem = (bytes % (1024 * 1024)) / (1024 * 10);
        let mut out = String::from("");
        out.push_str("~");
        out.push_str(&mb.to_string());
        out.push('.');
        out.push_str(&rem.to_string());
        out.push_str(" MB");
        out
    } else if bytes >= 1024 {
        let kb = bytes / 1024;
        let rem = (bytes % 1024) * 10 / 1024;
        let mut out = String::from("");
        out.push_str("~");
        out.push_str(&kb.to_string());
        out.push('.');
        out.push_str(&rem.to_string());
        out.push_str(" KB");
        out
    } else {
        let mut out = String::from("");
        out.push_str(&bytes.to_string());
        out.push_str(" B");
        out
    }
}

struct CalcParser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> CalcParser<'a> {
    fn new(expr: &'a str) -> Self {
        CalcParser {
            bytes: expr.as_bytes(),
            pos: 0,
        }
    }

    fn skip_ws(&mut self) {
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_whitespace() {
            self.pos += 1;
        }
    }

    fn consume(&mut self, ch: u8) -> bool {
        self.skip_ws();
        if self.pos < self.bytes.len() && self.bytes[self.pos] == ch {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn parse_expr(&mut self) -> Result<i64, &'static str> {
        let mut value = self.parse_term()?;
        loop {
            if self.consume(b'+') {
                let rhs = self.parse_term()?;
                value = value.checked_add(rhs).ok_or("integer overflow")?;
            } else if self.consume(b'-') {
                let rhs = self.parse_term()?;
                value = value.checked_sub(rhs).ok_or("integer overflow")?;
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn parse_term(&mut self) -> Result<i64, &'static str> {
        let mut value = self.parse_factor()?;
        loop {
            if self.consume(b'*') {
                let rhs = self.parse_factor()?;
                value = value.checked_mul(rhs).ok_or("integer overflow")?;
            } else if self.consume(b'/') {
                let rhs = self.parse_factor()?;
                if rhs == 0 {
                    return Err("division by zero");
                }
                value = value.checked_div(rhs).ok_or("integer overflow")?;
            } else if self.consume(b'%') {
                let rhs = self.parse_factor()?;
                if rhs == 0 {
                    return Err("division by zero");
                }
                value = value.checked_rem(rhs).ok_or("integer overflow")?;
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn parse_factor(&mut self) -> Result<i64, &'static str> {
        if self.consume(b'+') {
            return self.parse_factor();
        }
        if self.consume(b'-') {
            let v = self.parse_factor()?;
            return v.checked_neg().ok_or("integer overflow");
        }
        if self.consume(b'(') {
            let v = self.parse_expr()?;
            if !self.consume(b')') {
                return Err("missing closing ')'");
            }
            return Ok(v);
        }
        self.parse_number()
    }

    fn parse_number(&mut self) -> Result<i64, &'static str> {
        self.skip_ws();
        let mut has_digit = false;
        let mut value: i64 = 0;

        while self.pos < self.bytes.len() {
            let b = self.bytes[self.pos];
            if !b.is_ascii_digit() {
                break;
            }
            has_digit = true;
            let digit = (b - b'0') as i64;
            value = value.checked_mul(10).ok_or("integer overflow")?;
            value = value.checked_add(digit).ok_or("integer overflow")?;
            self.pos += 1;
        }

        if !has_digit {
            return Err("expected number");
        }

        Ok(value)
    }
}

fn eval_calc_expression(expr: &str) -> Result<i64, &'static str> {
    let mut parser = CalcParser::new(expr);
    let value = parser.parse_expr()?;
    parser.skip_ws();
    if parser.pos != parser.bytes.len() {
        return Err("unexpected trailing input");
    }
    Ok(value)
}

fn show_help() {
    crate::println!("ENOS built-in commands:");
    crate::println!("  help                 - Show this list");
    crate::println!("  clear                - Clear screen");
    crate::println!("  echo <msg>           - Print message");
    crate::println!("  calc <expr>          - Arithmetic (+,-,*,/,%,())");
    crate::println!("  uptime               - Show human-readable uptime");
    crate::println!("  ps                   - Show running processes");
    crate::println!("  top                  - Show process CPU usage");
    crate::println!("  tasks                - Show scheduler stats + process list");
    crate::println!("  meminfo              - Show memory usage");
    crate::println!("  whoami               - Show current shell user");
    crate::println!("  id                   - Show user identity");
    crate::println!("  login <enos|guest>   - Switch shell identity");
    crate::println!("  su <enos|guest>      - Alias for login");
    crate::println!("  logout               - Switch to guest");
    crate::println!("  kill <pid>           - Kill user process (root only)");
    crate::println!("  touch <file>         - Create empty file");
    crate::println!("  rm <file>            - Delete file (root only)");
    crate::println!("  ls                   - List files");
    crate::println!("  write <file> <text>  - Write text to file");
    crate::println!("  cat <file>           - Show file content");
    crate::println!("  fsinfo               - Show filesystem stats");
    crate::println!("  about                - Show kernel info");
}

fn command_echo(command_line: &str) {
    let text = command_line
        .strip_prefix("echo")
        .unwrap_or("")
        .trim_start();

    if text.is_empty() {
        crate::println!("Usage: echo <msg>");
        return;
    }

    crate::println!("{}", text);
}

fn command_calc(command_line: &str) {
    let expr = command_line
        .strip_prefix("calc")
        .unwrap_or("")
        .trim_start();
    if expr.is_empty() {
        crate::println!("Usage: calc <expression>");
        return;
    }

    match eval_calc_expression(expr) {
        Ok(value) => crate::println!("= {}", value),
        Err(err) => crate::println!("calc: {}", err),
    }
}

fn command_uptime() {
    let stats = crate::task::scheduler_stats();
    let (h, m, s, ms) = format_duration_from_ticks(stats.total_ticks);
    crate::println!(
        "Uptime: {:02}:{:02}:{:02}.{:03} ({} ticks @ {} Hz)",
        h,
        m,
        s,
        ms,
        stats.total_ticks,
        crate::pit::TICKS_PER_SECOND
    );
}

fn command_ps() {
    let processes = crate::task::scheduler_process_snapshot();

    crate::println!("PID   PRIV     STATE    CPU-TIME   NAME");
    for process in processes {
        let cpu_ms = ticks_to_ms(process.run_ticks);
        crate::println!(
            "{:<5} {:<8} {:<8} {:>6}ms   {}",
            process.id,
            process.privilege,
            process.state,
            cpu_ms,
            process.name
        );
    }
}

fn command_top() {
    let stats = crate::task::scheduler_stats();
    let processes = crate::task::scheduler_process_snapshot();

    crate::println!(
        "TOP (ticks={} @ {}Hz)",
        stats.total_ticks,
        crate::pit::TICKS_PER_SECOND
    );
    crate::println!("PID   CPU%   CPU-TIME   STATE    NAME");

    for process in processes {
        let cpu_pct = if stats.total_ticks == 0 {
            0
        } else {
            (process.run_ticks.saturating_mul(100)) / stats.total_ticks
        };
        let cpu_ms = ticks_to_ms(process.run_ticks);
        crate::println!(
            "{:<5} {:>3}%   {:>6}ms   {:<8} {}",
            process.id,
            cpu_pct,
            cpu_ms,
            process.state,
            process.name
        );
    }
}

fn command_tasks() {
    let stats = crate::task::scheduler_stats();
    let (h, m, s, ms) = format_duration_from_ticks(stats.total_ticks);
    crate::println!("Scheduler:");
    crate::println!("  total tasks : {}", stats.total_tasks);
    crate::println!("  current task: {}", stats.current_task_id);
    crate::println!("  uptime      : {:02}:{:02}:{:02}.{:03}", h, m, s, ms);
    crate::println!("  ticks       : {}", stats.total_ticks);
    command_ps();
}

fn command_meminfo() {
    let heap = crate::allocator::heap_stats();
    let fs = crate::fs::stats();

    crate::println!("Memory:");
    crate::println!(
        "  heap used : {} ({}%)",
        format_bytes(heap.used_bytes),
        heap.used_percent
    );
    crate::println!("  heap free : {}", format_bytes(heap.free_bytes));
    crate::println!("  heap total: {}", format_bytes(heap.total_bytes));
    crate::println!(
        "  fs usage  : {}/{} files, {} data",
        fs.file_count,
        fs.max_files,
        format_bytes(fs.total_bytes)
    );
}

fn command_whoami() {
    let session = *SESSION.lock();
    crate::println!("{}", session.user.name());
}

fn command_id() {
    let session = *SESSION.lock();
    let uid = session.user.uid();
    crate::println!(
        "uid={}({}) gid={}({}) role={}",
        uid,
        session.user.name(),
        uid,
        session.user.name(),
        if session.user.is_root() { "root" } else { "user" }
    );
}

fn parse_login_target(command_line: &str) -> Option<&str> {
    let mut parts = command_line.split_whitespace();
    parts.next();
    parts.next()
}

fn command_login(command_line: &str) {
    let Some(target) = parse_login_target(command_line) else {
        crate::println!("Usage: login <enos|guest>");
        return;
    };

    match target {
        "enos" => {
            switch_user(SessionUser::Enos);
            crate::println!("Switched to enos (root).");
        }
        "guest" | "user" => {
            switch_user(SessionUser::Guest);
            crate::println!("Switched to guest.");
        }
        _ => crate::println!("login: unknown user"),
    }
}

fn command_logout() {
    switch_user(SessionUser::Guest);
    crate::println!("Logged out to guest.");
}

fn command_kill(command_line: &str) {
    if !require_root("kill") {
        return;
    }

    let mut parts = command_line.split_whitespace();
    parts.next(); // kill
    let Some(pid_str) = parts.next() else {
        crate::println!("Usage: kill <pid>");
        return;
    };

    let Ok(pid) = pid_str.parse::<usize>() else {
        crate::println!("kill: invalid pid");
        return;
    };

    match crate::task::kill_task(pid) {
        Ok(crate::task::KillTaskResult::KilledNow) => {
            crate::println!("PID {} terminated.", pid);
        }
        Ok(crate::task::KillTaskResult::MarkedForTermination) => {
            crate::println!("PID {} marked for termination (next timer tick).", pid);
        }
        Err(err) => {
            crate::println!("kill: {}", err);
        }
    }

    command_ps();
}

fn command_touch(command_line: &str) {
    let mut parts = command_line.split_whitespace();
    parts.next(); // touch
    let Some(name) = parts.next() else {
        crate::println!("Usage: touch <file>");
        return;
    };

    match crate::fs::create_file(name) {
        Ok(()) => crate::println!("Created: {}", name),
        Err(err) => crate::println!("touch: {}", err),
    }
}

fn command_rm(command_line: &str) {
    if !require_root("rm") {
        return;
    }

    let mut parts = command_line.split_whitespace();
    parts.next(); // rm
    let Some(name) = parts.next() else {
        crate::println!("Usage: rm <file>");
        return;
    };

    match crate::fs::delete_file(name) {
        Ok(()) => crate::println!("Deleted: {}", name),
        Err(err) => crate::println!("rm: {}", err),
    }
}

fn command_ls() {
    let mut files = crate::fs::list_files();
    if files.is_empty() {
        crate::println!("(empty)");
        return;
    }

    files.sort_by(|a, b| a.name.cmp(&b.name));
    crate::println!("NAME                             SIZE");
    for file in files {
        crate::println!("{:<32} {}", file.name, format_bytes(file.size));
    }
}

fn command_write(command_line: &str) {
    let mut parts = command_line.splitn(3, ' ');
    parts.next(); // write
    let Some(name) = parts.next().filter(|s| !s.is_empty()) else {
        crate::println!("Usage: write <file> <text>");
        return;
    };
    let Some(content_raw) = parts.next() else {
        crate::println!("Usage: write <file> <text>");
        return;
    };
    let content = content_raw.trim_start();

    match crate::fs::write_file(name, content) {
        Ok(()) => crate::println!("Written: {}", name),
        Err(err) => crate::println!("write: {}", err),
    }
}

fn command_cat(command_line: &str) {
    let mut parts = command_line.split_whitespace();
    parts.next(); // cat
    let Some(name) = parts.next() else {
        crate::println!("Usage: cat <file>");
        return;
    };

    match crate::fs::read_file(name) {
        Ok(content) => crate::println!("{}", content),
        Err(err) => crate::println!("cat: {}", err),
    }
}

fn command_fsinfo() {
    let stats = crate::fs::stats();
    crate::println!("Filesystem:");
    crate::println!("  files     : {}/{}", stats.file_count, stats.max_files);
    crate::println!("  data size : {}", format_bytes(stats.total_bytes));
    crate::println!("  max file  : {}", format_bytes(stats.max_file_size));
}

fn command_about() {
    crate::println!("ENOS microkernel (x86, no_std, Rust)");
    crate::println!("Features: ring3 syscall gate, paging, preemptive scheduler, shell, ramfs");
}

pub fn evaluate_command() {
    let command_line = {
        let mut buffer = INPUT_BUFFER.lock();
        let line = String::from(buffer.trim());
        buffer.clear();
        line
    };

    if command_line.is_empty() {
        HISTORY.lock().reset_navigation();
        print_prompt();
        return;
    }

    HISTORY.lock().record(&command_line);

    let command = command_line.split_whitespace().next().unwrap_or("");

    match command {
        "clear" => crate::vga_buffer::clear_screen(),
        "echo" => command_echo(&command_line),
        "calc" => command_calc(&command_line),
        "help" => show_help(),
        "uptime" => command_uptime(),
        "ps" => command_ps(),
        "top" => command_top(),
        "tasks" => command_tasks(),
        "meminfo" => command_meminfo(),
        "whoami" => command_whoami(),
        "id" => command_id(),
        "login" => command_login(&command_line),
        "su" => command_login(&command_line),
        "logout" => command_logout(),
        "kill" => command_kill(&command_line),
        "touch" => command_touch(&command_line),
        "rm" => command_rm(&command_line),
        "ls" => command_ls(),
        "write" => command_write(&command_line),
        "cat" => command_cat(&command_line),
        "fsinfo" => command_fsinfo(),
        "about" => command_about(),
        _ => crate::println!("Error: command not found: {}", command_line),
    }

    print_prompt();
}
