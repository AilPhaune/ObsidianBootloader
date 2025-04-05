protected_idt:
    dq 0
protected_gdt:
    dq 0
idt_addr:
    dq 0
temp_eax:
    dd 0
temp_ebx:
    dd 0
temp_ecx:
    dd 0
temp_edx:
    dd 0
temp_esi:
    dd 0
temp_edi:
    dd 0
temp_eflags:
    dd 0
temp_ds:
    dd 0
temp_es:
    dd 0
temp_fs:
    dd 0
temp_gs:
    dd 0

bios_interrupt:
    [bits 16]
    db 0xCD
.interrupt_num:
    db 0
    ret

GLOBAL unsafe_call_bios_interrupt
unsafe_call_bios_interrupt:
    [bits 32]
    cli
    sidt [protected_idt]
    sgdt [protected_gdt]

    push ebp
    mov ebp, esp

    ; Save registers
    pushad
    pushfd

    ; Get parameters
    mov eax, [ebp + 8]     ; bios_idt address
    mov ebx, [ebp + 12]    ; interrupt number (8-bit, stored in BX)
    
    mov ecx, [ebp + 16]
    mov dword [temp_eax], ecx
    mov ecx, [ebp + 20]
    mov dword [temp_ebx], ecx
    mov ecx, [ebp + 24]
    mov dword [temp_ecx], ecx
    mov ecx, [ebp + 28]
    mov dword [temp_edx], ecx
    mov ecx, [ebp + 32]
    mov dword [temp_esi], ecx
    mov ecx, [ebp + 36]
    mov dword [temp_edi], ecx
    mov ecx, [ebp + 40]
    mov dword [temp_es], ecx
    mov ecx, [ebp + 44]
    mov dword [temp_ds], ecx
    mov ecx, [ebp + 48]
    mov dword [temp_fs], ecx
    mov ecx, [ebp + 52]
    mov dword [temp_gs], ecx

    mov byte [bios_interrupt.interrupt_num], bl
    mov [idt_addr], eax        ; store BIOS IDT pointer

    jmp word 18h:.pmode16
.pmode16:
    [bits 16]
    ; DISABLE PROTECTED MODE
    mov eax, cr0
    and al, ~1
    mov cr0, eax

    jmp word 00h:.rmode
.rmode:
    [bits 16]
    xor eax, eax
    mov ds, ax
    mov es, ax
    mov ss, ax

    ; LOAD BIOS IDT:
    lidt [ds:idt_addr]

    ; LOAD GENERAL PURPOSE REGISTERS
    mov eax, [ds:temp_eax]
    mov ebx, [ds:temp_ebx]
    mov ecx, [ds:temp_ecx]
    mov edx, [ds:temp_edx]
    mov esi, [ds:temp_esi]
    mov edi, [ds:temp_edi]

    ; LOAD SEGMENT REGISTERS
    push eax
    mov eax, [ds:temp_es]
    mov es, ax
    mov eax, [ds:temp_fs]
    mov fs, ax
    mov eax, [ds:temp_gs]
    mov gs, ax
    mov eax, [ds:temp_ds]
    mov ds, ax
    pop eax

    clc
    sti
    call bios_interrupt
    cli
    pushfd

    ; RESET SEGMENT REGISTERS
    push eax
    mov eax, 0
    mov ds, ax
    mov es, ax
    pop eax

    ; SAVE GENERAL PURPOSE REGISTERS
    mov [ds:temp_eax], eax
    mov [ds:temp_ebx], ebx
    mov [ds:temp_ecx], ecx
    mov [ds:temp_edx], edx
    mov [ds:temp_esi], esi
    mov [ds:temp_edi], edi

    ; SAVE EFLAGS
    pop eax
    mov [ds:temp_eflags], eax

    lgdt [protected_gdt]

    ; ENABLE PROTECTED MODE
    mov eax, cr0
    or al, 1
    mov cr0, eax
    jmp word 0x08:.pmode32
.pmode32:
    [bits 32]
    mov eax, 0x10
    mov ds, ax
    mov ss, ax
    mov es, ax
    mov fs, ax
    mov gs, ax

    popfd
    popad

    ; LOAD PROTECTED MODE IDT
    lidt [protected_idt]

    ; RETURN POINTER
    mov eax, temp_eax

    mov esp, ebp
    pop ebp

    ret