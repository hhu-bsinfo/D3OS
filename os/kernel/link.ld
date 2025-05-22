ENTRY(start)

SECTIONS {
    . = 100M;   /* load kernel at 100 MiB */

    ___KERNEL_DATA_START__ = .;

    .boot :
    {
        *(.multiboot2_header)
    }

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
    ___KERNEL_DATA_END__ = .;
}
