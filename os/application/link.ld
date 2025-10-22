ENTRY(entry)

PHDRS {
    text PT_LOAD;
    data PT_LOAD;
    bss PT_LOAD;
}

SECTIONS {
    . = 0x10000000000;   /* load at address 1 TB */

    ___APP_DATA_START__ = .;

    .text ALIGN (4K) :
    {
        *(.text*)
    } :text

   .bss ALIGN (4K) :
    {
      ___BSS_START__ = .;
      *(".bss*")
      ___BSS_END__ = .;
    } :bss

    .data ALIGN (4K) :
    {
        *(.data*)
    } :data
    ___APP_DATA_END__ = .;
}