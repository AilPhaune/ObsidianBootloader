; Write a byte to an I/O port
; Parameters:
;   [esp + 4]: Port address (16-bit)
;   [esp + 8]: Value to write (8-bit)
GLOBAL outb
outb:
    movzx edx, word [esp + 4]   ; Load port address into DX (16-bit)
    movzx eax, byte [esp + 8]   ; Load value to write into AL (8-bit)
    out dx, al                  ; Output the byte in AL to the port in DX
    ret                         ; Return from function

; Read a byte from an I/O port
; Returns:
;   AL: The value read from the port
;   [esp + 4]: Port address (16-bit)
GLOBAL inb
inb:
    xor eax, eax                ; Clear eax
    movzx edx, word [esp + 4]   ; Load port address into DX (16-bit)
    in al, dx                   ; Input a byte from the port in DX into AL
    ret                         ; Return from function

; Arguments: 
; - [esp + 4] = port (16-bit)
; - [esp + 8] = value (16-bit)
GLOBAL outw
outw:
    movzx edx, word [esp + 4]
    mov eax, [esp + 8]
    out dx, eax
    ret

; Arguments:
; - [esp + 4] = port (16-bit)

; Load port number from stack (esp + 4) into edx
GLOBAL inw
inw:
    xor eax, eax
    movzx dx, [esp + 4]
    in ax, dx
    ret

; Arguments: 
; - [esp + 4] = port (16-bit)
; - [esp + 8] = value (32-bit)
GLOBAL outl
outl:
    mov edx, dword [esp + 4]
    mov eax, [esp + 8]
    out dx, eax
    ret

; Arguments:
; - [esp + 4] = port (16-bit)

; Load port number from stack (esp + 4) into edx
GLOBAL inl
inl:
    movzx dx, [esp + 4]
    in eax, dx
    ret