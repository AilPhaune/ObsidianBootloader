EXTERN GDTR

addr_64:
    .lo: dd 0
    .hi: dd 0    

GLOBAL enable_paging_and_jump64
enable_paging_and_jump64:
    [bits 32]
    lgdt [GDTR]

    ; Disable paging
    mov ebx, cr0
    and ebx, ~(1 << 31)
    mov cr0, ebx

    ; Enable PAE
    mov edx, cr4
    or  edx, (1 << 5)
    mov cr4, edx

    ; Set LME (long mode enable)
    mov ecx, 0xC0000080
    rdmsr
    or  eax, (1 << 8)
    wrmsr

    ; Load PML4
    mov eax, [esp + 4] ; PML4 ptr
    mov cr3, eax

    ; Enable paging
    or ebx, (1 << 31) | (1 << 0)
    mov cr0, ebx

    mov eax, [esp + 8] ; 64-bit data selector
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax

    mov eax, [esp + 16] ; entry64 lo
    mov [addr_64.lo], eax
    mov eax, [esp + 20] ; entry64 hi
    mov [addr_64.hi], eax

    mov eax, [esp + 12] ; 64-bit code selector
    push eax
    push dword .lmode64
    retf
.lmode64:
    [bits 64]
    mov rbx, [addr_64]
    call rbx

    cli
    hlt
    jmp $
    [bits 32]