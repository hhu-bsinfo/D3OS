;**********************************************************************
;*                                                                    *
;*                  B O O T _ B P                                     *
;*                                                                    *
;*--------------------------------------------------------------------*
;* Description:     This is the boot code for all application         *
;*                  processors. The boostrap processor sends in an    *
;*                  IPI (= Inter-Processor-Interrupt) in 'boot.rs' to *
;*                  all application processors with the addess of the *
;*                  function 'boot_ap'. The bootstrap processor will  *
;*                  have already done further initialization in Rust  *
;*                  and relocated the boot code for application       *
;*                  processors using 'ap_boot_region' and the linker  *
;*                  labels '___BOOT_AP_START__' & '___BOOT_AP_END__'. *
;*                                                                    *
;*                  All application processors start their execution  *
;*                  in real mode and we need to switch to protected   *
;*                  mode first. Afterwards we switch to long mode by  +
;*                  calling the 32 bit protected mode code entry      *
;*                  function 'start_asm'. It will setup the GDT, page *
;*                  tables, a stack, switch to long mode and call     *
;*                  'setup_ap_idt' followed by 'startup_ap',          *
;*                  the first Rust functions.                         *
;*                                                                    *
;*                  We reserve 6400 KB for all stacks. Each processor *
;*                  gets 400 KB for its stack. This will limit the    *
;*                  number of supported cores to 16.                  *
;*                                                                    *
;* Autor:           Michael Schoettner, Univ. Duesseldorf, 25.8.2022  *
;* Modified by:     Alex Wantz, Univ. Duesseldorf, 14.8.2025          *
;**********************************************************************

; Stack
STACK_MEM_SIZE: equ 6553600	; totale Groesse für alle AP Stacks (16)
STACK_SIZE_ONE: equ 409600	; Stack Groesse von einem einzelnem AP

; In 'boot.rs' - hier starten die AP's
[EXTERN startup_ap]
; In 'interrupt_dispatcher.rs'
[EXTERN setup_ap_idt]
; In 'linker.ld'
[EXTERN ___BOOT_AP_START__]
[EXTERN ___BOOT_AP_END__]


[global ___KERNEL_CR3__]


; The Segment '.boot_seg_ap' will be relocated by the bootstrap proc.
; grub loads the code above 1 MB but real mode code needs to be <1 MB
[SECTION .boot_seg_ap]

; 'boot_ap' is the entry function for all application processors
USE16
boot_ap:
	; Initialize segment registers
	mov ax, cs 		; Same segment for code and data
	mov ds, ax	 	; For the time being we need no stack

	cli
	mov al, 0x80
	out 0x70, al   	; disable NMI

	; Set GDT
	lgdt [gdtd_ap - boot_ap]

	; Switch to protected mode
	mov eax, cr0 	; Set PM bit in control register cr0
	or  eax, 1
	mov cr0, eax

	; Far jump to load cs
	jmp dword 0x08:boot_ap32


; GDT for application processors (will be replaced later)
; Attentation: This GDT is in the segment '.boot_seg_ap' because it
; will be copied below 1 MB during code relocation
align 4

gdt_ap:
	dw  0,0,0,0   ; NULL descriptor

	; 32 bit code segment deskriptor
	dw  0xFFFF    ; 4Gb - (0x100000*0x1000 = 4Gb)
	dw  0x0000    ; base address=0
	dw  0x9A00    ; code read/exec
	dw  0x00CF    ; granularity=4096, 386 (+5th nibble of limit)

	; 32 bit data segment deskriptor
	dw  0xFFFF    ; 4Gb - (0x100000*0x1000 = 4Gb)
	dw  0x0000    ; base address=0
	dw  0x9200    ; data read/write
	dw  0x00CF    ; granularity=4096, 386 (+5th nibble of limit)

; Limit and address of GDT, needed to load GDT
gdtd_ap:
	dw  3*8 - 1   ; GDT Limit=24, 4 GDT entries - 1
	dd  gdt_ap    ; Adress of GDT


[SECTION .text]

USE32
; 32 bit protected mode from here
; setting up segment registers and continuing to 'start_asm'
boot_ap32:
	mov ax, 0x10
	mov ds, ax
	mov es, ax
	mov fs, ax
	mov gs, ax
	mov ss, ax

; 32 bit entry function for all application processors
; Sets up GDT and a stack
[BITS 32]
start_asm:
	cld              ; required by GCC
	cli
	lgdt   [gdt_80]  ; load GDT

	; Set segment registers
	mov    eax, 3 * 0x8
	mov    ds, ax
	mov    es, ax
	mov    fs, ax
	mov    gs, ax
	mov    ss, ax

	; Init stack
	mov    eax, STACK_SIZE_ONE
	lock xadd [stack_mem_ptr], eax
	; Stack grows downwards, thus we need to add STACK_SIZE again
	add eax, STACK_SIZE_ONE
	mov    esp, eax

	; Init cpuid: atomically fetch-and-increment the counter
init_cpuid:
	mov    eax, 1
	lock xadd [cpu_id_counter], eax
	push dword 0
	push eax

	jmp    init_longmode

;
;  Switch to long mode
;
init_longmode:
	; Activate page address extensions
	mov    eax, cr4
	or     eax, 1 << 5
	mov    cr4, eax

set_cr3:
	; Setup paging tables (required for long mode)
    mov eax, [___KERNEL_CR3__]
    mov    cr3, eax

	; Switch to long mode (for the time being in compatibility mode)
	mov    ecx, 0x0C0000080 ; Select Extended Feature Enable Register
	rdmsr
	or     eax, 1 << 8 		; LME (Long Mode Enable)
	wrmsr

	; Activate paging
	mov    eax, cr0
	or     eax, 1 << 31
	mov    cr0, eax

	; Jump to 64 bit code segment (activates full long mode)
	jmp    2 * 0x8 : longmode_start

;
; Long mode entry function
;
; The bootstrap processor will setup the IDT and reprogram PICs
; The application processors simply only load the same IDT
longmode_start:
[BITS 64]
    ; Sets the idt via setup_ap_idt in interrupt_dispatcher.rs
	call setup_ap_idt

    ; pop cpu_id back up to pass it to startup_ap
	pop rdi

    ; Call Rust entry function in 'boot.rs' for application proc.
    call startup_ap

end:
	; We should never end up here
    cli
    mov dword [0xb8000], 0x2f2a2f2a
	hlt


[SECTION .data]

___KERNEL_CR3__:
	dq 0

;
; GDT
;
gdt:
	dw  0,0,0,0  ; NULL-Deskriptor

	; 32-Bit-Codesegment-Deskriptor
	dw  0xFFFF   ; 4Gb - (0x100000*0x1000 = 4Gb)
	dw  0x0000   ; base address=0
	dw  0x9A00   ; code read/exec
	dw  0x00CF   ; granularity=4096, 386 (+5th nibble of limit)

	; 64-Bit-Codesegment-Deskriptor
	dw  0xFFFF   ; 4Gb - (0x100000*0x1000 = 4Gb)
	dw  0x0000   ; base address=0
	dw  0x9A00   ; code read/exec
	dw  0x00AF   ; granularity=4096, 386 (+5th nibble of limit),long mode

	; Datensegment-Deskriptor
	dw  0xFFFF   ; 4Gb - (0x100000*0x1000 = 4Gb)
	dw  0x0000   ; base address=0
	dw  0x9200   ; data read/write
	dw  0x00CF   ; granularity=4096, 386 (+5th nibble of limit)

gdt_80:
	dw  4*8 - 1  ; GDT Limit=24, 4 GDT Eintraege - 1
	dq  gdt      ; Adresse der GDT

;
;  Memory for stacks
;
stack_mem_ptr:
	dd  reserve_stack_mem
cpu_id_counter:
	dd  1

[SECTION .bss]
reserve_stack_mem:
	resb STACK_MEM_SIZE
.end: