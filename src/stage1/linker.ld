ENTRY(stage1_entry)

SECTIONS
{
    . = 0x7c00;
    .text : {
        *(.text.stage1_entry)
        *(.text*)
    }
}