org 0x7c00
bits 16

stage1_entry:
    cli
    ; Setup segment registers
    xor ax, ax
    mov cs, ax
    mov ds, ax
    mov es, ax

    ; Setup stack
    mov ss, ax
    mov sp, 0x7c00
    sti

    mov si, msg_load_gdt
    call puts

    lgdt [gdt_descriptor]

    ; Enable A20
    in al, 0x92
    or al, 2
    out 0x92, al

    cli

    ; Load GDT

    ; Enable PE
    mov eax, cr0
    or eax, 1
    mov cr0, eax
    
    jmp 0x08:stage1_pmode

msg_load_gdt: db "Loading GDT", CR, ENDL, 0

%include "./src/stage1/gdt_def.asm"
%include "./src/stage1/print.asm"

ENDL EQU 10
CR EQU 13

bits 32
stage1_pmode:
    mov eax, gdt_data_selector
    mov ds, ax
    mov ss, ax
    mov es, ax
    mov fs, ax
    mov gs, ax

    mov ebp, 0x7c00
    mov esp, ebp
    
    and edx, 0xFF
    push edx
    
    sidt [idt_store]
    mov eax, [idt_store]
    push eax

    jmp 0x9000

    pop eax
    pop edx

stage1_end:
    cli
    hlt
    jmp $

idt_store:
    dq 0