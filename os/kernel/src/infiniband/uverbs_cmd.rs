use crate::device::mlx4::{get_dev_list, device_handle_to_idx, ConnectX3Nic};
use alloc::vec::Vec;
use rdma::uverbs_uapi::{AllocPdResponse, CreateCqRequest, CreateCqResponse, CreateMrResponse, CreateQpRequest, CreateQpResponse, DeallocPdRequest, ModifyQpRequest, OpenDeviceResponse};
use rdma::{AccessFlags,DeviceAttr, DeviceHandle, PortAttr, ProtectionDomainHandle};
use crate::process_manager;

pub fn uverbs_query_devices(max_len: usize) -> Vec<DeviceHandle> {
    get_dev_list().lock().iter()
        .map(|dev| DeviceHandle::from(dev.handle) )
        .take(max_len)
        .collect()
}

pub fn uverbs_open_device(device_handle: usize) -> Result<OpenDeviceResponse, &'static str> {
    let process = process_manager().read().current_process();
    let mut device_list = get_dev_list().lock();
    let dev = device_list.get_mut(device_handle_to_idx(device_handle)).unwrap();
    let ctx = dev.open()?;
    Ok(OpenDeviceResponse {
        uar_index: ctx.uar_page.index() as u32,
        doorbell_page: ctx.uar_page.map_doorbell_page(&process)?.start_address().as_mut_ptr(),
        blueflame_page: ctx.uar_page.map_blueflame_page(&process)?.start_address().as_mut_ptr(),
    })
}

pub fn uverbs_query_device(device_handle: usize) -> Result<DeviceAttr, &'static str> {
    get_dev_list().lock()
        .get_mut(device_handle_to_idx(device_handle)).unwrap()
        .query_device()
}

pub fn uverbs_query_port(device_handle: usize, port_num: u8) -> Result<PortAttr, &'static str> {
    get_dev_list().lock()
        .get_mut(device_handle_to_idx(device_handle)).unwrap()
        .query_port(port_num)
}

pub fn uverbs_register_mem_region(device_handle: usize, pd: ProtectionDomainHandle, access_flags: AccessFlags, user_data_ref: &mut [u8]) -> Result<CreateMrResponse, &'static str> {
    get_dev_list().lock().get_mut(device_handle_to_idx(device_handle)).unwrap()
        .create_mr(pd, user_data_ref, access_flags)
        .map(|d| {
            CreateMrResponse { handle: d.handle(), lkey: d.lkey(), rkey: d.rkey() }
        })
        .map_err(|_| "failed to create memory region")
}

pub fn uverbs_create_cq<'cq>(device_handle: usize, req: &'cq CreateCqRequest) -> Result<CreateCqResponse, &'static str> {
    let cq_num = get_dev_list().lock()
        .get_mut(device_handle_to_idx(device_handle)).unwrap()
        .create_cq(req.cq_entries, req.buffer, req.doorbell_ptr, req.uar_index)?;
    Ok(CreateCqResponse { cq_num })
}

pub fn uverbs_create_qp<'qp>(device_handle: usize, req: &CreateQpRequest) -> Result<CreateQpResponse, &'static str> {
    let qp_num = get_dev_list().lock()
        .get_mut(device_handle_to_idx(device_handle)).unwrap()
        .create_qp(
            req.pd,
            req.qp_type,
            req.send_cq_num,
            req.recv_cq_num,
            req.buffer,
            req.doorbell_ptr,
            req.uar_index,
            req.log_sq_bb_count,
            req.log_sq_stride,
            req.log_rq_wqe_count,
            req.log_rq_stride,
        )?;
    Ok(CreateQpResponse { qp_num })
}

pub fn uverbs_modify_qp(device_handle: usize, qp_modify_container: ModifyQpRequest) -> Result<(), &'static str> {
    get_dev_list().lock()
        .get_mut(device_handle_to_idx(device_handle)).unwrap()
        .modify_qp(qp_modify_container.qp_num, &qp_modify_container.attr, qp_modify_container.attr_mask,
    )
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
