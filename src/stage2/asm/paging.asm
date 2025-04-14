EXTERN GDTR

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

    mov eax, [esp + 12] ; 64-bit code selector
    mov ebx, [esp + 16] ; entry64

    push eax
    push ebx
    retf