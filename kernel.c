// Multiboot header
__attribute__((section(".multiboot")))
const unsigned int multiboot_header[] = {
    0x1BADB002,        // magic number
    0x0,               // flags
    -(0x1BADB002)      // checksum
};

void kernel_main() {
    volatile unsigned short* video = (unsigned short*) 0xB8000;

    for (int i = 0; i < 80 * 25; i++) {
        video[i] = 0x1F41; 
    }

    while (1);
}

