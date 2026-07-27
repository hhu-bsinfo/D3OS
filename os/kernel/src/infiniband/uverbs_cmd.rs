use crate::device::mlx4::{get_dev_list, device_handle_to_idx, ConnectX3Nic};
use alloc::vec::Vec;
use rdma::uverbs_uapi::{CreateCqRequest, CreateCqResponse, CreateMrResponse, CreateQpRequest, ModifyQpRequest, PostReceiveRequest, PostSendRequest};
use rdma::{ibv_access_flags, ibv_device, ibv_device_attr, ibv_port_attr, ibv_wc};

pub fn uverbs_query_devices(max_len: usize) -> Vec<ibv_device> {
    get_dev_list().lock().iter()
        .map(|dev| ibv_device { handle: dev.handle } )
        .take(max_len)
        .collect()
}

pub fn uverbs_query_device(device_handle: usize) -> Result<ibv_device_attr, &'static str> {
    get_dev_list().lock()
        .get_mut(device_handle_to_idx(device_handle)).unwrap()
        .query_device()
}

pub fn uverbs_query_port(device_handle: usize, port_num: u8) -> Result<ibv_port_attr, &'static str> {
    get_dev_list().lock()
        .get_mut(device_handle_to_idx(device_handle)).unwrap()
        .query_port(port_num)
}

pub fn uverbs_register_mem_region(device_handle: usize, access_flags: ibv_access_flags, user_data_ref: &mut [u8]) -> Result<CreateMrResponse, &'static str> {
    get_dev_list().lock().get_mut(device_handle_to_idx(device_handle)).unwrap()
        .create_mr(user_data_ref, access_flags)
        .map(CreateMrResponse::from)
        .map_err(|_| "failed to create memory region")
}

pub fn uverbs_create_cq<'cq>(device_handle: usize, cq_container: &'cq CreateCqRequest) -> Result<CreateCqResponse, &'static str> {
    let cq_num = get_dev_list().lock()
        .get_mut(device_handle_to_idx(device_handle)).unwrap()
        .create_cq(cq_container.cq_entries)?;
    Ok(CreateCqResponse { cq_num })
}

pub fn uverbs_create_qp<'qp>(device_handle: usize, qp_container: &CreateQpRequest) -> Result<u32, &'static str> {
    let mut caps = qp_container.ib_caps;
    get_dev_list().lock()
        .get_mut(device_handle_to_idx(device_handle)).unwrap()
        .create_qp(qp_container.qp_type, qp_container.send_cq_num, qp_container.recv_cq_num, &mut caps)
}

pub fn uverbs_modify_qp(device_handle: usize, qp_modify_container: ModifyQpRequest) -> Result<(), &'static str> {
    get_dev_list().lock()
        .get_mut(device_handle_to_idx(device_handle)).unwrap()
        .modify_qp(qp_modify_container.qp_num, &qp_modify_container.attr, qp_modify_container.attr_mask,
    )
}

pub fn uverbs_poll_cq(device_handle: usize, cq_num: u32, wc: &mut [ibv_wc]) -> Result<usize, &'static str> {
    get_dev_list().lock()
        .get_mut(device_handle_to_idx(device_handle)).unwrap()
        .poll_cq(cq_num, wc)
}

pub fn uverbs_post_send(device_handle: usize, req: &PostSendRequest) -> Result<(), &'static str> {
    get_dev_list().lock()
        .get_mut(device_handle_to_idx(device_handle)).unwrap()
        .post_send(req.qp_num, &req.wrs)
}

pub fn uverbs_post_recv(device_handle: usize, req: &PostReceiveRequest) -> Result<(), &'static str> {
    get_dev_list().lock()
        .get_mut(device_handle_to_idx(device_handle)).unwrap()
        .post_receive(req.qp_num, &req.wrs)
}

pub fn uverbs_destroy(device_handle: usize, destroy_spec_fn: fn(&mut ConnectX3Nic, u32) -> Result<(), &'static str>, x_num: u32) -> Result<(), &'static str> {
    let mut device_list = get_dev_list().lock();
    let device = device_list.get_mut(device_handle_to_idx(device_handle)).unwrap();
    destroy_spec_fn(device, x_num)
}

// todo; map user address region into user space, let user ring doorbell
pub fn uverbs_mmap_uar() {}
