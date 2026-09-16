#![no_std]

use ibverbs::devices;
use ibverbs::QueuePairType;
use terminal::println;

#[allow(unused_imports)]
use runtime::*;

#[unsafe(no_mangle)]
pub fn main() {
    let devices = devices().expect("failed to get device list");
    println!("Found {:} devices!", devices.len());

    if let Some(dev) = devices.iter().next() {
        let ctx = dev.open().expect("failed to open device");
        let cq = ctx.create_cq(10, 69).unwrap();
        let pd = ctx.alloc_pd().unwrap();
        let _qp = pd.create_qp(&cq, &cq, QueuePairType::RC)
            .set_max_send_wr(10)
            .set_max_recv_wr(10)
            .set_max_send_sge(1)
            .set_max_recv_sge(1)
            .set_max_inline_data(1)
            .build().expect("faild to create qp");
    }
    return
}
