use crate::device::mlx4::queue_pair::{QueuePairCapabilities, QueuePairType};
use crate::device::mlx4::{get_dev_list, minor_to_idx, AccessFlags, ConnectX3Nic};
use alloc::vec::Vec;
use rdma::ibv_qp_type::Type;
use rdma::uverbs_uapi::{CreateCqRequest, CreateCqResponse, CreateMrResponse, CreateQpRequest, ModifyQpRequest, PostReceiveRequest, PostSendRequest};
use rdma::{ibv_access_flags, ibv_device, ibv_device_attr, ibv_port_attr, ibv_wc};

pub fn uverbs_query_devices(max_len: usize) -> Vec<ibv_device> {
    get_dev_list().lock().iter()
        .map(|dev| ibv_device { nic: dev.minor } )
        .take(max_len)
        .collect()
}

pub fn uverbs_query_device(minor: usize) -> Result<ibv_device_attr, &'static str> {
    get_dev_list().lock()
        .get_mut(minor_to_idx(minor)).unwrap()
        .query_device()
}

pub fn uverbs_query_port(minor: usize, port_num: u8) -> Result<ibv_port_attr, &'static str> {
    get_dev_list().lock()
        .get_mut(minor_to_idx(minor)).unwrap()
        .query_port(port_num)
}

pub fn uverbs_register_mem_region(minor: usize, access_flags: ibv_access_flags, user_data_ref: &mut [u8]) -> Result<CreateMrResponse, &'static str> {
    // TODO this assumes that ibv_access_flags and AccessFlags have the same bit layout
    let access_flags = AccessFlags::from_bits(access_flags.bits()).ok_or("Invalid access flags for uverbs")?;
    get_dev_list().lock().get_mut(minor_to_idx(minor)).unwrap()
        .create_mr(user_data_ref, access_flags)
        .map(CreateMrResponse::from)
        .map_err(|_| "failed to create memory region")
}

pub fn uverbs_create_cq<'cq>(minor: usize, cq_container: &'cq CreateCqRequest) -> Result<CreateCqResponse, &'static str> {
    let cq_num = get_dev_list().lock()
        .get_mut(minor_to_idx(minor)).unwrap()
        .create_cq(cq_container.cq_entries)?;
    Ok(CreateCqResponse { cq_num })
}

pub fn uverbs_create_qp<'qp>(minor: usize, qp_container: &CreateQpRequest) -> Result<u32, &'static str> {
    let qp_type: QueuePairType = match qp_container.qp_type {
        Type::IBV_QPT_RC => QueuePairType::ReliableConnection,
        Type::IBV_QPT_UC => QueuePairType::UnreliableConnection,
        Type::IBV_QPT_UD => QueuePairType::UnreliableDatagram,
        _ => return Err("unsupported qp type"),
    };

    let mut caps: QueuePairCapabilities = QueuePairCapabilities {
        max_send_wr: qp_container.ib_caps.max_send_wr,
        max_recv_wr: qp_container.ib_caps.max_recv_wr,
        max_send_sge: qp_container.ib_caps.max_send_sge,
        max_recv_sge: qp_container.ib_caps.max_recv_sge,
        max_inline_data: qp_container.ib_caps.max_inline_data,
    };

    get_dev_list().lock()
        .get_mut(minor_to_idx(minor)).unwrap()
        .create_qp(qp_type, qp_container.send_cq_num, qp_container.recv_cq_num, &mut caps)
}

pub fn uverbs_modify_qp(minor: usize, qp_modify_container: ModifyQpRequest) -> Result<(), &'static str> {
    get_dev_list().lock()
        .get_mut(minor_to_idx(minor)).unwrap()
        .modify_qp(qp_modify_container.qp_num, &qp_modify_container.attr, qp_modify_container.attr_mask,
    )
}

pub fn uverbs_poll_cq(minor: usize, cq_num: u32, wc: &mut [ibv_wc]) -> Result<usize, &'static str> {
    get_dev_list().lock()
        .get_mut(minor_to_idx(minor)).unwrap()
        .poll_cq(cq_num, wc)
}

pub fn uverbs_post_send(minor: usize, req: &PostSendRequest) -> Result<(), &'static str> {
    get_dev_list().lock()
        .get_mut(minor_to_idx(minor)).unwrap()
        .post_send(req.qp_num, &req.wrs)
}

pub fn uverbs_post_recv(minor: usize, req: &PostReceiveRequest) -> Result<(), &'static str> {
    get_dev_list().lock()
        .get_mut(minor_to_idx(minor)).unwrap()
        .post_receive(req.qp_num, &req.wrs)
}

pub fn uverbs_destroy(minor: usize, destroy_spec_fn: fn(&mut ConnectX3Nic, u32) -> Result<(), &'static str>, x_num: u32) -> Result<(), &'static str> {
    let mut device_list = get_dev_list().lock();
    let device = device_list.get_mut(minor_to_idx(minor)).unwrap();
    destroy_spec_fn(device, x_num)
}

// todo; map user address region into user space, let user ring doorbell
pub fn uverbs_mmap_uar() {}
