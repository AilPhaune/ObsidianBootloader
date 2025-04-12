; Checks extended disk features
; Parameters:
;    dl: drive number
; Returns:
;    al: 1 if extended disk, 0 otherwise
; Modifies:
;    Nothing
check_extended_disk:
    push bx
    push cx
    clc
    mov ah, 0x41
    mov bx, 0x55AA
    int 0x13

    jc .no_edd
    cmp bx, 0xAA55
    pop bx
    jne .no_edd

    and cx, 0b111
    cmp cx, 0b111
    pop cx
    jne .no_edd

    mov al, 1
    ret

.no_edd:
    mov al, 0
    ret

; Reads sectors from disk. Some BIOSES are limited to 127 or 128 in a single call !!
; Parameters:
;    dl:    drive number
;    disk_address_packet
; Returns:
;    ah:    error code if carry flag is set
; Modifies:
;    /!\ DON'T TRUST THE BIOS
;   (in theory: Nothing)
read_sectors:
    mov ah, 0x42
    mov si, disk_address_packet
    clc
    int 0x13
    ret

; Reads many sectors from disk
; Parameters:
;    dl = drive number
;    eax = LBA start (32-bit)
;    ecx = total sectors to read
;    es:0 = destination pointer
; Returns:
;    Nothing
; Modifies:
;    Nothing
read_many_sectors:
    pusha

.next_chunk:
    cmp ecx, 64
    jge .next_chunk_64
    cmp ecx, 0
    je .end
    mov esi, ecx
    jmp .next_chunk_num

.next_chunk_64:
    mov esi, 64

.next_chunk_num:
    mov word [disk_address_packet.dap_num_sectors_read], si
    mov word [disk_address_packet.dap_dest_offset], 0
    mov word [disk_address_packet.dap_dest_segment], es
    mov dword [disk_address_packet.dap_lba_lo], eax
    mov dword [disk_address_packet.dap_lba_hi], 0

    pusha
    call read_sectors
    jc .fail
    popa

    push edx
    xor edx, edx
    mov dx, si
    add eax, edx
    sub ecx, edx
    pop edx

    ; Update es pointer
    push dx
    push ax
    mov ax, si
    mov dx, es
    shl ax, 5   ; Multiply by 32 = SECTOR SIZE / 16 = 512 / 16, because 1 segment offset is 16 bytes offset
    add dx, ax
    mov es, dx
    pop ax
    pop dx

    jmp .next_chunk

.end:
    popa
    clc
    ret

.fail:
    popa
    stc
    ret

disk_address_packet:
    .dap_size:              db 0x10
    .dap_null:              db 0
    .dap_num_sectors_read:  dw 0
    .dap_dest_offset:       dw 0
    .dap_dest_segment:      dw 0
    .dap_lba_lo:            dd 0
    .dap_lba_hi:            dd 0