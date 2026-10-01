use crate::device::infiniband::mlx4::{Mlx4Device, device_handle_to_idx, get_dev_list};
use crate::process::process::Process;
use alloc::vec::Vec;
use rdma::uverbs_uapi::{
    AllocPdRequest, AllocPdResponse, CreateCqRequest, CreateCqResponse, CreateMrRequest, CreateMrResponse, CreateQpRequest, CreateQpResponse,
    DeallocPdRequest, DestroyRequest, ModifyQpRequest, OpenDeviceResponse, UserSlice,
};
use rdma::{ContextHandle, DeviceAttr, DeviceHandle, PortAttr};

const DEVICE_NOT_FOUND: &'static str = "Device not found";

pub fn uverbs_query_devices(max_len: usize) -> Vec<DeviceHandle> {
    get_dev_list().lock().iter().map(|dev| DeviceHandle::from(dev.handle)).take(max_len).collect()
}

pub fn uverbs_open_device(device_handle: usize, process: &Process) -> Result<OpenDeviceResponse, &'static str> {
    let mut device_list = get_dev_list().lock();
    let ctx = device_list.get_mut(device_handle_to_idx(device_handle)).ok_or(DEVICE_NOT_FOUND)?.open(process)?;
    Ok(OpenDeviceResponse {
        context: ctx.handle(),
        doorbell_page: ctx.doorbell_page().start_address().as_mut_ptr(),
        blueflame_page: ctx.blueflame_page().start_address().as_mut_ptr(),
    })
}

pub fn uverbs_query_device(device_handle: usize) -> Result<DeviceAttr, &'static str> {
    get_dev_list().lock().get_mut(device_handle_to_idx(device_handle))
        .ok_or(DEVICE_NOT_FOUND)?
        .query_device()
}

pub fn uverbs_query_port(device_handle: usize, port_num: u8) -> Result<PortAttr, &'static str> {
    get_dev_list().lock().get_mut(device_handle_to_idx(device_handle))
        .ok_or(DEVICE_NOT_FOUND)?
        .query_port(port_num)
}

pub fn uverbs_register_mem_region(
    device_handle: usize, process: &Process, req: &CreateMrRequest,
) -> Result<CreateMrResponse, &'static str> {
    get_dev_list()
        .lock()
        .get_mut(device_handle_to_idx(device_handle))
        .ok_or(DEVICE_NOT_FOUND)?
        .create_mr(process, req.context, req.pd, UserSlice::new(req.data_ptr, req.len as usize), req.access_flags)
        .map(|metadata| CreateMrResponse { metadata })
        .map_err(|_| "failed to create memory region")
}

pub fn uverbs_create_cq(device_handle: usize, process: &Process, req: &CreateCqRequest) -> Result<CreateCqResponse, &'static str> {
    let cq_num =
        get_dev_list()
            .lock()
            .get_mut(device_handle_to_idx(device_handle))
            .ok_or(DEVICE_NOT_FOUND)?
            .create_cq(process, req.context, req.cq_entries, req.buffer, req.doorbell_ptr)?;
    Ok(CreateCqResponse { cq_num })
}

pub fn uverbs_create_qp(device_handle: usize, process: &Process, req: &CreateQpRequest) -> Result<CreateQpResponse, &'static str> {
    let qp_num = get_dev_list().lock().get_mut(device_handle_to_idx(device_handle))
        .ok_or(DEVICE_NOT_FOUND)?
        .create_qp(
            process,
            req.context,
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
    Ok(CreateQpResponse { qp_num })
}

pub fn uverbs_modify_qp(device_handle: usize, process: &Process, qp_modify_container: ModifyQpRequest) -> Result<(), &'static str> {
    get_dev_list().lock().get_mut(device_handle_to_idx(device_handle))
        .ok_or(DEVICE_NOT_FOUND)?
        .modify_qp(
            process,
            qp_modify_container.context,
            qp_modify_container.qp_num,
            &qp_modify_container.attr,
            qp_modify_container.attr_mask,
        )
}

pub fn uverbs_destroy(
    device_handle: usize, process: &Process, destroy_spec_fn: fn(&mut Mlx4Device, &Process, ContextHandle, u32) -> Result<(), &'static str>,
    req: DestroyRequest,
) -> Result<(), &'static str> {
    let mut device_list = get_dev_list().lock();
    let device = device_list.get_mut(device_handle_to_idx(device_handle))
        .ok_or(DEVICE_NOT_FOUND)?;
    destroy_spec_fn(device, process, req.context, req.handle)
}

pub fn uverbs_alloc_pd(device_handle: usize, process: &Process, req: AllocPdRequest) -> Result<AllocPdResponse, &'static str> {
    let mut device_list = get_dev_list().lock();
    let device = device_list.get_mut(device_handle_to_idx(device_handle))
        .ok_or(DEVICE_NOT_FOUND)?;
    let pd = device.alloc_pd(process, req.context)?;
    Ok(AllocPdResponse { pd })
}

pub fn uverbs_dealloc_pd(device_handle: usize, process: &Process, req: DeallocPdRequest) -> Result<(), &'static str> {
    let mut device_list = get_dev_list().lock();
    let device = device_list.get_mut(device_handle_to_idx(device_handle))
        .ok_or(DEVICE_NOT_FOUND)?;
    device.dealloc_pd(process, req.context, req.pd)
}
