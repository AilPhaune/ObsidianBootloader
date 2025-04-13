BITS 32
EXTERN rust_entry

SECTION .text

GLOBAL stage3_entry
stage3_entry:
    call rust_entry
    cli
    hlt
    jmp $

%include "asm/io.asm"
%include "asm/bios.asm"
%include "asm/cpuid.asm"