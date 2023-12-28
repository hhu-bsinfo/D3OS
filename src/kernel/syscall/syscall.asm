[GLOBAL init_syscalls]
[GLOBAL syscall_handler]

[EXTERN syscall_disp]
[EXTERN syscall_abort]
[EXTERN tss_get_rsp0]

[SECTION .text]
[BITS 64]

; Maximum system call ID (must be consistent with NUM_SYSCALLS in 'kernel/syscall/user_api/mod.rs')
NUM_SYSCALLS equ 4

syscall_handler:
    ; We are now in ring 0, but still on the user stack
    ; Disable interrupts until we have switched to kernel stack
    cli

    ; Save registers (except rax, which is used for system call ID and return value)
    push   rbx
    push   rcx ; Contains rip for returning to ring 3
    push   rdx
    push   rdi
    push   rsi
    push   r8
    push   r9
    push   r10
    push   r11 ; Contains eflags for returning to ring 3
    push   r12
    push   r13
    push   r14
    push   r15

    ; Switch to kernel stack and enable interrupts
    mov r15, rax ; Save system call ID in r15
    mov r14, rdi ; Save first parameter in r14
    mov r13, rsi ; Save second parameter in r13
    mov r12, rdx ; Save third parameter in r12
    call tss_get_rsp0 ; Get kernel rsp (returned in rax)
    mov rbx, rax ; Save kernel rsp in rbx
    mov rcx, rsp ; Save user rsp in rcx
    mov rdx, r12 ; Restore third parameter
    mov rsi, r13 ; Restore second parameter
    mov rdi, r14 ; Restore first parameter
    mov rax, r15 ; Restore system call ID
    mov rsp, rbx ; Switch to kernel stack
    push rcx ; Save user rsp on stack
    sti

    ; Check if system call ID is in bounds
    cmp rax, NUM_SYSCALLS
    jge syscall_abort ; Panics and does not return

    ; Call system call handler, corresponding to ID (in rax)
    call syscall_disp

    ; Switch to user stack (user rsp is last value on stack)
    ; Disable interrupts, since we are still in Ring 0 and no interrupt handler should be called with the user stack
    cli
    pop rsp

    ; Restore registers
    pop    r15
    pop    r14
    pop    r13
    pop    r12
    pop    r11 ; Contains eflags for returning to ring 3
    pop    r10
    pop    r9
    pop    r8
    pop    rsi
    pop    rdi
    pop    rdx
    pop    rcx ; Contains rip for returning to ring 3
    pop    rbx

    ; Return to Ring 3
    ; Interrupts will be enabled automatically, because eflags gets restored from r11
    o64 sysret