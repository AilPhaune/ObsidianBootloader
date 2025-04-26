EXTERN GDTR

addr_64:
    .lo: dd 0
    .hi: dd 0
sp_64:
    .lo: dd 0
    .hi: dd 0
memory_layout_ptr:
    dd 0
memory_layout_entries:
    dd 0
page_allocator_ptr:
    dd 0
page_allocator_end:
    dd 0
begin_usable_memory:
    dd 0

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

    mov eax, [esp + 24] ; stack pointer lo
    mov [sp_64.lo], eax
    mov eax, [esp + 28] ; stack pointer hi
    mov [sp_64.hi], eax

    mov eax, [esp + 32] ; memory layout pointer
    mov [memory_layout_ptr], eax

    mov eax, [esp + 36] ; memory layout entries count
    mov [memory_layout_entries], eax

    mov eax, [esp + 40] ; Page allocator current pointer
    mov [page_allocator_ptr], eax

    mov eax, [esp + 44] ; Page allocator end pointer
    mov [page_allocator_end], eax

    mov eax, [esp + 48] ; begin usable memory
    mov [begin_usable_memory], eax

    mov eax, [esp + 12] ; 64-bit code selector
    push eax
    push dword .lmode64
    retf
.lmode64:
    [bits 64]
    mov rsp, [sp_64]
    mov rbp, rsp
    
    ; Arguments
    xor rax, rax
    
    mov eax, [memory_layout_ptr]
    mov rdi, rax

    mov eax, [memory_layout_entries]
    mov rsi, rax

    mov rax, cr3
    mov rdx, rax

    mov eax, [page_allocator_ptr]
    mov rcx, rax

    mov eax, [page_allocator_end]
    mov r8, rax

    mov eax, [begin_usable_memory]
    mov r9, rax

    ; Call 64-bit kernel entry
    mov rbx, [addr_64]
    call rbx

    cli
    hlt
    jmp $
    [bits 32]