use alloc::boxed::Box;
use alloc::vec::Vec;
use spin::Mutex;

// Size of our Task Stacks: 8 Kilobytes
const STACK_SIZE: usize = 8192;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaskId(usize);

impl TaskId {
    fn as_usize(self) -> usize {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskPrivilege {
    Kernel,
    User,
}

impl TaskPrivilege {
    fn label(self) -> &'static str {
        match self {
            TaskPrivilege::Kernel => "kernel",
            TaskPrivilege::User => "user",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskState {
    Ready,
    Sleeping { wake_at: u64 },
    Terminated,
}

impl TaskState {
    fn is_ready(self) -> bool {
        matches!(self, TaskState::Ready)
    }
}

pub struct Task {
    id: TaskId,
    name: &'static str,
    privilege: TaskPrivilege,
    state: TaskState,
    stack: Box<[u8; STACK_SIZE]>,
    stack_ptr: u32,
    run_ticks: u64,
}

impl Task {
    fn new(id: TaskId, name: &'static str, privilege: TaskPrivilege, entry_point: fn()) -> Self {
        let mut task = Task {
            id,
            name,
            privilege,
            state: TaskState::Ready,
            stack: Box::new([0; STACK_SIZE]),
            stack_ptr: 0,
            run_ticks: 0,
        };

        let stack_top = (task.stack.as_ptr() as usize) + STACK_SIZE;
        let mut sp = stack_top as u32;

        unsafe {
            // Mock interrupt frame + pushad frame expected by timer_interrupt_wrapper.
            sp -= 4;
            core::ptr::write(sp as *mut u32, 0x202); // EFLAGS

            let mut cs: u16;
            core::arch::asm!("mov {0:x}, cs", out(reg) cs);
            sp -= 4;
            core::ptr::write(sp as *mut u32, cs as u32); // CS

            sp -= 4;
            core::ptr::write(sp as *mut u32, entry_point as usize as u32); // EIP

            // pushad frame: EAX, ECX, EDX, EBX, ESP, EBP, ESI, EDI
            for _ in 0..8 {
                sp -= 4;
                core::ptr::write(sp as *mut u32, 0);
            }

            task.stack_ptr = sp;
        }

        task
    }

    fn placeholder_main(stack_ptr: u32) -> Self {
        Task {
            id: TaskId(1),
            name: "user_main",
            privilege: TaskPrivilege::User,
            state: TaskState::Ready,
            // Placeholder storage to keep layout consistent with other tasks.
            stack: Box::new([0; STACK_SIZE]),
            stack_ptr,
            run_ticks: 0,
        }
    }

    fn state_label(&self, is_current: bool) -> &'static str {
        match self.state {
            TaskState::Ready => {
                if is_current {
                    "running"
                } else {
                    "ready"
                }
            }
            TaskState::Sleeping { .. } => "sleep",
            TaskState::Terminated => "dead",
        }
    }
}

pub struct TaskManager {
    tasks: Vec<Task>,
    current_task_idx: usize,
    next_task_id: usize,
    tick_count: u64,
    main_task_tracked: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct SchedulerStats {
    pub total_ticks: u64,
    pub total_tasks: usize,
    pub current_task_id: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct ProcessSnapshot {
    pub id: usize,
    pub name: &'static str,
    pub state: &'static str,
    pub privilege: &'static str,
    pub run_ticks: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KillTaskResult {
    KilledNow,
    MarkedForTermination,
}

impl TaskManager {
    pub const fn new() -> Self {
        TaskManager {
            tasks: Vec::new(),
            current_task_idx: 0,
            next_task_id: 2, // 0=kernel_idle (virtual), 1=initial user thread
            tick_count: 0,
            main_task_tracked: false,
        }
    }

    fn ensure_main_task_tracked(&mut self, current_esp: u32) {
        if !self.main_task_tracked {
            self.tasks.insert(0, Task::placeholder_main(current_esp));
            self.main_task_tracked = true;
            self.current_task_idx = 0;
        }
    }

    fn wake_sleeping_tasks(&mut self) {
        for task in &mut self.tasks {
            if let TaskState::Sleeping { wake_at } = task.state {
                if self.tick_count >= wake_at {
                    task.state = TaskState::Ready;
                }
            }
        }
    }

    fn cleanup_terminated_non_current(&mut self) {
        let mut idx = 0usize;
        while idx < self.tasks.len() {
            if idx == self.current_task_idx {
                idx += 1;
                continue;
            }

            if self.tasks[idx].state == TaskState::Terminated {
                self.tasks.remove(idx);
                if idx < self.current_task_idx && self.current_task_idx > 0 {
                    self.current_task_idx -= 1;
                }
                continue;
            }

            idx += 1;
        }

        if self.current_task_idx >= self.tasks.len() && !self.tasks.is_empty() {
            self.current_task_idx = 0;
        }
    }

    fn first_ready_index(&self) -> Option<usize> {
        self.tasks.iter().position(|task| task.state.is_ready())
    }

    fn next_ready_after(&self, start_idx: usize) -> Option<usize> {
        if self.tasks.is_empty() {
            return None;
        }

        for offset in 1..=self.tasks.len() {
            let idx = (start_idx + offset) % self.tasks.len();
            if self.tasks[idx].state.is_ready() {
                return Some(idx);
            }
        }

        None
    }

    pub fn register_named_task(&mut self, name: &'static str, entry_point: fn()) {
        let task = Task::new(
            TaskId(self.next_task_id),
            name,
            TaskPrivilege::Kernel,
            entry_point,
        );
        self.next_task_id += 1;
        self.tasks.push(task);
    }

    pub fn schedule_next(&mut self, current_esp: u32) -> u32 {
        self.tick_count = self.tick_count.wrapping_add(1);

        if self.tasks.is_empty() {
            return current_esp;
        }

        self.ensure_main_task_tracked(current_esp);
        self.wake_sleeping_tasks();
        self.cleanup_terminated_non_current();

        if self.tasks.is_empty() {
            panic!("Scheduler stalled: no tasks available");
        }
        if self.current_task_idx >= self.tasks.len() {
            self.current_task_idx = 0;
        }

        if self.tasks[self.current_task_idx].state == TaskState::Terminated {
            self.tasks.remove(self.current_task_idx);
            if self.tasks.is_empty() {
                panic!("All tasks terminated");
            }
            if self.current_task_idx >= self.tasks.len() {
                self.current_task_idx = 0;
            }
        } else {
            self.tasks[self.current_task_idx].stack_ptr = current_esp;
            self.tasks[self.current_task_idx].run_ticks =
                self.tasks[self.current_task_idx].run_ticks.wrapping_add(1);
        }

        self.cleanup_terminated_non_current();

        let next_idx = self
            .next_ready_after(self.current_task_idx)
            .or_else(|| {
                self.tasks.get(self.current_task_idx).and_then(|task| {
                    if task.state.is_ready() {
                        Some(self.current_task_idx)
                    } else {
                        None
                    }
                })
            })
            .or_else(|| self.first_ready_index())
            .unwrap_or_else(|| panic!("Scheduler stalled: no runnable tasks"));

        self.current_task_idx = next_idx;
        self.tasks[next_idx].stack_ptr
    }

    pub fn sleep_current_task(&mut self, current_esp: u32, ticks: u32) -> bool {
        if self.tasks.is_empty() {
            return false;
        }

        self.ensure_main_task_tracked(current_esp);
        if self.tasks.is_empty() {
            return false;
        }
        if self.current_task_idx >= self.tasks.len() {
            return false;
        }

        let current = &mut self.tasks[self.current_task_idx];
        if current.privilege != TaskPrivilege::User {
            return false;
        }
        if current.state == TaskState::Terminated {
            return false;
        }

        current.stack_ptr = current_esp;
        let sleep_ticks = core::cmp::max(1_u64, ticks as u64);
        current.state = TaskState::Sleeping {
            wake_at: self.tick_count.wrapping_add(sleep_ticks),
        };
        true
    }

    pub fn kill_task_by_pid(&mut self, pid: usize) -> Result<KillTaskResult, &'static str> {
        if pid == 0 {
            return Err("cannot kill kernel task");
        }

        let Some(idx) = self.tasks.iter().position(|task| task.id.as_usize() == pid) else {
            return Err("pid not found");
        };

        if self.tasks[idx].privilege != TaskPrivilege::User {
            return Err("cannot kill kernel task");
        }

        if idx == self.current_task_idx {
            self.tasks[idx].state = TaskState::Terminated;
            return Ok(KillTaskResult::MarkedForTermination);
        }

        self.tasks.remove(idx);
        if idx < self.current_task_idx && self.current_task_idx > 0 {
            self.current_task_idx -= 1;
        }
        if self.current_task_idx >= self.tasks.len() && !self.tasks.is_empty() {
            self.current_task_idx = 0;
        }
        Ok(KillTaskResult::KilledNow)
    }

    pub fn terminate_current_user_task_from_fault(&mut self, current_esp: u32) -> u32 {
        self.ensure_main_task_tracked(current_esp);

        if self.tasks.is_empty() {
            panic!("Task list empty while handling user fault");
        }
        if self.current_task_idx >= self.tasks.len() {
            self.current_task_idx = 0;
        }
        if self.tasks[self.current_task_idx].privilege != TaskPrivilege::User {
            panic!("User-fault path invoked while current task is not user");
        }

        self.tasks.remove(self.current_task_idx);
        if self.tasks.is_empty() {
            panic!("All tasks terminated after user fault");
        }
        if self.current_task_idx >= self.tasks.len() {
            self.current_task_idx = 0;
        }

        self.wake_sleeping_tasks();
        self.cleanup_terminated_non_current();

        let next_idx = self
            .next_ready_after(self.current_task_idx)
            .or_else(|| {
                self.tasks.get(self.current_task_idx).and_then(|task| {
                    if task.state.is_ready() {
                        Some(self.current_task_idx)
                    } else {
                        None
                    }
                })
            })
            .or_else(|| self.first_ready_index())
            .unwrap_or_else(|| panic!("No runnable task after user fault termination"));

        self.current_task_idx = next_idx;
        self.tasks[next_idx].stack_ptr
    }

    pub fn stats(&self) -> SchedulerStats {
        let total_tasks = if self.main_task_tracked {
            self.tasks.len() + 1 // + kernel_idle virtual task
        } else {
            self.tasks.len() + 2 // + kernel_idle + pending initial user thread
        };

        let current_task_id = if self.main_task_tracked {
            self.tasks
                .get(self.current_task_idx)
                .map(|task| task.id.as_usize())
                .unwrap_or(0)
        } else {
            0
        };

        SchedulerStats {
            total_ticks: self.tick_count,
            total_tasks,
            current_task_id,
        }
    }

    pub fn process_snapshot(&self) -> Vec<ProcessSnapshot> {
        let mut list = Vec::new();

        list.push(ProcessSnapshot {
            id: 0,
            name: "kernel_idle",
            state: "idle",
            privilege: "kernel",
            run_ticks: 0,
        });

        if self.main_task_tracked {
            for (idx, task) in self.tasks.iter().enumerate() {
                list.push(ProcessSnapshot {
                    id: task.id.as_usize(),
                    name: task.name,
                    state: task.state_label(idx == self.current_task_idx),
                    privilege: task.privilege.label(),
                    run_ticks: task.run_ticks,
                });
            }
        } else {
            list.push(ProcessSnapshot {
                id: 1,
                name: "user_main",
                state: "running",
                privilege: "user",
                run_ticks: 0,
            });

            for task in &self.tasks {
                list.push(ProcessSnapshot {
                    id: task.id.as_usize(),
                    name: task.name,
                    state: task.state_label(false),
                    privilege: task.privilege.label(),
                    run_ticks: task.run_ticks,
                });
            }
        }

        list
    }
}

pub static SCHEDULER: Mutex<TaskManager> = Mutex::new(TaskManager::new());

pub fn scheduler_stats() -> SchedulerStats {
    SCHEDULER.lock().stats()
}

pub fn scheduler_process_snapshot() -> Vec<ProcessSnapshot> {
    SCHEDULER.lock().process_snapshot()
}

pub fn sleep_current_task(current_esp: u32, ticks: u32) -> bool {
    SCHEDULER.lock().sleep_current_task(current_esp, ticks)
}

pub fn kill_task(pid: usize) -> Result<KillTaskResult, &'static str> {
    SCHEDULER.lock().kill_task_by_pid(pid)
}

pub fn terminate_current_user_task_from_fault(current_esp: u32) -> u32 {
    SCHEDULER
        .lock()
        .terminate_current_user_task_from_fault(current_esp)
}

// Invoked directly from the assembly timer interrupt wrapper.
#[no_mangle]
pub extern "C" fn scheduler_tick(esp: u32) -> u32 {
    crate::pic::ack(32);
    SCHEDULER.lock().schedule_next(esp)
}
