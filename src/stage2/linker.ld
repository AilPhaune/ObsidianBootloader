ENTRY(stage3_entry)

SECTIONS
{
    . = 0x9000; /* Start address */

    .text : {
        *(.text.stage3_entry)
        *(.text*)
    }

    .rodata : {
        *(.rodata*)
    }

    .data : {
        *(.data*)
    }

    .bss : {
        bss_start = .;
        *(.bss*)
        bss_end = .;
    }
}