ENTRY(start)

SECTIONS {
   /*
    * a QEMU VM with OVMF roughly has the following memory layout:
    *   0KB to 640KB: free
    * 640KB to 1MB:   devices
    *   1MB to 8MB:   free
    *   8MB to 9MB:   ACPI
    *   9MB to 24MB:  Boot Services
    *
    * on real hardware, this may look different, e.g. like this:
    *   0KB to 32KB:  Boot Services
    *  32KB to 232KB: free
    * 352KB to 356KB: reserved
    * 376KB to 636KB: Boot Services
    * 636KB to 640KB: reserved
    * 640KB to 1MB:   devices
    *   1MB to 2.5GB: free
    * 2.5GB to 2.6GB: Boot Services
    * 2.6GB to 3GB:   free
    *        > 3GB:   ACPI, Boot Services, Runtime Services
    */

    /*
     * start code for the APs
     *
     * This needs to be below 1MB, because they start in Real Mode.
     * It also needs to be page-aligned. And it can't start at 0.
     */
    . = 32K;
	.boot_seg_ap :
	{
		___BOOT_AP_START__ = .;
		*(".boot_seg_ap")
		*(".boot_seg_ap$")
	    ___BOOT_AP_END__ = .;
	}
	
    /* the rest of the kernel code and data */
    . = 25M;

    ___KERNEL_DATA_START__ = .;

    .boot :
    {
        *(.multiboot2_sec)
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