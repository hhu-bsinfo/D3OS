use acpi::AcpiTables;
use spin::Mutex;
use crate::device::lfb_terminal::LFBTerminal;
use crate::device::pit::Pit;
use crate::device::ps2::PS2;
use crate::device::speaker::Speaker;
use crate::device::terminal::Terminal;
use crate::kernel::Service;
use crate::kernel::service::memory_service::AcpiHandler;

pub struct DeviceService {
    pit: Pit,
    speaker: Mutex<Speaker>,
    ps2: PS2,
    terminal: Mutex<LFBTerminal>,
    acpi_tables: Option<AcpiTables<AcpiHandler>>
}

impl Service for DeviceService {}

impl DeviceService {
    pub const fn new() -> Self {
        Self { pit: Pit::new(), speaker: Mutex::new(Speaker::new()), ps2: PS2::new(), terminal: Mutex::new(LFBTerminal::empty()), acpi_tables: None }
    }

    pub fn init_timer(&mut self) {
        self.pit.set_int_rate(1);
    }

    pub fn init_keyboard(&mut self) {
        self.ps2.init_controller().unwrap_or_else(|err| panic!("Failed to initialize PS2 controller (Error: {:?})", err));
        self.ps2.init_keyboard().unwrap_or_else(|err| panic!("Failed to initialize PS2 keyboard (Error: {:?})", err));
    }

    pub fn init_terminal(&mut self, buffer: *mut u8, pitch: u32, width: u32, height: u32, bpp: u8) {
        self.terminal = Mutex::new(LFBTerminal::new(buffer, pitch, width, height, bpp, true));
    }

    pub fn init_acpi_tables(&mut self, rsdp_addr: usize) {
        let handler = AcpiHandler::default();

        unsafe {
            let tables = AcpiTables::from_rsdp(handler, rsdp_addr);
            match tables {
                Ok(tables) => {
                    self.acpi_tables = Some(tables);
                }
                Err(_) => {
                    panic!("Failed to parse ACPI tables");
                }
            }
        }
    }

    pub fn get_timer(&mut self) -> &mut Pit {
        return &mut self.pit;
    }

    pub fn get_speaker(&self) -> &Mutex<Speaker> {
        return &self.speaker;
    }

    pub fn get_ps2(&mut self) -> &mut PS2 {
        return &mut self.ps2;
    }

    pub fn get_terminal(&mut self) -> &Mutex<dyn Terminal> {
        return &mut self.terminal;
    }

    pub fn get_acpi_tables(&mut self) -> &mut AcpiTables<AcpiHandler> {
        return self.acpi_tables.as_mut().expect("ACPI: Accessing tables before initialization!");
    }
}