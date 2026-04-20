ENTRY(_start)

SECTIONS {
    . = 1M;
    _kernel_start = .;

    .multiboot : {
        *(.multiboot)
    }

    .text : {
        *(.text*)
    }

    .data : {
        *(.data*)
    }

    .bss : {
        *(.bss*)
    }
    
    _kernel_end = .;
}

