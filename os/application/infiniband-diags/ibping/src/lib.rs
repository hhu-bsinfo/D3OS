#![no_std]

use terminal::println;
use ibverbs::devices;
use ibverbs::ibv_qp_type::Type;
use ibverbs::ffi::ibv_qp_cap;

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
        let _qp = pd.create_qp(&cq, &cq, Type::IBV_QPT_RC, ibv_qp_cap {
            max_send_wr: 10,
            max_recv_wr: 10,
            max_send_sge: 3,
            max_recv_sge: 3,
            max_inline_data: 3,
        });
    }
    return
}
