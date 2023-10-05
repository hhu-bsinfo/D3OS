ENTRY(start)

SECTIONS {
    . = 1M;   /* load at address 1MB */

    ___KERNEL_DATA_START__ = .;

    .boot :
    {
        /* ensure that the multiboot header is at the beginning */
        *(.multiboot_header)
    }

    /* Startcode fuer die APs, wird von System::init() reloziert */
	.boot_seg_ap ALIGN(0x10) :
	{
		*(".boot_seg_ap")
		*(".boot_seg_ap$")
	}


    .text :
    {
        *(.text)
    }

   .bss : 
    {
      ___BSS_START__ = .;
      *(".bss")
      *(".bss.*")
      ___BSS_END__ = .;
    }

    ___KERNEL_DATA_END__ = .;
}
