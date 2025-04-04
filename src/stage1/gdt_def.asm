; https://wiki.osdev.org/Global_Descriptor_Table
gdt:
; NULL descriptor
gdt_null: 
    ; Null descriptor
    dq 0
gdt_code_segment:
    ; 32-bit code segment
    dw 0FFFFh                   ; limit (bits 0-15) = 0xFFFFF for full 32-bit range
    dw 0                        ; base (bits 0-15) = 0x0
    db 0                        ; base (bits 16-23)
    db 10011010b                ; access (present, ring 0, code segment, executable, direction 0, readable)
    db 11001111b                ; granularity (4k pages, 32-bit pmode) + limit (bits 16-19)
    db 0                        ; base high
gdt_data_segment:
    ; 32-bit data segment
    dw 0FFFFh                   ; limit (bits 0-15) = 0xFFFFF for full 32-bit range
    dw 0                        ; base (bits 0-15) = 0x0
    db 0                        ; base (bits 16-23)
    db 10010010b                ; access (present, ring 0, data segment, executable, direction 0, writable)
    db 11001111b                ; granularity (4k pages, 32-bit pmode) + limit (bits 16-19)
    db 0                        ; base high
; gdt_code_segment16:
;     ; 16-bit code segment
;     dw 0FFFFh                   ; limit (bits 0-15) = 0xFFFFF
;     dw 0                        ; base (bits 0-15) = 0x0
;     db 0                        ; base (bits 16-23)
;     db 10011010b                ; access (present, ring 0, code segment, executable, direction 0, readable)
;     db 00001111b                ; granularity (1b pages, 16-bit pmode) + limit (bits 16-19)
;     db 0                        ; base high
; gdt_data_segment16:
;     ; 16-bit data segment
;     dw 0FFFFh                   ; limit (bits 0-15) = 0xFFFFF
;     dw 0                        ; base (bits 0-15) = 0x0
;     db 0                        ; base (bits 16-23)
;     db 10010010b                ; access (present, ring 0, data segment, executable, direction 0, writable)
;     db 00001111b                ; granularity (1b pages, 16-bit pmode) + limit (bits 16-19)
;     db 0                        ; base high
gdt_end:

gdt_descriptor:
    dw gdt_end - gdt - 1
    dd gdt

gdt_null_selector EQU gdt_null - gdt
gdt_code_selector EQU gdt_code_segment - gdt
gdt_data_selector EQU gdt_data_segment - gdt