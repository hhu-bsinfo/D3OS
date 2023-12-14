;******************************************************************************
;*                        B O O T . A S M                                     *
;*----------------------------------------------------------------------------*
;* Die Funktion 'boot_start' ist der Eintrittspunkt des eigentlichen Systems. *
;* Die Umschaltung in den 32-bit 'Protected Mode' ist bereits durch grub      *
;* erfolgt. Es wird alles vorbereitet, damit so schnell wie möglich mit der   * 
;* Ausführung von C++-Code im 64-bit 'Long Mode' begonnen werden kann.        *
;* boot.bin wird an 1 MB geladen und konsumiert mit PageTables 1 MB, sodass   *
;* der C Code oberhalb von 2 MB liegt.                                        *
;*                                                                            *
;* Autor: Michael Schoettner, Uni Duesseldorf, 26.2.2023                      *
;******************************************************************************

%include "src/constants.asm"

; Speicherplatz fuer die Seitentabelle
[GLOBAL pagetable_start]
pagetable_start:  equ 0x103000    ; 1 MB + 12 KB

[GLOBAL pagetable_end]
pagetable_end:  equ 0x200000      ;  = 2 MB

;
;   System
;

; Von uns bereitgestellte Funktionen
[GLOBAL start]
[GLOBAL idt]
[GLOBAL tss]
[GLOBAL get_tss_address]
[GLOBAL tss_set_rsp0]

; C-Funktion die am Ende des Assembler-Codes aufgerufen werden
[EXTERN startup]
[EXTERN setup_idt]


; Vom Compiler bereitgestellte Adressen
[EXTERN ___BSS_START__]
[EXTERN ___BSS_END__]

; In 'sections' definiert
[EXTERN ___KERNEL_DATA_START__]
[EXTERN ___KERNEL_DATA_END__]


; Multiboot constants
MULTIBOOT_HEADER_MAGIC equ 0xe85250d6
MULTIBOOT_HEADER_ARCHITECTURE equ 0
MULTIBOOT_HEADER_LENGTH equ (start - multiboot_header)
MULTIBOOT_HEADER_CHECKSUM equ -(MULTIBOOT_HEADER_MAGIC + MULTIBOOT_HEADER_ARCHITECTURE + MULTIBOOT_HEADER_LENGTH)

[SECTION .text]

;
;   System-Start, Teil 1 (im 32-bit Protected Mode)
;
;   Initialisierung von GDT und Seitentabelle und Wechsel in den 64-bit
;   Long Mode.
;

[BITS 32]

multiboot_header:
; Header
    align 8
    dd MULTIBOOT_HEADER_MAGIC
    dd MULTIBOOT_HEADER_ARCHITECTURE
    dd MULTIBOOT_HEADER_LENGTH
    dd MULTIBOOT_HEADER_CHECKSUM

    ; Address tag
    align 8
    dw MULTIBOOT_TAG_ADDRESS
    dw MULTIBOOT_TAG_FLAG_OPTIONAL
    dd 24
    dd (multiboot_header)
    dd (___KERNEL_DATA_START__)
    dd (___KERNEL_DATA_END__)
    dd (___BSS_END__)

    ; Entry address tag
    align 8
    dw MULTIBOOT_TAG_ENTRY_ADDRESS
    dw MULTIBOOT_TAG_FLAG_OPTIONAL
    dd 12
    dd (start)

    ; Information request tag (required)
    align 8
    dw MULTIBOOT_TAG_INFORMATION_REQUEST
    dw 0
    dd 32
    dd MULTIBOOT_REQUEST_BOOT_COMMAND_LINE
    dd MULTIBOOT_REQUEST_MODULE
    dd MULTIBOOT_REQUEST_MEMORY_MAP
    dd MULTIBOOT_REQUEST_FRAMEBUFFER_INFO
    dd MULTIBOOT_REQUEST_ACPI_OLD_RSDP
    dd MULTIBOOT_REQUEST_ACPI_NEW_RSDP

    ; Information request tag (optional)
    align 8
    dw MULTIBOOT_TAG_INFORMATION_REQUEST
    dw MULTIBOOT_TAG_FLAG_OPTIONAL
    dd 12
    dd MULTIBOOT_REQUEST_BOOT_LOADER_NAME

    ; Framebuffer tag
    align 8
    dw MULTIBOOT_TAG_FRAMEBUFFER
    dw 0
    dd 20
    dd MULTIBOOT_GRAPHICS_WIDTH
    dd MULTIBOOT_GRAPHICS_HEIGHT
    dd MULTIBOOT_GRAPHICS_BPP

    ; Module alignment tag
    align 8
    dw MULTIBOOT_TAG_MODULE_ALIGNMENT
    dw MULTIBOOT_TAG_FLAG_OPTIONAL
    dd 8

    ; Termination tag
    align 8
    dw MULTIBOOT_TAG_TERMINATE
    dw 0
    dd 8

;  GRUB Einsprungspunkt
start:
	cld              ; GCC-kompilierter Code erwartet das so
	cli              ; Interrupts ausschalten
	lgdt   [gdt_80]  ; Neue Segmentdeskriptoren setzen

	; Stack festlegen
	mov    ax, 3 * 0x8
	mov    ss, ax
	mov    esp, init_stack+STACK_SIZE
   
	; Sichere Adresse der Multiboot-Struktur (ist in EBX)
	; da wird den Inhalt erst im 64 Bit Mode wieder herunterholen
	; muessen wir 8 Bytes 'pushen'
    push   0
    push   ebx

	jmp    init_longmode


;
;  Umschalten in den 64 Bit Long-Mode
;
init_longmode:
	; Adresserweiterung (PAE) aktivieren
	mov    eax, cr4
	or     eax, 1 << 5
	mov    cr4, eax

	; Seitentabelle anlegen (Ohne geht es nicht)
	call   setup_paging

	; Long-Mode (fürs erste noch im Compatibility-Mode) aktivieren
	mov    ecx, 0x0C0000080 ; EFER (Extended Feature Enable Register) auswaehlen
	rdmsr
	or     eax, 1 << 8 ; LME (Long Mode Enable)
	wrmsr

	; Paging aktivieren
	mov    eax, cr0
	or     eax, 1 << 31
	mov    cr0, eax

	; Sprung ins 64 Bit-Codesegment -> Long-Mode wird vollständig aktiviert
	jmp    2 * 0x8 : longmode_start


;
;   Anlegen einer (provisorischen) Seitentabelle mit 2 MB Seitengröße, die die
;   ersten MAX_MEM GB direkt auf den physikalischen Speicher abbildet.
;   Dies ist notwendig, da eine funktionierende Seitentabelle für den Long-Mode
;   vorausgesetzt wird. Mehr Speicher darf das System im Moment nicht haben.
;
setup_paging:
	; PML4 (Page Map Level 4 / 1. Stufe)
	mov    eax, pdp
	or     eax, 0xf
	mov    dword [pml4+0], eax
	mov    dword [pml4+4], 0

	; PDPE (Page-Directory-Pointer Entry / 2. Stufe) für aktuell 16GB
	mov    eax, pd
	or     eax, 0x7           ; Adresse der ersten Tabelle (3. Stufe) mit Flags.
	mov    ecx, 0
fill_tables2:
	cmp    ecx, MAX_MEM       ; MAX_MEM Tabellen referenzieren
	je     fill_tables2_done
	mov    dword [pdp + 8*ecx + 0], eax
	mov    dword [pdp + 8*ecx + 4], 0
	add    eax, 0x1000        ; Die Tabellen sind je 4kB groß
	inc    ecx
	ja     fill_tables2
fill_tables2_done:

	; PDE (Page Directory Entry / 3. Stufe)
	mov    eax, 0x0 | 0x87    ; Startadressenbyte 0..3 (=0) + Flags
	mov    ebx, 0             ; Startadressenbyte 4..7 (=0)
	mov    ecx, 0
fill_tables3:
	cmp    ecx, 512*MAX_MEM   ; MAX_MEM Tabellen mit je 512 Einträgen füllen
	je     fill_tables3_done
	mov    dword [pd + 8*ecx + 0], eax ; low bytes
	mov    dword [pd + 8*ecx + 4], ebx ; high bytes
	add    eax, 0x200000      ; 2 MB je Seite
	adc    ebx, 0             ; Overflow? -> Hohen Adressteil inkrementieren
	inc    ecx
	ja     fill_tables3
fill_tables3_done:

	; Basiszeiger auf PML4 setzen
	mov    eax, pml4
	mov    cr3, eax
	ret

;
;   System-Start, Teil 2 (im 64-bit Long-Mode)
;
;   Das BSS-Segment wird gelöscht und die IDT die PICs initialisiert.
;   Anschließend werden die Konstruktoren der globalen C++-Objekte und
;   schließlich main() ausgeführt.
;
longmode_start:
[BITS 64]
    ; zuvor gesicherter Zeiger auf multiboot infos vom Stack holen und
    ; in 'multiboot_info_address' sichern. Durch die Konstruktoren wird 
    ; der Stack manipuliert, daher muessen wir das gleich hier machen
    pop    rax  
    mov    [multiboot_info_address], rax
    
	; BSS löschen
	mov    rdi, ___BSS_START__
clear_bss:
	mov    byte [rdi], 0
	inc    rdi
	cmp    rdi, ___BSS_END__
	jne    clear_bss

    ; TSS-Basisadresse im GDT-Eintrag setzen
    call tss_set_base_address

    ; Kernel-Stack im TSS = rsp0 setzen
    mov rdi, init_stack.end
    call tss_set_rsp0

    ; Lade TSS-Register mit dem 5. GDT-Eintrag
    xor rax, rax
    mov rax, 6 * 8
    ltr ax

    call setup_idt
	
    mov    rdi, [multiboot_info_address] ; 1. Parameter wird in rdi uebergeben
	call   startup ; multiboot infos auslesen und 'main' aufrufen
	
	cli            ; Hier sollten wir nicht hinkommen
	hlt

;
; TSS Basisadresse in GDT-Eintrag setzen
;
tss_set_base_address:
    ; TSS Basisadresse in GDT-Eintrag aktualisieren
    mov rbx, tss_entry

    ; Basis-Adresse setzen [00:15]
    mov rax, tss
    mov word [rbx+2], ax

    ; Basis-Adresse setzen [16:23]
    mov rax, tss
    shr rax, 16
    mov byte [rbx+4], al

    ; Basis-Adresse setzen [24:31]
    mov rax, tss
    shr rax, 24
    mov byte [rbx+7], al

    ; Obere 32 Bit der Basis im TSS-Deskriptor schreiben
    mov rax, tss
    shr rax, 32
    mov dword [rbx+8], eax

    ; Letzte beiden Bytes auf 0 setzen
    xor rax, rax
    mov word [rbx+12], ax

    ret

; Kernel-Stack im TSS = rsp0 setzen
; rdi = Zeiger auf Stack (letzter genutzer Eintrag)
tss_set_rsp0:
    mov rax, tss
    mov [rax+4], rdi
    ret

; Adresse des TSS abfragen
get_tss_address:
    mov rax, tss
    ret

;
; Funktionen für den C++ Compiler. Diese Label müssen für den Linker
; definiert sein; da bei OOStuBS keine Freigabe des Speichers erfolgt, können
; die Funktionen aber leer sein.
;
__cxa_pure_virtual: ; "virtual" Methode ohne Implementierung aufgerufen
;_ZdlPv:             ; void operator delete(void*)
;_ZdlPvj:            ; void operator delete(void*, unsigned int) fuer g++ 6.x
;_ZdlPvm:            ; void operator delete(void*, unsigned long) fuer g++ 6.x
	ret


[SECTION .data]

;
; Segment-Deskriptoren
;
gdt:
	dw  0,0,0,0   ; NULL-Deskriptor

    ; Kernel 32-Bit-Codesegment-Deskriptor (nur fuer das Booten benoetigt)
	dw  0xFFFF    ; 4Gb - (0x100000*0x1000 = 4Gb)
	dw  0x0000    ; base address=0
	dw  0x9A00    ; code read/exec
	dw  0x00CF    ; granularity=4096, 386 (+5th nibble of limit)

	; Kernel 64-Bit-Codesegment-Deskriptor
	dw  0xFFFF    ; 4Gb - (0x100000*0x1000 = 4Gb)
	dw  0x0000    ; base address=0
	dw  0x9A00    ; code read/exec
	dw  0x00AF    ; granularity=4096, 386 (+5th nibble of limit), Long-Mode

	; Kernel 64-Bit-Datensegment-Deskriptor
	dw  0xFFFF    ; 4Gb - (0x100000*0x1000 = 4Gb)
	dw  0x0000    ; base address=0
	dw  0x9200    ; data read/write
	dw  0x00CF    ; granularity=4096, 386 (+5th nibble of limit)

    ; User 64-Bit-Datensegment-Deskriptor
    dw  0xFFFF    ; limit [00:15] = 4Gb - (0x100000*0x1000 = 4Gb)
    dw  0x0000    ; base  [00:15] = 0
    dw  0xF200    ; base  [16:23] = 0, data read/write, DPL=3, present
    dw  0x00CF    ; limit [16:19], granularity=4096, 386, base [24:31]

    ; User 64-Bit-Codesegment-Deskriptor
    dw  0xFFFF    ; limit [00:15] = 4Gb - (0x100000*0x1000 = 4Gb)
    dw  0x0000    ; base  [00:15] = 0
    dw  0xFA00    ; base  [16:23] = 0, data code/exec, DPL=3, present
    dw  0x00AF    ; limit [16:19], granularity=4096, 386, base [24:31]

tss_entry:
    ; Task State Segment Deskriptor
    dw  0x0068    ; limit [00:15] = 0x6b = 104 Bytes (no I/O bitmap)
    dw  0x0000    ; base  [00:15] = 0
    dw  0x8900    ; base  [16:23, ], tss, DPL=0, present
    dw  0x0000    ; limit [16:19] = 0, granularity=0, 386, Long-Mode, base [24:31]

    dw  0x0000    ; base [47:32]
    dw  0x0000    ; base [63:32]
    dw  0x0000    ; 000 + reserved
    dw  0x0000    ; reserved

gdt_80:
    ; 7 Eintraege in der GDT, aber der TSS-Eintrag hat 16 Byte und zaehlt daher doppelt!
	dw  8*8 - 1   ; GDT Limit = 64, 7 GDT Eintraege - 1
	dq  gdt       ; Adresse der GDT

multiboot_info_address:
	dq  0

;
; Speicher (104 Bytes) fuer ein Task State Segment (TSS) ohne IO-Bitmap
; siehe auch: https://stackoverflow.com/questions/54876039/creating-a-proper-task-state-segment-tss-structure-with-and-without-an-io-bitm
;
tss:
    times 100 db 0
    dw 0
    dw 0x68

[SECTION .bss]

global init_stack:data (init_stack.end - init_stack)
init_stack:
	resb STACK_SIZE
.end:


;
; Speicher fuer Page-Tables
;
[SECTION .global_pagetable]

[GLOBAL pml4]
[GLOBAL pdp]
[GLOBAL pd]

pml4:
    times 4096 db 0
	alignb 4096

pd:
    times MAX_MEM*4096 db 0
	alignb 4096

pdp:
    times MAX_MEM*8 db 0    ; 254*8 = 2032

