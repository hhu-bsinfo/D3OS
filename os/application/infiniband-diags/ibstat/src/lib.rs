#![no_std]

use terminal::{print, println};
use ibverbs::devices;
use runtime::*;

pub fn invoke() {
    let devices = devices().expect("failed to get device list");
    println!("Found {} devices", devices.len());

    for dev in devices.iter() {
        let dev_name = dev.name().expect("failed to get device name");
        //let dev_guid = dev.guid().expect("failed to get device guid"); not yet impl.

        println!("Found {:?} !", dev_name); //, dev_guid);

        // The port comes first and goes through `Device::port_attr`, which does not require the
        // port to be usable — reporting a port that has gone down is the whole point of this
        // tool, and `Device::open` refuses exactly that case.
        let port_stats = dev.port_attr().expect("failed to query port");

        match dev.open() {
            Ok(ctx) => {
                let device_stats = ctx.query_device().expect("failed to query device");
                println!("    Number of ports: {}", device_stats.phys_port_cnt);
                println!(
                    "    Firmware version: {}.{}.{}",
                    device_stats.fw_ver_major, device_stats.fw_ver_minor, device_stats.fw_ver_subminor
                );
            }
            Err(e) => println!("    (cannot open a context: {:?})", e),
        }

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
