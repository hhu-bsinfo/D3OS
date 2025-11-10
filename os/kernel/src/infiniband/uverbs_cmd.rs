use alloc::vec::Vec;
use rdma::{ibv_access_flags, ibv_device_attr, ibv_port_attr, ibv_wc};
use rdma::uverbs_uapi::{ibv_cq_container, ibv_mr_res, ibv_qp_container, ibv_qp_modify_container, ibv_qp_post_recv_container, ibv_qp_post_send_container};
use spin::{Mutex, MutexGuard};

use crate::device::mlx4::{
    ConnectX3Nic, devices_supported, get_dev_list, minor_to_idx
};

pub fn uverbs_query_devices(dev_store: &mut [usize]) -> usize {
    let len = dev_store.len().min(devices_supported());
    let mut query_hit = 0;
    for dev in get_dev_list().lock().iter().take(len) {
        dev_store[query_hit] = dev.minor;
        query_hit += 1;
    }

    query_hit
}

fn uverbs_lock_dev() -> MutexGuard<'static, Vec<ConnectX3Nic>> {
    get_dev_list().lock()
}

pub fn uverbs_query_device(minor: usize) -> Result<ibv_device_attr, &'static str> {
    let mut x = uverbs_lock_dev();

    x.get_mut(minor_to_idx(minor)).unwrap().query_device()
}

pub fn uverbs_query_port(
    minor: usize, 
    port_num: u8) -> Result<ibv_port_attr, &'static str> {
    let mut x = uverbs_lock_dev();

    x.get_mut(minor_to_idx(minor)).unwrap().query_port(port_num)
}

pub fn uverbs_register_mem_region(
    minor: usize, 
    access_flags: ibv_access_flags, 
    user_data_ref: &mut [u8]) -> Result<ibv_mr_res, &'static str> {
    let mut x = uverbs_lock_dev();

    let mr_res = x.get_mut(minor_to_idx(minor)).unwrap().create_mr(user_data_ref, access_flags)?;

    Ok(mr_res.into())
}

pub fn uverbs_create_cq<'cq>(minor: usize, cq_container: &'cq mut ibv_cq_container) -> Result<&'cq u32, &'static str> {
    let mut x = uverbs_lock_dev();

    let number = x.get_mut(minor_to_idx(minor)).unwrap().create_cq(cq_container.cq_entries)?;
    cq_container.cq_num = number;

    Ok(&cq_container.cq_num)
}

pub fn uverbs_create_qp<'qp>(minor: usize, qp_container: &'qp mut ibv_qp_container) -> Result<&'qp u32, &'static str> {
    let mut x = uverbs_lock_dev();

    let number = x.get_mut(minor_to_idx(minor)).unwrap().create_qp(
        qp_container.qp_type,
        qp_container.send_cq_num, 
    qp_container.recv_cq_num, 
        unsafe { qp_container.ib_caps.as_mut().unwrap() })?;

    qp_container.qp_num = number;
    Ok(&qp_container.qp_num)
}

pub fn uverbs_modify_qp(minor: usize, qp_modify_container: ibv_qp_modify_container) -> Result<(), &'static str> {
    let mut x = uverbs_lock_dev();

    x.get_mut(minor_to_idx(minor)).unwrap().modify_qp(
        qp_modify_container.qp_num, 
        unsafe { qp_modify_container.attr.as_ref().unwrap() },
        qp_modify_container.attr_mask
    )
}

pub fn uverbs_poll_cq(minor: usize, cq_num: u32, wc: &mut [ibv_wc]) -> Result<usize, &'static str> {
    let mut x = uverbs_lock_dev();

    x.get_mut(minor_to_idx(minor)).unwrap().poll_cq(cq_num, wc)
}

pub fn uverbs_post_send(minor: usize, send_container_wr: &ibv_qp_post_send_container) -> Result<(), &'static str> {
    let mut x = uverbs_lock_dev();

    x.get_mut(minor_to_idx(minor)).unwrap().post_send(send_container_wr.qp_num, unsafe { send_container_wr.ibv_send_wr.as_mut().unwrap() })
}

pub fn uverbs_post_recv(minor: usize, recv_container_wr: &ibv_qp_post_recv_container) -> Result<(), &'static str> {
    let mut x = uverbs_lock_dev();

    x.get_mut(minor_to_idx(minor)).unwrap().post_receive(recv_container_wr.qp_num, unsafe { recv_container_wr.ibv_recv_wr.as_mut().unwrap() })
}

pub fn uverbs_destroy(
    minor: usize, 
    destroy_spec_fn: fn(&mut ConnectX3Nic, u32) -> Result<(), &'static str>,
    x_num: u32)
    -> Result<(), &'static str>{
    let mut x = uverbs_lock_dev();

    destroy_spec_fn(x.get_mut(minor_to_idx(minor)).unwrap(), x_num)
}

// todo; map user address region into user space, let user ring doorbell
pub fn uverbs_mmap_uar() {

}