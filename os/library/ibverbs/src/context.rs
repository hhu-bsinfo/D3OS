use alloc::sync::Arc;
use core3::io;
use rdma::{DeviceAttr, Gid, PortAttr, PortState};
use crate::{CompletionQueue, PORT_NUM};
use crate::device::Device;
use crate::pd::ProtectionDomain;
use crate::provider::{open_device, IbvContext};

/// An RDMA context bound to a device.
pub struct Context {
    pub inner: Arc<dyn IbvContext>,
    pub(crate) port_attr: PortAttr,
    pub(crate) gid: Gid,
}

unsafe impl Sync for Context {}
unsafe impl Send for Context {}

impl Context {

    /// Opens a context for the given device, and queries its port and gid.
    pub(crate) fn with_device(dev: &Device) -> io::Result<Context> {

        let ctx = open_device(dev)?;

        // TODO: from http://www.rdmamojo.com/2012/07/21/ibv_query_port/
        //
        //   Most of the port attributes, returned by ibv_query_port(), aren't constant and may be
        //   changed, mainly by the SM (in InfiniBand), or by the Hardware. It is highly
        //   recommended avoiding saving the result of this query, or to flush them when a new SM
        //   (re)configures the subnet.
        //
        let port_attr = ctx.query_port(PORT_NUM)?;

        // From http://www.rdmamojo.com/2012/08/02/ibv_query_gid/:
        //
        //   The content of the GID table is valid only when the port_attr.state is either
        //   IBV_PORT_ARMED or IBV_PORT_ACTIVE. For other states of the port, the value of the GID
        //   table is indeterminate.
        //
        match port_attr.state {
            PortState::Active | PortState::Armed => {}
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "port is not ACTIVE or ARMED",
                ));
            }
        }

        let gid = ctx.query_gid(PORT_NUM, 0)?.into();

        Ok(Context {
            inner: Arc::from(ctx),
            port_attr,
            gid,
        })
    }

    /// Create a completion queue (CQ).
    ///
    /// When an outstanding Work Request, within a Send or Receive Queue, is completed, a Work
    /// Completion is being added to the CQ of that Work Queue. This Work Completion indicates that
    /// the outstanding Work Request has been completed (and no longer considered outstanding) and
    /// provides details on it (status, direction, opcode, etc.).
    ///
    /// A single CQ can be shared for sending, receiving, and sharing across multiple QPs. The Work
    /// Completion holds the information to specify the QP number and the Queue (Send or Receive)
    /// that it came from.
    ///
    /// `min_cq_entries` defines the minimum size of the CQ. The actual created size can be equal
    /// or higher than this value. `id` is an opaque identifier that is echoed by
    /// `CompletionQueue::poll`.
    ///
    /// # Errors
    ///
    ///  - `EINVAL`: Invalid `min_cq_entries` (must be `1 <= cqe <= dev_cap.max_cqe`).
    ///  - `ENOMEM`: Not enough resources to complete this operation.
    pub fn create_cq(&self, min_cq_entries: i32, id: isize) -> io::Result<CompletionQueue> {
        let cq = self.inner.clone().create_cq(
            min_cq_entries,
            id,
            None,
            0,
        )?;

        Ok(CompletionQueue { inner: Arc::from(cq) })
    }

    /// Allocate a protection domain (PDs) for the device's context.
    ///
    /// The created PD will be used primarily to create `QueuePair`s and `MemoryRegion`s.
    ///
    /// A protection domain is a means of protection, and helps you create a group of object that
    /// can work together. If several objects were created using PD1, and others were created using
    /// PD2, working with objects from group1 together with objects from group2 will not work.
    pub fn alloc_pd(&self) -> io::Result<ProtectionDomain<'_>> {
        let pd = self.inner.alloc_pd()?;
        Ok(ProtectionDomain { ctx: self, pd })
    }

    pub fn query_port(&self) -> &PortAttr {
        todo!("query_port")
    }

    pub fn query_device(&self) -> io::Result<DeviceAttr> {
        self.inner.query_device()
    }
}

