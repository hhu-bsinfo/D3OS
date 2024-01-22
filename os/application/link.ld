ENTRY(main)

SECTIONS {
    . = 0x10000000000;   /* load at address 1 TB */

    ___APP_DATA_START__ = .;

    .text :
    {
        *(.text*)
    }

   .bss :
    {
      ___BSS_START__ = .;
      *(".bss")
      *(".bss.*")
      ___BSS_END__ = .;
    }

    ___APP_DATA_END__ = .;
}