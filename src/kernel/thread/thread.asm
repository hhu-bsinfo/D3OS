[GLOBAL thread_kernel_start]
[GLOBAL thread_user_start]
[GLOBAL thread_switch]
[GLOBAL thread_set_segment_register]

[EXTERN tss_set_rsp0]

[SECTION .text]
[BITS 64]

thread_kernel_start:
    mov rsp, rdi    ; First parameter -> load 'old_rsp0'
    pop rbp
    pop rdi         ; 'old_rsp0' is here
    pop rsi
    pop rdx
    pop rcx
    pop rbx
    pop rax
    pop r15
    pop r14
    pop r13
    pop r12
    pop r11
    pop r10
    pop r9
    pop r8
    popf
    retq

thread_switch:
    ; Save registers of current thread
    pushf
    push r8
    push r9
    push r10
    push r11
    push r12
    push r13
    push r14
    push r15
    push rax
    push rbx
    push rcx
    push rdx
    push rsi
    push rdi
    push rbp

    ; Save stack pointer in 'current_rsp0' (first parameter)
    mov [rdi], rsp

    ; Set rsp0 of kernel stack in tss (3. parameter 'next_rsp0_end')
    mov rdi, rdx
    call tss_set_rsp0

    ; Load registers of next thread by using 'next_rsp0' (second parameter)
    mov rsp, rsi
    pop rbp
    pop rdi
    pop rsi
    pop rdx
    pop rcx
    pop rbx
    pop rax
    pop r15
    pop r14
    pop r13
    pop r12
    pop r11
    pop r10
    pop r9
    pop r8
    popf

    retq    ; Return to next thread

thread_user_start:
    mov rsp, rdi                ; Load 'old_rsp' (first parameter)
    pop rdi
    iretq                       ; Switch to user-mode