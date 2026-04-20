use spin::Mutex;
use alloc::vec::Vec;
use alloc::boxed::Box;

// Size of our Task Stacks: 8 Kilobytes
const STACK_SIZE: usize = 8192;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskId(usize);

pub struct Task {
    id: TaskId,
    stack: Box<[u8; STACK_SIZE]>,
    stack_ptr: u32,
}

impl Task {
    pub fn new(id: TaskId, entry_point: fn()) -> Self {
        let mut task = Task {
            id,
            stack: Box::new([0; STACK_SIZE]),
            stack_ptr: 0,
        };

        let stack_top = (task.stack.as_ptr() as usize) + STACK_SIZE;
        
        let mut sp = stack_top as u32;

        unsafe {
            // -- Mock the hardware exception frame pushed by CPU --
            
            // 1. EFLAGS (Enable Interrupts = 0x200 | Reserved bit = 0x2)
            sp -= 4;
            core::ptr::write(sp as *mut u32, 0x202);

            // 2. CS (Code Segment)
            let mut cs: u16;
            core::arch::asm!("mov {0:x}, cs", out(reg) cs);
            sp -= 4;
            core::ptr::write(sp as *mut u32, cs as u32);

            // 3. EIP (Instruction Pointer)
            sp -= 4;
            core::ptr::write(sp as *mut u32, entry_point as usize as u32);

            // -- Mock the pushad frame -- (EAX, ECX, EDX, EBX, ESP, EBP, ESI, EDI)
            for _ in 0..8 {
                sp -= 4;
                core::ptr::write(sp as *mut u32, 0); // Initialize general registers to 0
            }
            
            task.stack_ptr = sp;
        }

        task
    }
}

pub struct TaskManager {
    tasks: Vec<Task>,
    current_task_idx: usize,
    next_task_id: usize,
}

impl TaskManager {
    pub const fn new() -> Self {
        TaskManager {
            tasks: Vec::new(),
            current_task_idx: 0,
            next_task_id: 1, // 0 is reserved for the main kernel execution thread
        }
    }

    pub fn register_task(&mut self, entry_point: fn()) {
        let task = Task::new(TaskId(self.next_task_id), entry_point);
        self.next_task_id += 1;
        self.tasks.push(task);
    }

    pub fn schedule_next(&mut self, current_esp: u32) -> u32 {
        if self.tasks.is_empty() {
            // No other tasks to run, resume current thread without changing ESP
            return current_esp;
        }

        // Save current thread's stack pointer.
        // Wait! The very first thread (Main Thread) doesn't have a struct initially.
        // We will inject a dummy task struct at runtime if needed, 
        // but for simplicity, the Main Thread can just become tasks[0].
        // If we haven't tracked Main, we inject it dynamically.
        if self.tasks.len() < self.next_task_id {
            // Injecting the running thread
            self.tasks.insert(0, Task {
                id: TaskId(0),
                stack: Box::new([0; STACK_SIZE]), // This box isn't strictly correct for Main thread as it has its own BSS stack, but we just use it for the ID map natively.
                stack_ptr: 0,
            });
        }

        // Save the old stack pointer to the current task struct
        self.tasks[self.current_task_idx].stack_ptr = current_esp;

        // Robin-round to the next
        self.current_task_idx = (self.current_task_idx + 1) % self.tasks.len();

        // Return the fresh stack pointer to the CPU!
        self.tasks[self.current_task_idx].stack_ptr
    }
}

pub static SCHEDULER: Mutex<TaskManager> = Mutex::new(TaskManager::new());

// Invoked directly from the assembly timer naked interrupt!
#[no_mangle]
pub extern "C" fn scheduler_tick(esp: u32) -> u32 {
    crate::pic::ack(32); // Crucial! Acknowledge timer before jumping context!
    SCHEDULER.lock().schedule_next(esp)
}
