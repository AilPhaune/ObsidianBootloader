; Bootloader
org 0x7c00
bits 16

; Some BIOSes put the bootloader at 07c0:0000
_start:
    jmp 0x0000:start

start:
    cli
    ; Setup segment registers
    xor ax, ax
    mov cs, ax
    mov ds, ax
    mov es, ax

    ; Setup stack
    mov ss, ax
    mov sp, 0x7a00
    sti

; Relocate at 0x7a00
relocate:
    cld
    mov si, 0x7c00
    mov di, 0x7a00
    mov cx, 512 / 2
    rep movsw

    ; Jump to label
    mov ax, .end_relocate
    sub ax, 512
    push ax
    ret

.end_relocate:
    call check_extended_disk
    cmp al, 1
    jne no_edd

    mov si, msg_starting
    call puts

    ; Read stage 1 at 0x7c00
    mov word [disk_address_packet.dap_dest_segment], 0x07c0
    mov word [disk_address_packet.dap_dest_offset], 0x0000
    mov word [disk_address_packet.dap_num_sectors_read], STAGE1_SIZE
    mov dword [disk_address_packet.dap_lba_lo], 34
    call read_sectors

    jmp 0x0000:0x7c00

no_edd:
    mov si, msg_no_edd
    call puts

end:
    cli
    jmp $
    hlt

%include "./src/boot/diskutils.asm"
%include "./src/boot/print.asm"

msg_no_edd: db "No EDD", CR, ENDL, 0
msg_starting: db "Loading stage 1", CR, ENDL, 0

; Signature
times 510-($-$$) db 0
dw 0xaa55

ENDL EQU 10
CR EQU 13

STAGE1_SIZE EQU 1