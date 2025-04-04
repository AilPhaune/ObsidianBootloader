; Uses bios to print a string
; Parameters:
;    si: string address
; Returns:
;    Nothing
; Modifies:
;    si
puts:
    push ax
    mov ah, 0xE
.loop:
    lodsb
    cmp al, 0
    je .end_loop
    int 0x10
    jmp .loop
.end_loop:
    pop ax
    ret