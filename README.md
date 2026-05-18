# ENOS

ENOS is a small educational x86 kernel written in Rust. It boots through GRUB using the Multiboot 1 protocol, runs without the Rust standard library, and demonstrates the core pieces of a protected-mode operating system: descriptor tables, interrupts, paging, a simple scheduler, a syscall gate, a shell, and an in-memory filesystem.

## Current Features

- 32-bit x86 `no_std` kernel targeting `i386-unknown-none`.
- Multiboot boot path with a custom linker script.
- VGA text output and serial logging on COM1.
- GDT setup with kernel/user segments and a TSS for Ring 3 to Ring 0 transitions.
- IDT setup for CPU exceptions, timer, keyboard, page faults, and `int 0x80` syscalls.
- PIC remapping and PIT timer configuration at 100 Hz.
- Identity-mapped kernel pages and a separate user-accessible 4 MiB page range.
- Simple heap allocator and physical-frame bump allocator.
- Preemptive scheduler with task snapshots, sleeping, and user-task termination hooks.
- Ring 3 demo program that prints through a syscall and sleeps.
- Interactive shell with process, memory, identity, calculator, and RAM filesystem commands.

## Repository Layout

- `src/main.rs` - boot entry, kernel initialization, Ring 3 demo install, and mode switch.
- `src/gdt.rs` - global descriptor table and TSS setup.
- `src/idt.rs` and `src/interrupts.rs` - IDT entries, exception handlers, IRQ wrappers, and syscall wrapper.
- `src/pic.rs` and `src/pit.rs` - interrupt controller and timer setup.
- `src/memory.rs`, `src/paging.rs`, and `src/allocator.rs` - memory map parsing, paging, and kernel heap allocation.
- `src/task.rs` - simple preemptive task manager and scheduler state.
- `src/shell.rs` - keyboard input handling and shell commands.
- `src/fs.rs` - small RAM-backed filesystem.
- `src/vga_buffer.rs` and `src/serial.rs` - output drivers.
- `i686-enos.json` - custom Rust target specification.
- `link.ld` - kernel linker script.
- `isodir/boot/grub/grub.cfg` - GRUB ISO entry.
- `build.sh` - builds the kernel and produces `enos.iso`.

## Requirements

- Rust nightly toolchain with `rust-src`.
- `cargo` and `rustup`.
- `grub-mkrescue` or `grub2-mkrescue`.
- `xorriso` or the platform package required by GRUB rescue image creation.
- `qemu-system-i386` for local execution.

On many Linux distributions the non-Rust dependencies come from packages similar to:

```sh
grub-pc-bin grub-common xorriso qemu-system-x86
```

On Fedora:

```sh
sudo dnf install rustup grub2-tools-extra xorriso qemu-system-i386
rustup toolchain install nightly
rustup component add rust-src --toolchain nightly
```

## Build

```sh
./build.sh
```

The script runs:

```sh
cargo +nightly build -Z build-std=core,compiler_builtins,alloc -Z json-target-spec --target i686-enos.json
grub-mkrescue -o enos.iso isodir
```

The output ISO is generated as `enos.iso`.

## Run

```sh
qemu-system-i386 -cdrom enos.iso -serial file:serial.log
```

Useful debug run:

```sh
qemu-system-i386 -cdrom enos.iso -serial stdio -d int,cpu_reset -D qemu_debug.log
```

## Shell Commands

Inside ENOS, type `help` to list built-in commands. Current commands include:

- `help`, `clear`, `about`
- `echo <msg>`, `calc <expr>`
- `uptime`, `ps`, `top`, `tasks`
- `meminfo`, `fsinfo`
- `whoami`, `id`, `login <enos|guest>`, `su <enos|guest>`, `logout`
- `kill <pid>`
- `touch <file>`, `rm <file>`, `ls`, `write <file> <text>`, `cat <file>`

## Development Notes

This project intentionally keeps many subsystems small and readable. Some pieces are teaching implementations rather than production designs:

- The kernel heap is a bump allocator and does not reclaim freed memory.
- Paging is currently static and maps only the initial kernel/user ranges.
- The RAM filesystem is volatile and disappears on reboot.
- The Ring 3 program is generated directly into the user page range at boot.

Generated files such as `target/`, `enos.iso`, `kernel.bin`, `*.o`, and logs should not be committed.
