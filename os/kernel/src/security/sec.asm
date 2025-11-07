extern __fixup_copy_from_user 
extern __fixup_copy_to_user

global __load_user_byte
global __store_user_byte
global __ex_entries
global __ex_table

section .text

__load_user_byte:
    mov al, byte [rdi]
    ret

__store_user_byte:
    mov al, sil
__store_user_byte_f:
    mov byte [rdi], al
    ret

section .ex_table
align 8

__ex_table:
    ; Entry for copy_from_user
    dq __load_user_byte
    dq __fixup_copy_from_user

    ; Entry for copy_to_user
    dq __store_user_byte_f
    dq __fixup_copy_to_user

__ex_end:

__ex_entries:
    dq (__ex_end - __ex_table) / 16