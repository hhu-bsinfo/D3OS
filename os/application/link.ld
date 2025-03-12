ENTRY(entry)

SECTIONS {
    . = 0x10000000000;   /* load at address 1 TB */

    ___APP_DATA_START__ = .;

    .text ALIGN (4K) :
    {
        *(.text*)
    }

   .bss ALIGN (4K) :
    {
      ___BSS_START__ = .;
      *(".bss*")
      ___BSS_END__ = .;
    }

    .data ALIGN (4K) :
    {
        *(.data*)
    }
    ___APP_DATA_END__ = .;
}