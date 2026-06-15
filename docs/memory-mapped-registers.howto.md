## Structured access to memory-mapped registers

### Crate
Crate: `tock-registers`

### Example: Memory-Mapped Register Block
Let’s say your device has the following layout (all registers 32-bit):

| Offset | Register | Access |
|--------|----------|--------|
| 0x00   | CONTROL  | RW     |
| 0x04   | STATUS   | RO     |
| 0x08   | DATA     | RW     |


### Define the register layout

```
use tock_registers::registers::{ReadOnly, ReadWrite};
use tock_registers::register_bitfields;
use core::ptr::NonNull;

register_bitfields![u32,
    CONTROL [
        ENABLE  OFFSET(0)  NUMBITS(1) [],
        MODE    OFFSET(1)  NUMBITS(2) [],
        RESET   OFFSET(3)  NUMBITS(1) []
    ],
    STATUS [
        READY   OFFSET(0)  NUMBITS(1) [],
        ERROR   OFFSET(1)  NUMBITS(1) []
    ]
];
```

### Define the MMIO struct

```
pub struct Device {
    regs: NonNull<DeviceRegisters>,
}

impl Device {
    /// # Safety
    /// `base_addr` must point to a valid memory-mapped device register block.
    pub unsafe fn new(base_addr: *mut DeviceRegisters) -> Self {
        Self {
            regs: NonNull::new(base_addr).expect("null pointer to device registers"),
        }
    }

    #[inline]
    fn regs(&self) -> &DeviceRegisters {
        unsafe { self.regs.as_ref() }
    }

    pub fn enable(&self) {
        self.regs().control.modify(CONTROL::ENABLE::SET);
    }

    pub fn set_mode(&self, mode: u32) {
        self.regs().control.modify(CONTROL::MODE.val(mode));
    }

    pub fn reset(&self) {
        self.regs().control.modify(CONTROL::RESET::SET);
    }

    pub fn is_ready(&self) -> bool {
        self.regs().status.is_set(STATUS::READY)
    }

    pub fn write_data(&self, value: u32) {
        self.regs().data.set(value);
    }

    pub fn read_data(&self) -> u32 {
        self.regs().data.get()
    }
}
```

#### Usage example
```
fn main() {
    // Pretend we got this from mmap or kernel
    let mmio_addr: *mut DeviceRegisters = 0x1000_0000 as *mut _;

    let device = unsafe { Device::new(mmio_addr) };

    device.enable();
    device.set_mode(0b10);
    device.write_data(42);

    if device.is_ready() {
        let val = device.read_data();
        println!("Device returned: {}", val);
    }

    device.reset();
}
```