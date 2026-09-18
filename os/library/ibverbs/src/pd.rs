use alloc::vec::Vec;
use core::mem;
use core3::io;
use log::error;
use rdma::{AccessFlags, ProtectionDomainHandle, QueuePairCapabilities, QueuePairType};
use crate::{CompletionQueue, QueuePairBuilder};
use crate::context::Context;
use crate::mr::LocalMemoryRegion;

/// A protection domain for a device's context.
pub struct ProtectionDomain<'ctx> {
    pub ctx: &'ctx Context,
    pub pd: ProtectionDomainHandle,
}

unsafe impl<'a> Sync for ProtectionDomain<'a> {}
unsafe impl<'a> Send for ProtectionDomain<'a> {}

impl Drop for ProtectionDomain<'_> {
    fn drop(&mut self) {
        if let Err(e) = self.ctx.inner.dealloc_pd(self.pd) {
            error!("Failed to drop PD: {}", e.into_inner().unwrap_or("unkown reason"))
        };
    }
}

impl<'ctx> ProtectionDomain<'ctx> {
    /// Creates a queue pair builder associated with this protection domain.
    ///
    /// `send` and `recv` are the device `Context` to associate with the send and receive queues
    /// respectively. `send` and `recv` may refer to the same `Context`.
    ///
    /// `qp_type` indicates the requested Transport Service Type of this QP:
    ///
    ///  - `IBV_QPT_RC`: Reliable Connection
    ///  - `IBV_QPT_UC`: Unreliable Connection
    ///  - `IBV_QPT_UD`: Unreliable Datagram
    ///
    /// Note that both this protection domain, *and* both provided completion queues, must outlive
    /// the resulting `QueuePair`.
    pub fn create_qp<'pd, 'scq, 'rcq, 'res>(
        &'pd self,
        send: &'scq CompletionQueue,
        recv: &'rcq CompletionQueue,
        qp_type: QueuePairType,
    ) -> QueuePairBuilder<'res>
    where
        'scq: 'res,
        'rcq: 'res,
        'pd: 'res,
        'scq: 'ctx,
        'rcq: 'ctx,
        'pd: 'ctx,
        'res: 'ctx,
    {
        QueuePairBuilder::new(self, send, recv, qp_type)
    }

    /// Allocates and registers a Memory Region (MR) associated with this `ProtectionDomain`.
    ///
    /// This process allows the RDMA device to read and write data to the allocated memory. Only
    /// registered memory can be sent from and received to by `QueuePair`s. Performing this
    /// registration takes some time, so performing memory registration isn't recommended in the
    /// data path, when fast response is required.
    ///
    /// Every successful registration will result with a MR which has unique (within a specific
    /// RDMA device) `lkey` and `rkey` values. These keys must be communicated to the other end's
    /// `QueuePair` for direct memory access.
    ///
    /// The maximum size of the block that can be registered is limited to
    /// `device_attr.max_mr_size`. There isn't any way to know what is the total size of memory
    /// that can be registered for a specific device.
    ///
    /// `allocate` currently sets the following permissions for each new `MemoryRegion`:
    ///
    ///  - `IBV_ACCESS_LOCAL_WRITE`: Enables Local Write Access
    ///  - `IBV_ACCESS_REMOTE_WRITE`: Enables Remote Write Access
    ///  - `IBV_ACCESS_REMOTE_READ`: Enables Remote Read Access
    ///  - `IBV_ACCESS_REMOTE_ATOMIC`: Enables Remote Atomic Operation Access (if supported)
    ///
    /// Local read access is always enabled for the MR.
    ///
    /// # Panics
    ///
    /// Panics if the size of the memory region zero bytes, which can occur either if `n` is 0, or
    /// if `mem::size_of::<T>()` is 0.
    ///
    /// # Errors
    ///
    ///  - `EINVAL`: Invalid access value.
    ///  - `ENOMEM`: Not enough resources (either in operating system or in RDMA device) to
    ///    complete this operation.
    pub fn allocate<T: Sized + Copy + Default>(&self, n: usize) -> io::Result<LocalMemoryRegion<T>> {
        assert!(n > 0);
        assert!(mem::size_of::<T>() > 0);

        let mut data = Vec::with_capacity(n);
        data.resize(n, T::default());

        let access = AccessFlags::LOCAL_WRITE
            | AccessFlags::REMOTE_WRITE
            | AccessFlags::REMOTE_READ
            | AccessFlags::REMOTE_ATOMIC;

        // TODO
        // ibv_reg_mr()  returns  a  pointer to the registered MR, or NULL if the request fails.
        // The local key (L_Key) field lkey is used as the lkey field of struct ibv_sge when
        // posting buffers with ibv_post_* verbs, and the the remote key (R_Key)  field rkey  is
        // used by remote processes to perform Atomic and RDMA operations.  The remote process
        // places this rkey as the rkey field of struct ibv_send_wr passed to the ibv_post_send
        // function.

        LocalMemoryRegion::new(self, data, access)
    }
}

