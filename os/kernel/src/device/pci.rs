use alloc::vec::Vec;
use log::{info};
use pci_types::{BaseClass, ConfigRegionAccess, EndpointHeader, HeaderType, PciAddress, PciHeader, PciPciBridgeHeader, SubClass};
use spin::Mutex;
use x86_64::instructions::port::{Port, PortWriteOnly};

const MAX_DEVICES_PER_BUS: u8 = 32;
const MAX_FUNCTIONS_PER_DEVICE: u8 = 8;
const INVALID: u16 = 0xffff;

pub struct PciBus {
    config_space: ConfigurationSpace,
    devices: Vec<EndpointHeader>
}

pub struct ConfigurationSpace {
    ports: Mutex<ConfigurationPorts>
}

struct ConfigurationPorts {
    address_port: PortWriteOnly<u32>,
    data_port: Port<u32>,
}

impl ConfigurationPorts {
    const fn new() -> Self {
        Self { address_port: PortWriteOnly::new(0xcf8), data_port: Port::new(0xcfc) }
    }
}

impl ConfigurationSpace {
    const fn new() -> Self {
        Self { ports: Mutex::new(ConfigurationPorts::new()) }
    }

    unsafe fn prepare_access(ports: &mut ConfigurationPorts, address: PciAddress, offset: u16) {
        let address_raw = 0x80000000u32
            | (address.bus()  as u32) << 16
            | (address.device() as u32) << 11
            | (address.function() as u32) << 8
            | (offset & 0xfc) as u32;

        unsafe { ports.address_port.write(address_raw);  }
    }
}

impl ConfigRegionAccess for ConfigurationSpace {
    fn function_exists(&self, _address: PciAddress) -> bool {
        false
    }

    unsafe fn read(&self, address: PciAddress, offset: u16) -> u32 {
        let mut ports = self.ports.lock();

        unsafe {
            Self::prepare_access(&mut ports, address, offset);
            return ports.data_port.read();
        }
    }

    unsafe fn write(&self, address: PciAddress, offset: u16, value: u32) {
        let mut ports = self.ports.lock();

        unsafe {
            Self::prepare_access(&mut ports, address, offset);
            ports.data_port.write(value);
        }
    }
}

impl PciBus {
    pub fn scan() -> Self {
        let mut pci = Self { config_space: ConfigurationSpace::new(), devices: Vec::new() };

        let root = PciHeader::new(PciAddress::new(0x8000, 0, 0, 0));
        if root.has_multiple_functions(&pci.config_space) {
            info!("Multiple PCI host controllers detected");
            for i in 0..MAX_FUNCTIONS_PER_DEVICE {
                let address = PciAddress::new(0x8000, 0, 0, i);
                let header = PciHeader::new(address);
                if header.id(&pci.config_space).0 == INVALID {
                    break;
                }

                pci.scan_bus(address);
            }
        } else {
            info!("Single PCI host controller detected");
            pci.scan_bus(PciAddress::new(0x8000, 0, 0, 0));
        }


        return pci;
    }

    #[inline(always)]
    pub fn config_space(&self) -> &ConfigurationSpace {
        &self.config_space
    }

    pub fn search_by_ids(&self, vendor_id: u16, device_id: u16) -> Vec<&EndpointHeader> {
        self.devices.iter()
            .filter(|device| device.header().id(self.config_space()) == (vendor_id, device_id))
            .collect()
    }

    pub fn search_by_class(&self, base_class: BaseClass, sub_class: SubClass) -> Vec<&EndpointHeader> {
        self.devices.iter()
            .filter(|device| {
                let info = device.header().revision_and_class(self.config_space());
                info.1 == base_class && info.2 == sub_class
            })
            .collect()
    }

    fn scan_bus(&mut self, address: PciAddress) {
        assert_eq!(address.device(), 0);
        assert_eq!(address.function(), 0);

        for i in 0..MAX_DEVICES_PER_BUS {
            self.check_device(PciAddress::new(address.segment(), address.bus(), i, 0));
        }
    }

    fn check_device(&mut self, address: PciAddress) {
        assert_eq!(address.function(), 0);

        let device = PciHeader::new(address);
        let id = device.id(self.config_space());
        if id.0 == INVALID {
            return;
        }

        self.check_function(address);

        if device.has_multiple_functions(self.config_space()) {
            for i in 1..MAX_FUNCTIONS_PER_DEVICE {
                let address = PciAddress::new(address.segment(), address.bus(), address.device(), i);
                let device = PciHeader::new(address);
                if device.id(self.config_space()).0 == INVALID {
                    break;
                }

                self.check_function(address)
            }
        }
    }

    fn check_function(&mut self, address: PciAddress) {
        let device = PciHeader::new(address);
        let id = device.id(self.config_space());

        if device.header_type(self.config_space()) == HeaderType::PciPciBridge {
            info!("Found PCI-to-PCI bridge on bus [{}]", address.bus());
            let bridge = PciPciBridgeHeader::from_header(device, self.config_space()).unwrap();
            self.scan_bus(PciAddress::new(0x8000, bridge.secondary_bus_number(self.config_space()), 0 , 0));
        } else {
            info!("Found PCI device [0x{:0>4x}:0x{:0>4x}] on bus [{}]", id.0, id.1, address.bus());
            self.devices.push(EndpointHeader::from_header(device, self.config_space()).unwrap());
        }
    }
}