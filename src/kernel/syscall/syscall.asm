;******************************************************************************
;*                                                                            *
;*                  s y s c a l l s . a s m                                   *
;*                                                                            *
;*----------------------------------------------------------------------------*
;* Beschreibung:    Hier befindet sich alles rund um die low-level Behandlung *
;*                  von Systemaufrufen sowie die Weiterleitung an Rust.       *
;*                                                                            *
;*                  Achtung: '_init_syscalls' muss nach der Initialisieriung  *
;*                  der IDT aufgerufen werden!                                *
;*                                                                            *
;* Autor:           Michael Schoettner, 23.8.2023                             *
;******************************************************************************

[GLOBAL init_syscalls]       ; Funktion exportieren
[GLOBAL NO_SYSCALLS]

[EXTERN idt]                 ; IDT in 'interrupts.asm'
[EXTERN syscall_disp]         ; Funktion in Rust, die Syscalls behandelt
[EXTERN syscall_abort]        ; Funktion in Rust, die abbricht, 
                              ; falls der Systemaufruf nicht existiert

[SECTION .text]
[BITS 64]

; Hoechste Funktionsnummer für den System-Aufruf-Dispatcher
; Muss mit NO_SYSCALLS in 'kernel/syscall/mod.rs' konsistent sein!
NO_SYSCALLS equ 3

; Vektor fuer Systemaufrufe
SYSCALL_TRAPGATE equ 0x86

;
; Trap-Gate fuer Systemaufrufe einrichten
;
init_syscalls:
    mov    rax, syscall_handler

    ; Adresse von _syscall_handler wird aufgeteilt in der IDT gespeichert
    ; Bits 0..15 -> ax, 16..31 -> bx, 32..64 -> edx
    mov    rbx, rax
    mov    rdx, rax
    shr    rdx, 32
    shr    rbx, 16

    mov    r10, idt      ; Zeiger auf das este Interrupt-Gate
    add    r10, 16 * SYSCALL_TRAPGATE ; Adresse des Trap-Gates 0x86 in der IDT

    ; Adresse von syscall_handler eintragen
    mov    [r10+0], ax    ; Bits 0..15
    mov    [r10+6], bx    ; 16..31
    mov    [r10+8], edx   ; 32..64

    ; DPL = 3, Typ = Trap Gate (bisher Interrupt Gate)
    xor    rax, rax
    mov    rax, 0xef;
    mov    [r10+5], al    ; Schreibe Bits 38..47 im IDT Eintrag
    ret

;
; Handler fuer Systemaufrufe 
;
syscall_handler:
    ; Register sichern, nicht eax = Funktionsnummer + Ergebnis
    push   rbx
    push   rcx
    push   rdx
    push   rdi
    push   rsi
    push   r8
    push   r9
    push   r10
    push   r11
    push   r12
    push   r13
    push   r14
    push   r15

    ; DS und ES sichern und auf Kernel-Data Segment setzen
    mov rcx, ds
    push rcx
    mov rcx, es
    push rcx
    mov rcx, 0x8 * 3 ; Selector zeigt auf den 64-Bit-Datensegment-Deskriptor der GDT (4. Eintrag)
    mov ds, rcx
    mov es, rcx

    ; Pruefen, ob die Funktionsnummer nicht zu gross ist
    cmp rax, NO_SYSCALLS
    jge syscall_abort   ; wirft eine Panic, kehrt nicht zurueck

    ; Funktionsnummer ist OK -> Rust aufrufen
    call syscall_disp

    ; DS und ES wiederherstellen -> User-Data Segment setzen
    pop rcx
    mov es, rcx
    pop rcx
    mov ds, rcx

    ; Register wiederherstellen
    pop    r15
    pop    r14
    pop    r13
    pop    r12
    pop    r11
    pop    r10
    pop    r9
    pop    r8
    pop    rsi
    pop    rdi
    pop    rdx
    pop    rcx
    pop    rbx

    iretq