use crate::device::mlx4::{get_dev_list, device_handle_to_idx, ConnectX3Nic};
use alloc::vec::Vec;
use rdma::uverbs_uapi::{CreateCqRequest, CreateCqResponse, CreateMrResponse, AllocPdResponse, CreateQpRequest, CreateQpResponse, DeallocPdRequest, ModifyQpRequest};
use rdma::{ibv_access_flags, ibv_device, ibv_device_attr, ibv_port_attr};

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

pub fn uverbs_register_mem_region(device_handle: usize, pd: u32, access_flags: ibv_access_flags, user_data_ref: &mut [u8]) -> Result<CreateMrResponse, &'static str> {
    get_dev_list().lock().get_mut(device_handle_to_idx(device_handle)).unwrap()
        .create_mr(pd, user_data_ref, access_flags)
        .map(|d| {
            CreateMrResponse { handle: d.handle(), lkey: d.lkey(), rkey: d.rkey() }
        })
        .map_err(|_| "failed to create memory region")
}

pub fn uverbs_create_cq<'cq>(device_handle: usize, cq_container: &'cq CreateCqRequest) -> Result<CreateCqResponse, &'static str> {
    let (cq_num, doorbell_page) = get_dev_list().lock()
        .get_mut(device_handle_to_idx(device_handle)).unwrap()
        .create_cq(cq_container.cq_entries, cq_container.buffer, cq_container.doorbell_ptr)?;
    Ok(CreateCqResponse { cq_num, doorbell_page })
}

pub fn uverbs_create_qp<'qp>(device_handle: usize, req: &CreateQpRequest) -> Result<CreateQpResponse, &'static str> {
    let (qp_num, doorbell_page, blueflame_page) = get_dev_list().lock()
        .get_mut(device_handle_to_idx(device_handle)).unwrap()
        .create_qp(
            req.pd,
            req.qp_type,
            req.send_cq_num,
            req.recv_cq_num,
            req.buffer,
            req.doorbell_ptr,
            req.log_sq_bb_count,
            req.log_sq_stride,
            req.log_rq_wqe_count,
            req.log_rq_stride,
        )?;
    Ok(CreateQpResponse { qp_num, doorbell_page, blueflame_page })
}

pub fn uverbs_modify_qp(device_handle: usize, qp_modify_container: ModifyQpRequest) -> Result<(), &'static str> {
    get_dev_list().lock()
        .get_mut(device_handle_to_idx(device_handle)).unwrap()
        .modify_qp(qp_modify_container.qp_num, &qp_modify_container.attr, qp_modify_container.attr_mask,
    )
}

/// Drain the device's event queue, returning how many events were handled.
///
/// See [`ConnectX3Nic::drain_events`]. Posting and polling now happen entirely in userspace
/// against mapped memory, without a syscall per operation, so userspace calls this itself
/// (rate-limited) from its poll loop instead of it piggybacking on another verb.
pub fn uverbs_drain_events(device_handle: usize) -> usize {
    get_dev_list().lock()
        .get_mut(device_handle_to_idx(device_handle)).unwrap()
        .drain_events()
}

pub fn uverbs_destroy(device_handle: usize, destroy_spec_fn: fn(&mut ConnectX3Nic, u32) -> Result<(), &'static str>, x_num: u32) -> Result<(), &'static str> {
    let mut device_list = get_dev_list().lock();
    let device = device_list.get_mut(device_handle_to_idx(device_handle)).unwrap();
    destroy_spec_fn(device, x_num)
}

pub fn uverbs_alloc_pd(device_handle: usize) -> Result<AllocPdResponse, &'static str> {
    let mut device_list = get_dev_list().lock();
    let device = device_list.get_mut(device_handle_to_idx(device_handle)).unwrap();
    let pd = device.alloc_pd()?;
    Ok(AllocPdResponse { pd })
}

pub fn uverbs_dealloc_qp(device_handle: usize, req: DeallocPdRequest) -> Result<(), &'static str> {
    let mut device_list = get_dev_list().lock();
    let device = device_list.get_mut(device_handle_to_idx(device_handle)).unwrap();
    device.dealloc_pd(req.pd)
}


// todo; map user address region into user space, let user ring doorbell
pub fn uverbs_mmap_uar() {}
