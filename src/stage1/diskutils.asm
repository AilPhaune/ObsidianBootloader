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

; Reads sectors from disk
; Parameters:
;    dl:    drive number
;    disk_address_packet
; Returns:
;    ah:    error code
; Modifies:
;    Nothing
read_sectors:
    mov ah, 0x42
    mov si, disk_address_packet
    clc
    int 0x13
    ret

disk_address_packet:
    .dap_size:              db 0x10
    .dap_null:              db 0
    .dap_num_sectors_read:  dw 0
    .dap_dest_segment:      dw 0
    .dap_dest_offset:       dw 0
    .dap_lba_lo:            dd 0
    .dap_lba_hi:            dd 0