[GLOBAL setup_idt]
[GLOBAL interrupt_return]
[GLOBAL idt]

[EXTERN int_disp]

[SECTION .text]
[BITS 64]

%macro wrapper 1
wrapper_%1:
	push rbp
	mov rbp, rsp
	push rax
	mov al, %1
	jmp wrapper_body
%endmacro

%assign i 0
%rep 256
wrapper i
%assign i i + 1
%endrep

; Common interrupt routine body
wrapper_body:
	; Expected by gcc
	cld

	; Save registers
	push rcx
	push rdx
	push rdi
	push rsi
	push r8
	push r9
	push r10
	push r11

	; Wrapper only uses 8 bits
	and rax, 0xff

	; Call dispatcher with interrupt number as parameter
	mov rdi, rax
	call int_disp

interrupt_return:
	; Restore registers
	pop r11
	pop r10
	pop r9
	pop r8
	pop rsi
	pop rdi
	pop rdx
	pop rcx

	; ...also from wrapper
	pop rax
	pop rbp

	; Return from interrupt routine
	iretq

setup_idt:
	mov rax, wrapper_0

	; Bits 0..15 -> ax, 16..31 -> bx, 32..64 -> edx
	mov rbx, rax
	mov rdx, rax
	shr rdx, 32
	shr rbx, 16

	mov r10, idt   ; Pointer to current interrupt gate
	mov rcx, 255   ; Counter
.loop:
	add [r10 + 0], ax
	adc [r10 + 6], bx
	adc [r10 + 8], edx
	add r10, 16
	dec rcx
	jge .loop

	lidt [idt_descr]
	ret

[SECTION .data]

idt:
%macro idt_entry 1
	dw (wrapper_%1 - wrapper_0) & 0xffff ; Offset 0 -> 15
	dw 0x0008 ; Selector points to 64-Bit CS descriptor of GDT
	dw 0x8e00 ; 8 -> interrupt is present, e -> 80386 64-bit interrupt gate
	dw ((wrapper_%1 - wrapper_0) & 0xffff0000) >> 16 ; Offset 16 .. 31
	dd ((wrapper_%1 - wrapper_0) & 0xffffffff00000000) >> 32 ; Offset 32..63
	dd 0x00000000 ; Reserved
%endmacro

%assign i 0
%rep 256
idt_entry i
%assign i i+1
%endrep

idt_descr:
	dw 256 * 16 - 1 ; 256 entries
	dq idt