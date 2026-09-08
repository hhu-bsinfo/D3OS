// Copied from rust-ibverbs 0.8.1

//! Rust API wrapping the `ibverbs` RDMA library.
//!
//! `libibverbs` is a library that allows userspace processes to use RDMA "verbs" to perform
//! high-throughput, low-latency network operations for both Infiniband (according to the
//! Infiniband specifications) and iWarp (iWARP verbs specifications). It handles the control path
//! of creating, modifying, querying and destroying resources such as Protection Domains,
//! Completion Queues, Queue-Pairs, Shared Receive Queues, Address Handles, and Memory Regions. It
//! also handles sending and receiving data posted to QPs and SRQs, and getting completions from
//! CQs using polling and completions events.
//!
//! A good place to start is to look at the programs in [`examples/`](examples/), and the upstream
//! [C examples]. You can test RDMA programs on modern Linux kernels even without specialized RDMA
//! hardware by using [SoftRoCE][soft].
//!
//! # For the detail-oriented
//!
//! The control path is implemented through system calls to the `uverbs` kernel module, which
//! further calls the low-level HW driver. The data path is implemented through calls made to
//! low-level HW library which, in most cases, interacts directly with the HW provides kernel and
//! network stack bypass (saving context/mode switches) along with zero copy and an asynchronous
//! I/O model.
//!
//! iWARP ethernet NICs support RDMA over hardware-offloaded TCP/IP, while InfiniBand is a general
//! high-throughput, low-latency networking technology. InfiniBand host channel adapters (HCAs) and
//! iWARP NICs commonly support direct hardware access from userspace (kernel bypass), and
//! `libibverbs` supports this when available.
//!
//! For more information on RDMA verbs, see the [InfiniBand Architecture Specification][infini]
//! vol. 1, especially chapter 11, and the RDMA Consortium's [RDMA Protocol Verbs
//! Specification][RFC5040]. See also the upstream [`libibverbs/verbs.h`] file for the original C
//! definitions, as well as the manpages for the `ibv_*` methods.
//!
//! # Library dependency
//!
//! `libibverbs` is usually available as a free-standing [library package]. It [used to be][1]
//! self-contained, but has recently been adopted into [`rdma-core`]. `cargo` will automatically
//! build the necessary library files and place them in `vendor/rdma-core/build/lib`. If a
//! system-wide installation is not available, those library files can be used instead by copying
//! them to `/usr/lib`, or by adding that path to the dynamic linking search path.
//!
//! # Thread safety
//!
//! All interfaces are `Sync` and `Send` since the underlying ibverbs API [is thread safe][safe].
//!
//! # Documentation
//!
//! Much of the documentation of this crate borrows heavily from the excellent posts over at
//! [RDMAmojo]. If you are going to be working a lot with ibverbs, chances are you will want to
//! head over there. In particular, [this overview post][1] may be a good place to start.
//!
//! [`rdma-core`]: https://github.com/linux-rdma/rdma-core
//! [`libibverbs/verbs.h`]: https://github.com/linux-rdma/rdma-core/blob/master/libibverbs/verbs.h
//! [library package]: https://launchpad.net/ubuntu/+source/libibverbs
//! [C examples]: https://github.com/linux-rdma/rdma-core/tree/master/libibverbs/examples
//! [1]: https://git.kernel.org/pub/scm/libs/infiniband/libibverbs.git/about/
//! [infini]: http://www.infinibandta.org/content/pages.php?pg=technology_public_specification
//! [RFC5040]: https://tools.ietf.org/html/rfc5040
//! [safe]: http://www.rdmamojo.com/2013/07/26/libibverbs-thread-safe-level/
//! [soft]: https://github.com/SoftRoCE/rxe-dev/wiki/rxe-dev:-Home
//! [RDMAmojo]: http://www.rdmamojo.com/
//! [1]: http://www.rdmamojo.com/2012/05/18/libibverbs/

#![no_std]
//#![deny(missing_docs)]
// avoid warnings about RDMAmojo, iWARP, InfiniBand, etc. not being in backticks
#![allow(clippy::doc_markdown)]

extern crate alloc;

use crate::provider::{get_available_devices, get_device_name, open_device, IbvCompletionQueue, IbvContext, IbvQueuePair};
use core::convert::TryInto;
use core::marker::PhantomData;
use core::mem;
use core::ops::{Deref, DerefMut};

pub use rdma::ib_core as ffi;
pub use queue_pair::*;
pub use completion_queue::*;

use alloc::sync::Arc;
use alloc::vec::Vec;
use core3::io;

const PORT_NUM: u8 = 1;

#[cfg(feature = "serialize")]
use bincode::{Decode, Encode};
use log::error;
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// Access flags for use with `QueuePair` and `MemoryRegion`.
pub use rdma::AccessFlags;
pub use rdma::{Gid, Mtu};
use rdma::QueuePairType;
use rdma::ProtectionDomainHandle;

/// Because `std::slice::SliceIndex` is still unstable, we follow @alexcrichton's suggestion in
/// https://github.com/rust-lang/rust/issues/35729 and implement it ourselves.
pub mod sliceindex;
pub mod cmd;
mod provider;
pub mod completion_queue;
pub mod queue_pair;

/// Get list of available RDMA devices.
///
/// # Errors
///
///  - `EPERM`: Permission denied.
///  - `ENOMEM`: Insufficient memory to complete the operation.
///  - `ENOSYS`: No kernel support for RDMA.
pub fn devices() -> io::Result<DeviceList> {
    Ok(DeviceList(get_available_devices()?))
}

/// List of available RDMA devices.
pub struct DeviceList(Vec<ffi::Device>);

unsafe impl Sync for DeviceList {}
unsafe impl Send for DeviceList {}

impl DeviceList {
    /// Returns an iterator over all found devices.
    pub fn iter(&self) -> DeviceListIter<'_> {
        DeviceListIter { list: self, i: 0 }
    }

    /// Returns the number of devices.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if there are any devices.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns the device at the given `index`, or `None` if out of bounds.
    pub fn get(&self, index: usize) -> Option<Device<'_>> {
        self.0.get(index).map(|d| d.into())
    }
}

impl<'a> IntoIterator for &'a DeviceList {
    type Item = <DeviceListIter<'a> as Iterator>::Item;
    type IntoIter = DeviceListIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        DeviceListIter { list: self, i: 0 }
    }
}

/// Iterator over a `DeviceList`.
pub struct DeviceListIter<'iter> {
    list: &'iter DeviceList,
    i: usize,
}

impl<'iter> Iterator for DeviceListIter<'iter> {
    type Item = Device<'iter>;
    fn next(&mut self) -> Option<Self::Item> {
        let e = self.list.0.get(self.i);
        if e.is_some() {
            self.i += 1;
        }
        e.map(|e| e.into())
    }
}

/// An RDMA device.
pub struct Device<'devlist>(&'devlist ffi::Device);
unsafe impl<'devlist> Sync for Device<'devlist> {}
unsafe impl<'devlist> Send for Device<'devlist> {}

impl<'d> From<&'d ffi::Device> for Device<'d> {
    fn from(d: &'d ffi::Device) -> Self {
        Device(d)
    }
}

/// A Global unique identifier for ibv.
///
/// This struct acts as a rust wrapper for GUID value represented as `__be64` in
/// libibverbs. We introduce this struct, because u64 is stored in host
/// endianness, whereas ibverbs stores GUID in network order (big endian).
#[cfg_attr(feature = "serialize", derive(Encode, Decode))]
#[derive(Default, Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct Guid {
    raw: [u8; 8],
}

impl Guid {
    /// Upper 24 bits of the GUID are OUI (Organizationally Unique Identifier,
    /// http://standards-oui.ieee.org/oui/oui.txt). The function returns OUI as
    /// a 24-bit number inside a u32.
    pub fn oui(&self) -> u32 {
        let padded = [0, self.raw[0], self.raw[1], self.raw[2]];
        u32::from_be_bytes(padded)
    }

    /// Returns `true` if this GUID is all zeroes, which is considered reserved.
    pub fn is_reserved(&self) -> bool {
        self.raw == [0; 8]
    }
}

impl From<u64> for Guid {
    fn from(guid: u64) -> Self {
        Self {
            raw: guid.to_be_bytes(),
        }
    }
}

impl From<Guid> for u64 {
    fn from(guid: Guid) -> Self {
        u64::from_be_bytes(guid.raw)
    }
}

impl<'devlist> Device<'devlist> {
    /// Opens an RMDA device and creates a context for further use.
    ///
    /// This context will later be used to query its resources or for creating resources.
    ///
    /// Unlike what the verb name suggests, it doesn't actually open the device. This device was
    /// opened by the kernel low-level driver and may be used by other user/kernel level code. This
    /// verb only opens a context to allow user level applications to use it.
    ///
    /// # Errors
    ///
    ///  - `EINVAL`: `PORT_NUM` is invalid (from `ibv_query_port_attr`).
    ///  - `ENOMEM`: Out of memory (from `ibv_query_port_attr`).
    ///  - `EMFILE`: Too many files are opened by this process (from `ibv_query_gid`).
    ///  - Other: the device is not in `ACTIVE` or `ARMED` state.
    pub fn open(&self) -> io::Result<Context> {
        Context::with_device(&self.0)
    }

    /// Query this device's port without opening a full context.
    ///
    /// [`Self::open`] refuses any port that is not `ACTIVE` or `ARMED`, which is right for
    /// anything that intends to transfer data but useless for diagnostics: it means the port
    /// cannot be inspected exactly when something has gone wrong with it. This skips both that
    /// check and the GID query (whose result is only defined for an active port).
    pub fn port_attr(&self) -> io::Result<ffi::PortAttr> {
        open_device(&self.0)?.query_port(PORT_NUM)
    }

    /// Returns a string of the name, which is associated with this RDMA device.
    ///
    /// This name is unique within a specific machine (the same name cannot be assigned to more
    /// than one device). However, this name isn't unique across an InfiniBand fabric (this name
    /// can be found in different machines).
    ///
    /// When there are more than one RDMA devices in a computer, changing the device location in
    /// the computer (i.e. in the PCI bus) may result a change in the names associated with the
    /// devices. In order to distinguish between the device, it is recommended using the device
    /// GUID, returned by `Device::guid`.
    ///
    /// The name is composed from:
    ///
    ///  - a *prefix* which describes the RDMA device vendor and model
    ///    - `cxgb3` - Chelsio Communications, T3 RDMA family
    ///    - `cxgb4` - Chelsio Communications, T4 RDMA family
    ///    - `ehca` - IBM, eHCA family
    ///    - `ipathverbs` - QLogic
    ///    - `mlx4` - Mellanox Technologies, ConnectX family
    ///    - `mthca` - Mellanox Technologies, InfiniHost family
    ///    - `nes` - Intel, Intel-NE family
    ///  - an *index* that helps to differentiate between several devices from the same vendor and
    ///    family in the same computer
    pub fn name(&self) -> Option<&str> {
        get_device_name(&self.0)
    }

    /// Returns the Global Unique IDentifier (GUID) of this RDMA device.
    ///
    /// This GUID, that was assigned to this device by its vendor during the manufacturing, is
    /// unique and can be used as an identifier to an RDMA device.
    ///
    /// From the prefix of the RDMA device GUID, one can know who is the vendor of that device
    /// using the [IEEE OUI](http://standards.ieee.org/develop/regauth/oui/oui.txt).
    ///
    /// # Errors
    ///
    ///  - `EMFILE`: Too many files are opened by this process.
    pub fn guid(&self) -> io::Result<Guid> {
        todo!("implement guid")
    }

    /// Returns stable IB device index as it is assigned by the kernel
    /// # Errors
    ///
    ///  - `ENOTSUP`: Stable index is not supported
    pub fn index(&self) -> io::Result<i32> {
        todo!("implement index")
    }
}

/// An RDMA context bound to a device.
pub struct Context {
    inner: Arc<dyn IbvContext>,
    port_attr: ffi::PortAttr,
    gid: Gid,
}

unsafe impl Sync for Context {}
unsafe impl Send for Context {}

impl Context {
    /// Opens a context for the given device, and queries its port and gid.
    fn with_device(dev: &ffi::Device) -> io::Result<Context> {

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
            ffi::PortState::Active | ffi::PortState::Armed => {}
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

    pub fn query_port(&self) -> &ffi::PortAttr {
        &self.port_attr
    }

    pub fn query_device(&self) -> io::Result<ffi::DeviceAttr> {
        self.inner.query_device()
    }
}


/// A (local) memory region that has been registered for use with RDMA.
pub struct LocalMemoryRegion<'pd, T> {
    pd: &'pd ProtectionDomain<'pd>,
    metadata: MemoryRegionMetadata,
    data: Vec<T>,
}

pub struct MemoryRegionMetadata {
    handle: u32,
    lkey: u32,
    rkey: u32,
}

unsafe impl<'pd, T> Send for LocalMemoryRegion<'pd, T> {}
unsafe impl<'pd, T> Sync for LocalMemoryRegion<'pd, T> {}


impl<'pd, T> Deref for LocalMemoryRegion<'pd, T> {
    type Target = [T];
    fn deref(&self) -> &Self::Target {
        &self.data[..]
    }
}

impl<'pd, T> DerefMut for LocalMemoryRegion<'pd, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data[..]
    }
}

impl<'pd, T> LocalMemoryRegion<'pd, T> {
    /// Get the remote authentication used to allow direct remote access to this memory region.
    pub fn remote(&mut self) -> RemoteMemoryRegion<T> {
        RemoteMemoryRegion {
            addr: self.data.as_mut_ptr() as u64,
            len: self.data.len() * mem::size_of::<T>(),
            rkey: self.metadata.rkey,
            phantom: PhantomData {},
        }
    }
}

/// A (remote) memory region that has been registered for use with RDMA.
///
/// Having this information authorizes direct memory access to a memory region.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serialize", derive(Encode, Decode))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RemoteMemoryRegion<T> {
    /// the remote pointer
    pub addr: u64,
    /// the length
    pub len: usize,
    /// the remote key
    pub rkey: u32,
    /// This holds the type.
    pub phantom: PhantomData<T>,
}

/// A protection domain for a device's context.
pub struct ProtectionDomain<'ctx> {
    ctx: &'ctx Context,
    pd: ProtectionDomainHandle,
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
        cap: ffi::QueuePairCapabilities
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
        QueuePairBuilder::new(self, send, recv, qp_type, cap)
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

        let access = ffi::AccessFlags::LOCAL_WRITE
            | ffi::AccessFlags::REMOTE_WRITE
            | ffi::AccessFlags::REMOTE_READ
            | ffi::AccessFlags::REMOTE_ATOMIC;

        let metadata = self.ctx.inner.reg_mr(
            self.pd,
            data.as_mut_ptr() as *mut _,
            data.len() * mem::size_of::<T>(),
            access,
        )?;

        // TODO
        // ibv_reg_mr()  returns  a  pointer to the registered MR, or NULL if the request fails.
        // The local key (L_Key) field lkey is used as the lkey field of struct ibv_sge when
        // posting buffers with ibv_post_* verbs, and the the remote key (R_Key)  field rkey  is
        // used by remote processes to perform Atomic and RDMA operations.  The remote process
        // places this rkey as the rkey field of struct ibv_send_wr passed to the ibv_post_send
        // function.

        Ok(LocalMemoryRegion {
            pd: self,
            metadata,
            data
        })
    }
}

/*#[cfg(all(test, feature = "serde"))]
mod test_serde {
    use super::*;
    #[test]
    fn encode_decode() {
        let qpe_default = QueuePairEndpoint {
            num: 72,
            lid: 9,
            gid: Some(Default::default()),
        };

        let mut qpe = qpe_default;
        qpe.gid.as_mut().unwrap().raw =
            unsafe { core::mem::transmute([87_u64.to_be(), 192_u64.to_be()]) };
        let encoded = bincode::serialize(&qpe).unwrap();

        let decoded: QueuePairEndpoint = bincode::deserialize(&encoded).unwrap();
        assert_eq!(decoded.gid.unwrap().subnet_prefix(), 87);
        assert_eq!(decoded.gid.unwrap().interface_id(), 192);
        assert_eq!(qpe, decoded);
        assert_ne!(qpe, qpe_default);
    }

    #[test]
    fn encode_decode_guid() {
        let guid_u64 = 0x12_34_56_78_9a_bc_de_f0_u64;
        let _be: ffi::__be64 = guid_u64.to_be();
        let guid: Guid = guid_u64.into();

        assert_eq!(guid.is_reserved(), false);
        assert_eq!(guid.raw, [0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0]);
        println!("{:#08x}", guid.oui());
        assert_eq!(guid.oui(), 0x123456);
    }
} */
