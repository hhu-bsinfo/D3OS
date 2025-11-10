#![no_std]

use terminal::{print, println};
use rdma_core::devices;
use runtime::*;

pub fn invoke() {
    let devices = devices().expect("failed to get device list");

    for dev in devices.iter() {
        let dev_name = dev.name().expect("failed to get device name");
        //let dev_guid = dev.guid().expect("failed to get device guid"); not yet impl.

        println!("Found {:?} !", dev_name); //, dev_guid);
        
        let ctx = dev.open().expect("failed to open device context");
        
        let device_stats = ctx.query_device()
            .expect("failed to query device");

        println!("    Number of ports: {}", device_stats.phys_port_cnt);
        println!("    Firmware version: {}", device_stats.fw_ver);

        // assuming each hca to just have 1 port, which is the default for most
        let port_stats = ctx.query_port();

        println!("        State: {:?}", port_stats.state);
        println!("        Physical state: {:?}", port_stats.phys_state);
        println!("        Base lid: {}", port_stats.lid);
        println!("        LMC: {}", port_stats.lmc);
        println!("        SM lid: {}", port_stats.sm_lid);
        println!("        Capability mask: 0x{:x}", port_stats.port_cap_flags);
        println!("        Link layer: {}", port_stats.link_layer)
    }
}

#[unsafe(no_mangle)]
pub fn main() {
    invoke();
}