SECTION .text
BITS 16

GLOBAL stage1_entry
stage1_entry:
    cli
    ; Setup segment registers
    xor ax, ax
    mov cs, ax
    mov ds, ax
    mov es, ax
    mov [boot_drive], dl

    ; Setup stack
    mov ss, ax
    mov sp, 0x7c00

    pusha

    ; Load stage 2
    mov ax, 0x07e0
    mov es, ax              ; Destination segment (start at 0x07e00:0x0000 ==> [0x7E00, 0x7FFFF])
    mov eax, 35             ; LBA
    mov ecx, 961            ; Number of sectors to read, just enough to fit in the usable memory area after the bootloader (https://wiki.osdev.org/Memory_Map_(x86)#Overview)
    call read_many_sectors
    jc .fail_read_many

    popa

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
    
.fail_read_many:
    mov si, msg_fail_read_many
    call puts
    cli
    hlt
    jmp $

msg_load_gdt: db "Loading GDT", CR, ENDL, 0
msg_fail_read_many: db "Failed to read sectors", CR, ENDL, 0
boot_drive: db 0

%include "./src/stage1/gdt_def.asm"
%include "./src/stage1/print.asm"
%include "./src/stage1/diskutils.asm"

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

    jmp 0x7e00

    pop eax
    pop edx

stage1_end:
    cli
    hlt
    jmp $

idt_store:
    dq 0