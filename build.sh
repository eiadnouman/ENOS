#!/bin/bash
set -e

echo "Building Rust Microkernel..."
# We explicitly set rustup default inside just in case
cargo +nightly build -Z build-std=core,compiler_builtins,alloc -Z json-target-spec --target i686-enos.json

# Check if build was successful
if [ $? -eq 0 ]; then
    echo "Copying to isodir..."
    cp target/i686-enos/debug/enos isodir/boot/kernel.bin
    
    echo "Generating ISO..."
    grub-mkrescue -o enos.iso isodir
    
    echo "Done! You can now run: qemu-system-i386 -cdrom enos.iso"
else
    echo "Build failed!"
    exit 1
fi
