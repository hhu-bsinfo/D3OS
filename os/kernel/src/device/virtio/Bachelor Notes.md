# Bachelor Notes: Integration of Virtio Device in QEMU

This document describes the integration of a Virtio device in QEMU using a `Makefile.toml` configuration file. The setup uses the following identifiers:

- **Vendor ID**: `1AF4` (Virtio)
- **Device ID**: `1050` (GPU)

## Log Output

The following log output confirms the detection and configuration of the Virtio GPU device:
- **Device Detection:**  
  The log confirms that the Virtio GPU device (Vendor ID: `1AF4`, Device ID: `1050`) has been detected by the system.

- **Base Address Registers (BARs):**  
  - **BAR0:** Not available  
  - **BAR1 (Memory32):**  
    - **Base Address:** `0x81041000`  
    - **Size:** `0x1000`  
  - **BAR2:** Not available  
  - **BAR3:** Not available  
  - **BAR4 (Memory64):**  
    - **Base Address:** `0xc000000000`  
    - **Size:** `0x4000`  
  - **BAR5 (Memory32):**  
    - **Base Address:** `0xc0`  
    - **Size:** `0x10`

The provided log output verifies that the Virtio GPU device is properly integrated into QEMU through the specified configuration. The details regarding the available and unavailable BARs are clearly outlined, ensuring that the system correctly identifies the device capabilities.

## Struggles with reading pci 

Ich habe komplett flasch gearbeitet und versucht die PCi Capabilities in den Bars zu finden. Aber die sind nicht in den Bars. Die PCI Capabilities sind in der PCI Config Space.

Ich habe außerdem diese Hilfsfunktionen in die ConfigurationSpace Struct in der "pci.rs" Datei hinzugefügt:
```rust
pub fn read_u16(&self, address: PciAddress, offset: u16) -> u16 {
        // Align to a 32-bit boundary.
        let aligned_offset = offset & !0x3;
        // Read the 32-bit word.
        let word = unsafe { self.read(address, aligned_offset) };
        // Shift/mask to get the 16-bit value.
        let shift = (offset & 0x3) * 8;
        ((word >> shift) & 0xffff) as u16
    }

    pub fn read_u8(&self, address: PciAddress, offset: u16) -> u8 {
        let aligned_offset = offset & !0x3;
        let word = unsafe { self.read(address, aligned_offset) };
        let shift = (offset & 0x3) * 8;
        ((word >> shift) & 0xff) as u8
    }

    pub fn read_u32(&self, address: PciAddress, offset: u16) -> u32 {
        // Align offset to a 32-bit boundary.
        let aligned_offset = offset & !0x3;
        unsafe { self.read(address, aligned_offset) }
    }
```

