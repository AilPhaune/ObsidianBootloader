ENTRY(stage2_entry)

SECTIONS
{
    . = 0x7e00; /* Start address */

    .text : {
        *(.text.stage2_entry)
        *(.text.unsafe_call_bios_interrupt)
        main.o(.text*)
        *(.text.rust_entry)
    }

    .text : {
        libstage2.a(.text*)
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

    . = ALIGN(0x1000);

    /DISCARD/ : {
        *(.got*)
        *(.got.plt*)
        *(.comment*)
    }
}