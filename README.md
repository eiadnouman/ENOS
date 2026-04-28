# ENOS (Rust Microkernel)

A small experimental microkernel written in Rust.

## Features
- **Memory Management**: Custom allocator and paging.
- **Interrupts**: PIC, PIT, and IDT configured.
- **Multitasking**: Basic task scheduler with Ring 0 background threads and Ring 3 user tasks.
- **System Calls**: Interface between user space and kernel space.
- **Shell**: Interactive command-line interface.

## Build and Run
Requires `qemu-system-x86` and `cargo` with `rustup` nightly.

```bash
./build.sh
qemu-system-i386 -cdrom enos.iso
```
