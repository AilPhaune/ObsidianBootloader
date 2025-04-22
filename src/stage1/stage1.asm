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

    call read_drive_parameters
    jc .err
    
    ; ebx = BYTES_PER_SECTOR * 64 / 16 (reading 64 sectors of BYTES_PER_SECTOR bytes, 16 bytes offset = 1 segment offset)
    mov bx, word [disk_parameters_struct.dps_bytes_per_sector]
    shl bx, 2

    mov ecx, 5
    mov word [disk_address_packet.dap_dest_segment], 0x07c0
    mov word [disk_address_packet.dap_dest_offset], 0x0000
    mov word [disk_address_packet.dap_num_sectors_read], 64
    mov dword [disk_address_packet.dap_lba_lo], 34
.loop:
    add word [disk_address_packet.dap_lba_lo], 64
    add word [disk_address_packet.dap_dest_segment], bx

    pusha
    call read_sectors
    jc .err
    popa
    dec ecx
    cmp ecx, 0
    jne .loop

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
.err:
    mov si, msg_disk_error
    call puts
stage1_fail:
    cli
    hlt
    jmp stage1_fail

msg_load_gdt: db "Loading GDT", CR, ENDL, 0
msg_disk_error: db "Disk error", CR, ENDL, 0

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