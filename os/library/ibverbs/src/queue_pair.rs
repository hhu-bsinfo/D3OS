use alloc::sync::Arc;
use alloc::vec::Vec;
use core::mem;
use core::ops::Range;
use core3::io;
use rdma::{Gid, Mtu, QueuePairType, ScatterGatherEntry};
use crate::completion_queue::CompletionQueue;
use crate::{ffi, sliceindex, Context, LocalMemoryRegion, ProtectionDomain, RemoteMemoryRegion, PORT_NUM};
use crate::provider::{IbvQueuePair, QpInitAttr, ReceiveWorkRequest, SendWorkRequest};

#[cfg(feature = "serialize")]
use bincode::{Decode, Encode};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// An unconfigured `QueuePair`.
///
/// A `QueuePairBuilder` is used to configure a `QueuePair` before it is allocated and initialized.
/// To construct one, use `ProtectionDomain::create_qp`. See also [RDMAmojo] for many more details.
///
/// [RDMAmojo]: http://www.rdmamojo.com/2013/01/12/ibv_modify_qp/
pub struct QueuePairBuilder<'res> {
    ctx: isize,
    pd: &'res ProtectionDomain<'res>,

    send: &'res CompletionQueue,
    recv: &'res CompletionQueue,

    cap: ffi::QueuePairCapabilities,

    qp_type: QueuePairType,

    // carried along to handshake phase
    /// only valid for RC and UC
    access: Option<ffi::AccessFlags>,
    /// only valid for RC
    timeout: Option<u8>,
    /// only valid for RC
    retry_count: Option<u8>,
    /// only valid for RC
    rnr_retry: Option<u8>,
    /// only valid for RC
    min_rnr_timer: Option<u8>,
    /// only valid for RC
    max_rd_atomic: Option<u8>,
    /// only valid for RC
    max_dest_rd_atomic: Option<u8>,
    /// only valid for RC and UC
    path_mtu: Option<Mtu>,
    /// only valid for RC and UC
    rq_psn: Option<u32>,
}

impl<'res> QueuePairBuilder<'res> {
    /// Prepare a new `QueuePair` builder.
    ///
    /// `max_send_wr` is the maximum number of outstanding Work Requests that can be posted to the
    /// Send Queue in that Queue Pair. Value must be in `[0..dev_cap.max_qp_wr]`. There may be RDMA
    /// devices that for specific transport types may support less outstanding Work Requests than
    /// the maximum reported value.
    ///
    /// Similarly, `max_recv_wr` is the maximum number of outstanding Work Requests that can be
    /// posted to the Receive Queue in that Queue Pair. Value must be in `[0..dev_cap.max_qp_wr]`.
    /// There may be RDMA devices that for specific transport types may support less outstanding
    /// Work Requests than the maximum reported value. This value is ignored if the Queue Pair is
    /// associated with an SRQ
    pub(super) fn new<'scq, 'rcq, 'pd, 'ctx>(
        pd: &'pd ProtectionDomain<'ctx>,
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
        let path_mtu = (qp_type == QueuePairType::RC
            || qp_type == QueuePairType::UC)
            .then_some(pd.ctx.port_attr.active_mtu);
        QueuePairBuilder {
            ctx: 0,
            pd,

            send,
            cap,
            recv,

            qp_type,

            access: (qp_type == QueuePairType::RC
                || qp_type == QueuePairType::UC)
                .then_some(ffi::AccessFlags::LOCAL_WRITE),
            min_rnr_timer: (qp_type == QueuePairType::RC).then_some(16),
            retry_count: (qp_type == QueuePairType::RC).then_some(6),
            rnr_retry: (qp_type == QueuePairType::RC).then_some(6),
            timeout: (qp_type == QueuePairType::RC).then_some(4),
            max_rd_atomic: (qp_type == QueuePairType::RC).then_some(1),
            max_dest_rd_atomic: (qp_type == QueuePairType::RC).then_some(1),
            path_mtu,
            rq_psn: (qp_type == QueuePairType::RC
                || qp_type == QueuePairType::UC)
                .then_some(0),
        }
    }

    /// Set the access flags for the new `QueuePair`.
    ///
    /// Valid only for RC and UC QPs.
    ///
    /// Defaults to `IBV_ACCESS_LOCAL_WRITE`.
    pub fn set_access(&mut self, access: ffi::AccessFlags) -> &mut Self {
        if self.qp_type == QueuePairType::RC
            || self.qp_type == QueuePairType::UC
        {
            self.access = Some(access);
        }
        self
    }

    /// Set the access flags of the new `QueuePair` such that it allows remote reads and writes.
    ///
    /// Valid only for RC and UC QPs.
    pub fn allow_remote_rw(&mut self) -> &mut Self {
        if self.qp_type == QueuePairType::RC
            || self.qp_type == QueuePairType::UC
        {
            self.access = Some(
                self.access.expect("always set to Some in new")
                    | ffi::AccessFlags::REMOTE_WRITE
                    | ffi::AccessFlags::REMOTE_READ,
            );
        }
        self
    }

    /// Sets the minimum RNR NAK Timer Field Value for the new `QueuePair`.
    ///
    /// Defaults to 16 (2.56 ms delay).
    /// Valid only for RC QPs.
    ///
    /// When an incoming message to this QP should consume a Work Request from the Receive Queue,
    /// but no Work Request is outstanding on that Queue, the QP will send an RNR NAK packet to
    /// the initiator. It does not affect RNR NAKs sent for other reasons. The value must be one of
    /// the following values:
    ///
    ///  - 0 - 655.36 ms delay
    ///  - 1 - 0.01 ms delay
    ///  - 2 - 0.02 ms delay
    ///  - 3 - 0.03 ms delay
    ///  - 4 - 0.04 ms delay
    ///  - 5 - 0.06 ms delay
    ///  - 6 - 0.08 ms delay
    ///  - 7 - 0.12 ms delay
    ///  - 8 - 0.16 ms delay
    ///  - 9 - 0.24 ms delay
    ///  - 10 - 0.32 ms delay
    ///  - 11 - 0.48 ms delay
    ///  - 12 - 0.64 ms delay
    ///  - 13 - 0.96 ms delay
    ///  - 14 - 1.28 ms delay
    ///  - 15 - 1.92 ms delay
    ///  - 16 - 2.56 ms delay
    ///  - 17 - 3.84 ms delay
    ///  - 18 - 5.12 ms delay
    ///  - 19 - 7.68 ms delay
    ///  - 20 - 10.24 ms delay
    ///  - 21 - 15.36 ms delay
    ///  - 22 - 20.48 ms delay
    ///  - 23 - 30.72 ms delay
    ///  - 24 - 40.96 ms delay
    ///  - 25 - 61.44 ms delay
    ///  - 26 - 81.92 ms delay
    ///  - 27 - 122.88 ms delay
    ///  - 28 - 163.84 ms delay
    ///  - 29 - 245.76 ms delay
    ///  - 30 - 327.68 ms delay
    ///  - 31 - 491.52 ms delay
    pub fn set_min_rnr_timer(&mut self, timer: u8) -> &mut Self {
        if self.qp_type == QueuePairType::RC {
            self.min_rnr_timer = Some(timer);
        }
        self
    }

    /// Sets the minimum timeout that the new `QueuePair` waits for ACK/NACK from remote QP before
    /// retransmitting the packet.
    ///
    /// Defaults to 4 (65.536µs).
    /// Valid only for RC QPs.
    ///
    /// The value zero is special value that waits an infinite time for the ACK/NACK (useful
    /// for debugging). This means that if any packet in a message is being lost and no ACK or NACK
    /// is being sent, no retry will ever occur and the QP will just stop sending data.
    ///
    /// For any other value of timeout, the time calculation is `4.096*2^timeout`µs, giving:
    ///
    ///  - 0 - infinite
    ///  - 1 - 8.192 µs
    ///  - 2 - 16.384 µs
    ///  - 3 - 32.768 µs
    ///  - 4 - 65.536 µs
    ///  - 5 - 131.072 µs
    ///  - 6 - 262.144 µs
    ///  - 7 - 524.288 µs
    ///  - 8 - 1.048 ms
    ///  - 9 - 2.097 ms
    ///  - 10 - 4.194 ms
    ///  - 11 - 8.388 ms
    ///  - 12 - 16.777 ms
    ///  - 13 - 33.554 ms
    ///  - 14 - 67.108 ms
    ///  - 15 - 134.217 ms
    ///  - 16 - 268.435 ms
    ///  - 17 - 536.870 ms
    ///  - 18 - 1.07 s
    ///  - 19 - 2.14 s
    ///  - 20 - 4.29 s
    ///  - 21 - 8.58 s
    ///  - 22 - 17.1 s
    ///  - 23 - 34.3 s
    ///  - 24 - 68.7 s
    ///  - 25 - 137 s
    ///  - 26 - 275 s
    ///  - 27 - 550 s
    ///  - 28 - 1100 s
    ///  - 29 - 2200 s
    ///  - 30 - 4400 s
    ///  - 31 - 8800 s
    pub fn set_timeout(&mut self, timeout: u8) -> &mut Self {
        if self.qp_type == QueuePairType::RC {
            self.timeout = Some(timeout);
        }
        self
    }

    /// Sets the total number of times that the new `QueuePair` will try to resend the packets
    /// before reporting an error because the remote side doesn't answer in the primary path.
    ///
    /// This 3 bit value defaults to 6.
    /// Valid only for RC QPs.
    ///
    /// # Panics
    ///
    /// Panics if a count higher than 7 is given.
    pub fn set_retry_count(&mut self, count: u8) -> &mut Self {
        if self.qp_type == QueuePairType::RC {
            assert!(count <= 7);
            self.retry_count = Some(count);
        }
        self
    }

    /// Sets the total number of times that the new `QueuePair` will try to resend the packets when
    /// an RNR NACK was sent by the remote QP before reporting an error.
    ///
    /// This 3 bit value defaults to 6. The value 7 is special and specify to retry sending the
    /// message indefinitely when a RNR Nack is being sent by remote side.
    /// Valid only for RC QPs.
    ///
    /// # Panics
    ///
    /// Panics if a limit higher than 7 is given.
    pub fn set_rnr_retry(&mut self, n: u8) -> &mut Self {
        if self.qp_type == QueuePairType::RC {
            assert!(n <= 7);
            self.rnr_retry = Some(n);
        }
        self
    }

    /// Set the number of outstanding RDMA reads & atomic operations on the destination Queue Pair.
    ///
    /// This defaults to 1.
    /// Valid only for RC QPs.
    pub fn set_max_rd_atomic(&mut self, max_rd_atomic: u8) -> &mut Self {
        if self.qp_type == QueuePairType::RC {
            self.max_rd_atomic = Some(max_rd_atomic);
        }
        self
    }

    /// Set the number of responder resources for handling incoming RDMA reads & atomic operations.
    ///
    /// This defaults to 1.
    /// Valid only for RC QPs.
    pub fn set_max_dest_rd_atomic(&mut self, max_dest_rd_atomic: u8) -> &mut Self {
        if self.qp_type == QueuePairType::RC {
            self.max_dest_rd_atomic = Some(max_dest_rd_atomic);
        }
        self
    }

    /// Set the path MTU.
    ///
    /// Defaults to the port's active_mtu.
    /// Valid only for RC and UC QPs.
    /// The possible values are:
    ///  - 1: 256
    ///  - 2: 512
    ///  - 3: 1024
    ///  - 4: 2048
    ///  - 5: 4096
    pub fn set_path_mtu(&mut self, path_mtu: Mtu) -> &mut Self {
        if self.qp_type == QueuePairType::RC
            || self.qp_type == QueuePairType::UC
        {
            self.path_mtu = Some(path_mtu);
        }
        self
    }

    /// Set the PSN for the receive queue.
    ///
    /// Defaults to 0.
    /// Valid only for RC and UC QPs.
    pub fn set_rq_psn(&mut self, rq_psn: u32) -> &mut Self {
        if self.qp_type == QueuePairType::RC
            || self.qp_type == QueuePairType::UC
        {
            self.rq_psn = Some(rq_psn);
        }
        self
    }

    /// Set the opaque context value for the new `QueuePair`.
    ///
    /// Defaults to 0.
    pub fn set_context(&mut self, ctx: isize) -> &mut Self {
        self.ctx = ctx;
        self
    }

    /// Create a new `QueuePair` from this builder template.
    ///
    /// The returned `QueuePair` is associated with the builder's `ProtectionDomain`.
    ///
    /// This method will fail if asked to create QP of a type other than `IBV_QPT_RC` or
    /// `IBV_QPT_UD` associated with an SRQ.
    ///
    /// # Errors
    ///
    ///  - `EINVAL`: Invalid `ProtectionDomain`, sending or receiving `Context`, or invalid value
    ///    provided in `max_send_wr`, `max_recv_wr`, or in `max_inline_data`.
    ///  - `ENOMEM`: Not enough resources to complete this operation.
    ///  - `ENOSYS`: QP with this Transport Service Type isn't supported by this RDMA device.
    ///  - `EPERM`: Not enough permissions to create a QP with this Transport Service Type.
    pub fn build(&self) -> io::Result<PreparedQueuePair<'res>> {

        let attr = QpInitAttr {
            qp_context: 0,
            send_cq: self.send.inner.as_ref(),
            recv_cq: self.recv.inner.as_ref(),
            srq: None,
            cap: self.cap,
            qp_type: self.qp_type,
            sq_sig_all: 0,
        };

        let inner = self.pd.ctx.inner.clone().create_qp(self.pd.pd, &attr)?;

        Ok(PreparedQueuePair {
            ctx: self.pd.ctx,
            qp: crate::QueuePair {
                inner,
            },
            access: self.access,
            timeout: self.timeout,
            retry_count: self.retry_count,
            rnr_retry: self.rnr_retry,
            min_rnr_timer: self.min_rnr_timer,
            max_rd_atomic: self.max_rd_atomic,
            max_dest_rd_atomic: self.max_dest_rd_atomic,
            path_mtu: self.path_mtu,
            rq_psn: self.rq_psn,
        })
    }
}

/// An allocated but uninitialized `QueuePair`.
///
/// Specifically, this `QueuePair` has been allocated with `ibv_create_qp`, but has not yet been
/// initialized with calls to `ibv_modify_qp`.
///
/// To complete the construction of the `QueuePair`, you will need to obtain the
/// `QueuePairEndpoint` of the remote end (by using `PreparedQueuePair::endpoint`), and then call
/// `PreparedQueuePair::handshake` on both sides with the other side's `QueuePairEndpoint`:
///
/// ```rust,ignore
/// // on host 1
/// let pqp: PreparedQueuePair = ...;
/// let host1end = pqp.endpoint();
/// host2.send(host1end);
/// let host2end = host2.recv();
/// let qp = pqp.handshake(host2end);
///
/// // on host 2
/// let pqp: PreparedQueuePair = ...;
/// let host2end = pqp.endpoint();
/// host1.send(host2end);
/// let host1end = host1.recv();
/// let qp = pqp.handshake(host1end);
/// ```
pub struct PreparedQueuePair<'res> {
    ctx: &'res Context,
    qp: crate::QueuePair,

    // carried from builder
    /// only valid for RC and UC
    access: Option<ffi::AccessFlags>,
    /// only valid for RC
    min_rnr_timer: Option<u8>,
    /// only valid for RC
    timeout: Option<u8>,
    /// only valid for RC
    retry_count: Option<u8>,
    /// only valid for RC
    rnr_retry: Option<u8>,
    /// only valid for RC
    max_rd_atomic: Option<u8>,
    /// only valid for RC
    max_dest_rd_atomic: Option<u8>,
    /// only valid for RC and UC
    path_mtu: Option<Mtu>,
    /// only valid for RC and UC
    rq_psn: Option<u32>,
}

/// An identifier for the network endpoint of a `QueuePair`.
///
/// Internally, this contains the `QueuePair`'s `qp_num`, as well as the context's `lid` and `gid`.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serialize", derive(Encode, Decode))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct QueuePairEndpoint {
    /// the `QueuePair`'s `qp_num`
    pub num: u32,
    /// the context's `lid`
    pub lid: u16,
    /// the context's `gid`, used for global routing
    pub gid: Option<Gid>,
}

impl<'res> PreparedQueuePair<'res> {
    /// Get the network endpoint for this `QueuePair`.
    ///
    /// This endpoint will need to be communicated to the `QueuePair` on the remote end.
    pub fn endpoint(&self) -> QueuePairEndpoint {
        let num = self.qp.inner.number();

        // A peer that receives a GID here enables global routing and puts a GRH on every packet
        // it sends us. `ibv_query_gid` is still a stub returning an all-zero GID, and the mlx4
        // driver hardcodes `primary_grh = false` in the queue pair context, so advertising one
        // would ask the peer to address us by a GID we neither know nor honour — its packets
        // would go undelivered and it would fail with IBV_WC_RETRY_EXC_ERR. Advertise a GID only
        // once we actually have one; until then the connection is LID-routed, which is what the
        // driver programs anyway.
        let gid = (self.ctx.gid.raw != [0u8; 16]).then_some(self.ctx.gid);

        QueuePairEndpoint {
            num,
            lid: self.ctx.port_attr.lid,
            gid,
        }
    }

    /// Set up the `QueuePair` such that it is ready to exchange packets with a remote `QueuePair`.
    ///
    /// Internally, this uses `ibv_modify_qp` to mark the `QueuePair` as initialized
    /// (`IBV_QPS_INIT`), ready to receive (`IBV_QPS_RTR`), and ready to send (`IBV_QPS_RTS`).
    /// Further discussion of the protocol can be found on [RDMAmojo].
    ///
    /// If the endpoint contains a Gid, the routing will be global. This means:
    /// ```text,ignore
    /// ah_attr.is_global = 1;
    /// ah_attr.grh.hop_limit = 0xff;
    /// ```
    ///
    /// The handshake also sets the following parameters, which are currently not configurable:
    ///
    /// # Examples
    ///
    /// ```text,ignore
    /// port_num = PORT_NUM;
    /// pkey_index = 0;
    /// sq_psn = 0;
    ///
    /// ah_attr.sl = 0;
    /// ah_attr.src_path_bits = 0;
    /// ```
    ///
    /// # Errors
    ///
    ///  - `EINVAL`: Invalid value provided in `attr` or in `attr_mask`.
    ///  - `ENOMEM`: Not enough resources to complete this operation.
    ///
    /// [RDMAmojo]: http://www.rdmamojo.com/2014/01/18/connecting-queue-pairs/
    pub fn handshake(mut self, remote: QueuePairEndpoint) -> io::Result<crate::QueuePair> {
        // init and associate with port
        let mut attr = ffi::QueuePairAttr {
            qp_state: ffi::QueuePairtState::Init,
            pkey_index: 0,
            port_num: PORT_NUM,
            ..Default::default()
        };
        let mut mask = ffi::QueuePairAttrMask::IBV_QP_STATE
            | ffi::QueuePairAttrMask::IBV_QP_PKEY_INDEX
            | ffi::QueuePairAttrMask::IBV_QP_PORT;
        if let Some(access) = self.access {
            attr.qp_access_flags = access;
            mask |= ffi::QueuePairAttrMask::IBV_QP_ACCESS_FLAGS;
        }
        self.qp.inner.modify(&attr, mask)?;

        // set ready to receive
        let mut attr = ffi::QueuePairAttr {
            qp_state: ffi::QueuePairtState::ReadyToReceive,
            // TODO: this is only valid for RC and UC
            dest_qp_num: remote.num,
            // TODO: this is only valid for RC and UC
            ah_attr: ffi::AddressHandleAttr {
                dlid: remote.lid,
                sl: 0,
                src_path_bits: 0,
                port_num: PORT_NUM,
                grh: Default::default(),
                ..Default::default()
            },
            ..Default::default()
        };
        if let Some(gid) = remote.gid {
            attr.ah_attr.is_global = 1;
            attr.ah_attr.grh.dgid = gid.into();
            attr.ah_attr.grh.hop_limit = 0xff;
        }
        let mut mask = ffi::QueuePairAttrMask::IBV_QP_STATE
            | ffi::QueuePairAttrMask::IBV_QP_AV
            | ffi::QueuePairAttrMask::IBV_QP_DEST_QPN;
        if let Some(max_dest_rd_atomic) = self.max_dest_rd_atomic {
            attr.max_dest_rd_atomic = max_dest_rd_atomic;
            mask |= ffi::QueuePairAttrMask::IBV_QP_MAX_DEST_RD_ATOMIC;
        }
        if let Some(min_rnr_timer) = self.min_rnr_timer {
            attr.min_rnr_timer = min_rnr_timer;
            mask |= ffi::QueuePairAttrMask::IBV_QP_MIN_RNR_TIMER;
        }
        if let Some(path_mtu) = self.path_mtu {
            attr.path_mtu = path_mtu;
            mask |= ffi::QueuePairAttrMask::IBV_QP_PATH_MTU;
        }
        if let Some(rq_psn) = self.rq_psn {
            attr.rq_psn = rq_psn;
            mask |= ffi::QueuePairAttrMask::IBV_QP_RQ_PSN;
        }
        self.qp.inner.modify(&attr, mask)?;

        // set ready to send
        let mut attr = ffi::QueuePairAttr {
            qp_state: ffi::QueuePairtState::ReadyToSend,
            sq_psn: 0,
            ..Default::default()
        };
        let mut mask = ffi::QueuePairAttrMask::IBV_QP_STATE | ffi::QueuePairAttrMask::IBV_QP_SQ_PSN;
        if let Some(timeout) = self.timeout {
            attr.timeout = timeout;
            mask |= ffi::QueuePairAttrMask::IBV_QP_TIMEOUT;
        }
        if let Some(retry_count) = self.retry_count {
            attr.retry_cnt = retry_count;
            mask |= ffi::QueuePairAttrMask::IBV_QP_RETRY_CNT;
        }
        if let Some(rnr_retry) = self.rnr_retry {
            attr.rnr_retry = rnr_retry;
            mask |= ffi::QueuePairAttrMask::IBV_QP_RNR_RETRY;
        }
        if let Some(max_rd_atomic) = self.max_rd_atomic {
            attr.max_rd_atomic = max_rd_atomic;
            mask |= ffi::QueuePairAttrMask::IBV_QP_MAX_QP_RD_ATOMIC;
        }
        self.qp.inner.modify(&attr, mask)?;

        Ok(self.qp)
    }
}

#[derive(PartialEq, Debug, Copy, Clone, Encode, Decode)]
pub enum WorkRequestOpcode {
    RdmaWrite,
    Send,
    RdmaRead,
}

/// A fully initialized and ready `QueuePair`.
///
/// A queue pair is the actual object that sends and receives data in the RDMA architecture
/// (something like a socket). It's not exactly like a socket, however. A socket is an abstraction,
/// which is maintained by the network stack and doesn't have a physical resource behind it. A QP
/// is a resource of an RDMA device and a QP number can be used by one process at the same time
/// (similar to a socket that is associated with a specific TCP or UDP port number)
pub struct QueuePair {
    inner: Arc<dyn IbvQueuePair>,
}

unsafe impl Send for QueuePair {}
unsafe impl Sync for QueuePair {}

impl QueuePair {
    /// Posts a linked list of Work Requests (WRs) to the Send Queue of this Queue Pair.
    ///
    /// Generates a HW-specific Send Request for the memory at `mr[range]`, and adds it to the tail
    /// of the Queue Pair's Send Queue without performing any context switch. The RDMA device will
    /// handle it (later) in asynchronous way. If there is a failure in one of the WRs because the
    /// Send Queue is full or one of the attributes in the WR is bad, it stops immediately and
    /// return the pointer to that WR.
    ///
    /// `wr_id` is a 64 bits value associated with this WR. If a Work Completion will be generated
    /// when this Work Request ends, it will contain this value.
    ///
    /// Internally, the memory at `mr[range]` will be sent as a single `ibv_send_wr` using
    /// `IBV_WR_SEND`. The send has `IBV_SEND_SIGNALED` set, so a work completion will also be
    /// triggered as a result of this send.
    ///
    /// See also [RDMAmojo's `ibv_post_send` documentation][1].
    ///
    /// # Safety
    ///
    /// The memory region can only be safely reused or dropped after the request is fully executed
    /// and a work completion has been retrieved from the corresponding completion queue (i.e.,
    /// until `CompletionQueue::poll` returns a completion for this send).
    ///
    /// # Errors
    ///
    ///  - `EINVAL`: Invalid value provided in the Work Request.
    ///  - `ENOMEM`: Send Queue is full or not enough resources to complete this operation.
    ///  - `EFAULT`: Invalid value provided in `QueuePair`.
    ///
    /// [1]: http://www.rdmamojo.com/2013/01/26/ibv_post_send/
    #[inline]
    pub unsafe fn post_send<'pd, T, R>(
        &mut self,
        mr: &mut LocalMemoryRegion<'pd, T>,
        ranges: Vec<Vec<R>>,
        wr_ids: Vec<u64>,
        send_flags: Vec<ffi::SendFlags>
    ) -> io::Result<()>
    where
        R: sliceindex::SliceIndex<[T], Output = [T]>,
    {
        assert!(
            ranges.len() == wr_ids.len() && send_flags.len() == wr_ids.len(),
            "local ranges, wr ids and send flags must have the same size!");

        let mut wrs = Vec::new();

        // Pair each wr_id with the range/flags posted for it, in the order they were given —
        // not in reverse, which is what popping from the back of `ranges`/`send_flags` while
        // iterating `wr_ids` forward would do.
        for ((wr_id, range), wr_send_flags) in wr_ids.into_iter().zip(ranges).zip(send_flags) {
            let mut sg_list = Vec::new();

            for slice in range {
                let l = slice.index(mr);
                let sge = ScatterGatherEntry {
                    addr: l.as_ptr() as u64,
                    length: mem::size_of_val(l) as u32,
                    lkey: mr.metadata.lkey,
                };
                sg_list.push(sge);
            }

            wrs.push(SendWorkRequest {
                wr_id,
                sges: sg_list,
                opcode: WorkRequestOpcode::Send,
                send_flags: wr_send_flags,
                wr: Default::default(),
            });
        }

        // TODO:
        //
        // ibv_post_send()  posts the linked list of work requests (WRs) starting with wr to the
        // send queue of the queue pair qp.  It stops processing WRs from this list at the first
        // failure (that can  be  detected  immediately  while  requests  are  being posted), and
        // returns this failing WR through bad_wr.
        //
        // The user should not alter or destroy AHs associated with WRs until request is fully
        // executed and  a  work  completion  has been retrieved from the corresponding completion
        // queue (CQ) to avoid unexpected behavior.
        //
        // ... However, if the IBV_SEND_INLINE flag was set, the  buffer  can  be reused
        // immediately after the call returns.

        let _bad_wr = unsafe { self.inner.post_send(wrs.as_mut_slice())? };

        Ok(())
    }

    /// Posts a linked list of Work Requests (WRs) to the Receive Queue of this Queue Pair.
    ///
    /// Generates a HW-specific Receive Request out of it and add it to the tail of the Queue
    /// Pair's Receive Queue without performing any context switch. The RDMA device will take one
    /// of those Work Requests as soon as an incoming opcode to that QP will consume a Receive
    /// Request (RR). If there is a failure in one of the WRs because the Receive Queue is full or
    /// one of the attributes in the WR is bad, it stops immediately and return the pointer to that
    /// WR.
    ///
    /// `wr_id` is a 64 bits value associated with this WR. When a Work Completion is generated
    /// when this Work Request ends, it will contain this value.
    ///
    /// Internally, the memory at `mr[range]` will be received into as a single `ibv_recv_wr`.
    ///
    /// See also [RDMAmojo's `ibv_post_recv` documentation][1].
    ///
    /// # Safety
    ///
    /// The memory region can only be safely reused or dropped after the request is fully executed
    /// and a work completion has been retrieved from the corresponding completion queue (i.e.,
    /// until `CompletionQueue::poll` returns a completion for this receive).
    ///
    /// # Errors
    ///
    ///  - `EINVAL`: Invalid value provided in the Work Request.
    ///  - `ENOMEM`: Receive Queue is full or not enough resources to complete this operation.
    ///  - `EFAULT`: Invalid value provided in `QueuePair`.
    ///
    /// [1]: http://www.rdmamojo.com/2013/02/02/ibv_post_recv/
    #[inline]
    pub unsafe fn post_receive<'pd, T, R>(
        &mut self,
        mr: &mut LocalMemoryRegion<'pd, T>,
        ranges: Vec<Vec<R>>,
        wr_ids: Vec<u64>,
    ) -> io::Result<()>
    where
        R: sliceindex::SliceIndex<[T], Output = [T]>,
    {
        assert!(
            ranges.len() == wr_ids.len(),
            "local ranges, and wr ids must have the same size!");

        let mut wrs = Vec::new();

        // Pair each wr_id with the range posted for it, in the order they were given — not in
        // reverse, which is what popping from the back of `ranges` while iterating `wr_ids`
        // forward would do.
        for (wr_id, range) in wr_ids.into_iter().zip(ranges) {
            let mut sg_list = Vec::new();

            for slice in range {
                let l = slice.index(mr);
                let sge = ScatterGatherEntry {
                    addr: l.as_ptr() as u64,
                    length: mem::size_of_val(l) as u32,
                    lkey: mr.metadata.lkey,
                };
                sg_list.push(sge);
            }

            wrs.push(ReceiveWorkRequest {
                wr_id,
                sges: sg_list,
            });
        }


        // TODO:
        //
        // If the QP qp is associated with a shared receive queue, you must use the function
        // ibv_post_srq_recv(), and not ibv_post_recv(), since the QP's own receive queue will not
        // be used.
        //
        // If a WR is being posted to a UD QP, the Global Routing Header (GRH) of the incoming
        // message will be placed in the first 40 bytes of the buffer(s) in the scatter list. If no
        // GRH is present in the incoming message, then the first  bytes  will  be undefined. This
        // means that in all cases, the actual data of the incoming message will start at an offset
        // of 40 bytes into the buffer(s) in the scatter list.

        let _bad_wr = self.inner.post_receive(wrs.as_mut_slice())?;
        Ok(())
    }

    /// Posts a RDMA Write Work Request (WR) to the Send Queue of this Queue Pair.
    ///
    /// Generates a HW-specific Send Request for the memory at `mr[range]`, and adds it to the tail
    /// of the Queue Pair's Send Queue without performing any context switch. The RDMA device will
    /// handle it (later) in asynchronous way. If there is a failure in one of the WRs because the
    /// Send Queue is full or one of the attributes in the WR is bad, it stops immediately and
    /// return the pointer to that WR.
    ///
    /// `wr_id` is a 64 bits value associated with this WR. If a Work Completion will be generated
    /// when this Work Request ends, it will contain this value.
    ///
    /// Internally, the memory at `mr[range]` will be sent as a single `ibv_send_wr` using
    /// `IBV_WR_RDMA_WRITE`. The send has `IBV_SEND_SIGNALED` set, so a work completion will also
    /// be triggered as a result of this write.
    ///
    /// See also [RDMAmojo's `ibv_post_send` documentation][1].
    ///
    /// # Safety
    ///
    /// The memory region can only be safely reused or dropped after the request is fully executed
    /// and a work completion has been retrieved from the corresponding completion queue (i.e.,
    /// until `CompletionQueue::poll` returns a completion for this send).
    ///
    /// # Errors
    ///
    ///  - `EINVAL`: Invalid value provided in the Work Request.
    ///  - `ENOMEM`: Send Queue is full or not enough resources to complete this operation.
    ///  - `EFAULT`: Invalid value provided in `QueuePair`.
    ///
    /// [1]: http://www.rdmamojo.com/2013/01/26/ibv_post_send/
    #[inline]
    pub unsafe fn rdma_write<'pd, T, R>(
        &mut self,
        local_mr: &mut LocalMemoryRegion<'pd, T>,
        local_ranges: Vec<Vec<R>>,
        remote_mr: &mut RemoteMemoryRegion<T>,
        remote_ranges: Vec<Range<u64>>,
        wr_ids: Vec<u64>,
        send_flags: Vec<ffi::SendFlags>
    ) -> io::Result<()>
    where
        R: sliceindex::SliceIndex<[T], Output = [T]>,
    {
        self.rdma_backbone(
            remote_mr,
            remote_ranges,
            local_mr,
            local_ranges,
            wr_ids,
            WorkRequestOpcode::RdmaWrite,
            send_flags)
    }

    /// Posts a RDMA Read Work Request (WR) to the Send Queue of this Queue Pair.
    ///
    /// Generates a HW-specific Send Request for the memory at `mr[range]`, and adds it to the tail
    /// of the Queue Pair's Send Queue without performing any context switch. The RDMA device will
    /// handle it (later) in asynchronous way. If there is a failure in one of the WRs because the
    /// Send Queue is full or one of the attributes in the WR is bad, it stops immediately and
    /// return the pointer to that WR.
    ///
    /// `wr_id` is a 64 bits value associated with this WR. If a Work Completion will be generated
    /// when this Work Request ends, it will contain this value.
    ///
    /// Internally, the whole memory at `mr[range]` will be transferred as a single `ibv_send_wr`
    /// using `IBV_WR_RDMA_READ`. The send has `IBV_SEND_SIGNALED` set, so a work completion will
    /// also be triggered as a result of this read.
    ///
    /// See also [RDMAmojo's `ibv_post_send` documentation][1].
    ///
    /// # Safety
    ///
    /// The memory region can only be safely reused or dropped after the request is fully executed
    /// and a work completion has been retrieved from the corresponding completion queue (i.e.,
    /// until `CompletionQueue::poll` returns a completion for this send).
    ///
    /// # Errors
    ///
    ///  - `EINVAL`: Invalid value provided in the Work Request.
    ///  - `ENOMEM`: Send Queue is full or not enough resources to complete this operation.
    ///  - `EFAULT`: Invalid value provided in `QueuePair`.
    ///
    /// [1]: http://www.rdmamojo.com/2013/01/26/ibv_post_send/
    #[inline]
    pub unsafe fn rdma_read<'pd, T, R>(
        &mut self,
        remote_mr: &mut RemoteMemoryRegion<T>,
        remote_ranges: Vec<Range<u64>>,
        local_mr: &mut LocalMemoryRegion<'pd, T>,
        local_ranges: Vec<Vec<R>>,
        wr_ids: Vec<u64>,
        send_flags: Vec<ffi::SendFlags>
    ) -> io::Result<()>
    where
        R: sliceindex::SliceIndex<[T], Output = [T]>,
    {
        self.rdma_backbone(
            remote_mr,
            remote_ranges,
            local_mr,
            local_ranges,
            wr_ids,
            WorkRequestOpcode::RdmaRead,
            send_flags)
    }

    #[inline]
    fn rdma_backbone<'pd, T, R>(
        &mut self,
        remote_mr: &mut RemoteMemoryRegion<T>,
        mut remote_ranges: Vec<Range<u64>>,
        local_mr: &mut LocalMemoryRegion<'pd, T>,
        mut local_ranges: Vec<Vec<R>>,
        mut wr_ids: Vec<u64>,
        opcode: WorkRequestOpcode,
        mut send_flags: Vec<ffi::SendFlags>
    ) -> io::Result<()>
    where
        R: sliceindex::SliceIndex<[T], Output = [T]>,
    {
        assert!(
            (remote_ranges.len() == local_ranges.len()) && (local_ranges.len() == wr_ids.len())
                && (wr_ids.len() == send_flags.len()),
            "remote ranges, local ranges, and wr ids must have the same size!");

        let mut wrs = Vec::new();

        for wr_id in wr_ids {
            let mut sg_list = Vec::new();
            let local_range = local_ranges.pop().unwrap();
            let remote_range = remote_ranges.pop().unwrap();
            let wr_send_flags = send_flags.pop().unwrap();

            // check memory bounds before access
            let remote_start = remote_mr.addr + remote_range.start;
            let remote_end = remote_mr.addr + remote_range.end;
            if remote_end < remote_start {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "remote range is invalid",
                ));
            }
            if remote_end > remote_mr.addr + remote_mr.len as u64 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "remote range is invalid",
                ));
            }

            let remote_c = (remote_range.end - remote_range.start) as usize;
            let mut local_c = 0;

            for slice in local_range {
                let l = slice.index(local_mr);
                let sge = ScatterGatherEntry {
                    addr: l.as_ptr() as u64,
                    length: mem::size_of_val(l) as u32,
                    lkey: local_mr.metadata.lkey,
                };
                local_c += l.len();
                sg_list.push(sge);
            }

            if local_c != remote_c {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "local and remote range must have the same size",
                ));
            }

            wrs.push(SendWorkRequest {
                wr_id,
                sges: sg_list,
                opcode,
                send_flags: wr_send_flags,
                wr: ffi::SendWorkRequestData::Rdma {
                    remote_addr: remote_start,
                    rkey: remote_mr.rkey,
                },
            });
        }


        // TODO:
        //
        // ibv_post_send()  posts the linked list of work requests (WRs) starting with wr to the
        // send queue of the queue pair qp.  It stops processing WRs from this list at the first
        // failure (that can  be  detected  immediately  while  requests  are  being posted), and
        // returns this failing WR through bad_wr.
        //
        // The user should not alter or destroy AHs associated with WRs until request is fully
        // executed and  a  work  completion  has been retrieved from the corresponding completion
        // queue (CQ) to avoid unexpected behavior.
        //
        // ... However, if the IBV_SEND_INLINE flag was set, the  buffer  can  be reused
        // immediately after the call returns.

        let _bad_wr = unsafe { self.inner.post_send(wrs.as_mut_slice())? };

        Ok(())
    }
}
