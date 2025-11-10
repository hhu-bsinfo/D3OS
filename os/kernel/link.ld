ENTRY(start)

PHDRS {
    text PT_LOAD;
    data PT_LOAD;
    bss PT_LOAD;
}

SECTIONS {
    /*
     * load the kernel to the following address
     *
     * a QEMU VM with OVMF roughly has the following memory layout:
     *   0KB to 640KB: free
     * 640KB to 1MB:   devices
     *   1MB to 8MB:   free
     *   8MB to 9MB:   ACPI
     *   9MB to 24MB:  Boot Services
     */
    . = 25M;

    ___KERNEL_DATA_START__ = .;

    .boot :
    {
        *(.multiboot2_header)
    } :text

    .text ALIGN (4K) :
    {
        *(.text*)
    } :text

   .bss ALIGN (4K) :
    {
      ___BSS_START__ = .;
      *(.bss*)
      ___BSS_END__ = .;
    } :bss

    .ex_table ALIGN (8) :
    {
        KEEP(*sec.o(.ex_table*))
    }

    .data ALIGN (4K) :
    {
        *(.data*)
    } :data
    ___KERNEL_DATA_END__ = .;
}
