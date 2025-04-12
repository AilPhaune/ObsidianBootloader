BITS 32
EXTERN rust_entry

SECTION .text

GLOBAL stage2_entry
stage2_entry:
    call rust_entry
    cli
    hlt
    jmp $

%include "asm/io.asm"
%include "../lib/bios.asm"