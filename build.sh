#!/bin/bash
set -euo pipefail

missing=0

require_cmd() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo "Missing dependency: $1" >&2
        missing=1
    fi
}

require_cmd cargo

if [ "$missing" -ne 0 ]; then
    exit 127
fi

if command -v grub-mkrescue >/dev/null 2>&1; then
    GRUB_MKRESCUE=grub-mkrescue
elif command -v grub2-mkrescue >/dev/null 2>&1; then
    GRUB_MKRESCUE=grub2-mkrescue
else
    echo "Missing dependency: grub-mkrescue or grub2-mkrescue" >&2
    exit 127
fi

echo "Building Rust Microkernel..."
cargo +nightly build -Z build-std=core,compiler_builtins,alloc -Z json-target-spec --target i686-enos.json

echo "Copying to isodir..."
mkdir -p isodir/boot
cp target/i686-enos/debug/enos isodir/boot/kernel.bin

echo "Generating ISO..."
$GRUB_MKRESCUE -o enos.iso isodir

echo "Done! You can now run: qemu-system-i386 -cdrom enos.iso"
