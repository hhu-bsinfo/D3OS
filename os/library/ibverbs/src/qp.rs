use crate::cq::CompletionQueue;
use crate::provider::{IbvQueuePair, QpInitAttr};
use alloc::sync::Arc;
use core3::io;
use rdma::{AccessFlags, AddressHandleAttr, Gid, Mtu, QueuePairAttr, QueuePairAttrMask, QueuePairCapabilities, QueuePairType, QueuePairtState, ScatterGatherEntry, SendFlags, SendWorkRequestAddressHandle};

use crate::context::Context;
use crate::pd::ProtectionDomain;
use crate::{RemoteMemorySlice, PORT_NUM};
#[cfg(feature = "serialize")]
use bincode::{Decode, Encode};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use spin::RwLock;

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

    qp_type: QueuePairType,

    max_send_wr: u32,
    max_recv_wr: u32,
    max_send_sge: u32,
    max_recv_sge: u32,
    max_inline_data: u32,

    // carried along to handshake phase
    /// only valid for RC and UC
    access: Option<AccessFlags>,
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
            recv,

            qp_type,

            max_send_wr: 1,
            max_recv_wr: 1,
            max_send_sge: 1,
            max_recv_sge: 1,
            max_inline_data: 0,

            access: (qp_type == QueuePairType::RC
                || qp_type == QueuePairType::UC)
                .then_some(AccessFlags::LOCAL_WRITE),
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

    /// Sets the maximum number of outstanding Work Requests that can be posted to the Send Queue
    /// in the new `QueuePair`. Value must be in `[0..dev_cap.max_qp_wr]`. There may be RDMA
    /// devices that for specific transport types may support less outstanding Work Requests than
    /// the maximum reported value.
    ///
    /// Defaults to 1.
    pub fn set_max_send_wr(&mut self, max_send_wr: u32) -> &mut Self {
        self.max_send_wr = max_send_wr;
        self
    }

    /// Sets the maximum number of outstanding Work Requests that can be posted to the Receive
    /// Queue in the new `QueuePair`. Value must be in `[0..dev_cap.max_qp_wr]`. There may be RDMA
    /// devices that for specific transport types may support less outstanding Work Requests than
    /// the maximum reported value. This value is ignored if the Queue Pair is associated with an
    /// SRQ.
    ///
    /// Defaults to 1.
    pub fn set_max_recv_wr(&mut self, max_recv_wr: u32) -> &mut Self {
        self.max_recv_wr = max_recv_wr;
        self
    }

    /// Sets the maximum number of scatter/gather elements in any Work Request that can be posted
    /// to the Send Queue in the new `QueuePair`. Value must be in `[0..dev_cap.max_sge]`.
    ///
    /// Defaults to 1.
    pub fn set_max_send_sge(&mut self, max_send_sge: u32) -> &mut Self {
        self.max_send_sge = max_send_sge;
        self
    }

    /// Sets the maximum number of scatter/gather elements in any Work Request that can be posted
    /// to the Receive Queue in the new `QueuePair`. Value must be in `[0..dev_cap.max_sge]`.
    ///
    /// Defaults to 1.
    pub fn set_max_recv_sge(&mut self, max_recv_sge: u32) -> &mut Self {
        self.max_recv_sge = max_recv_sge;
        self
    }

    /// Sets the maximum message size (in bytes) that can be posted inline to the Send Queue of
    /// the new `QueuePair`. 0 if no inlining is requested.
    ///
    /// Defaults to 0.
    pub fn set_max_inline_data(&mut self, max_inline_data: u32) -> &mut Self {
        self.max_inline_data = max_inline_data;
        self
    }

    /// Set the access flags for the new `QueuePair`.
    ///
    /// Valid only for RC and UC QPs.
    ///
    /// Defaults to `IBV_ACCESS_LOCAL_WRITE`.
    pub fn set_access(&mut self, access: AccessFlags) -> &mut Self {
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
                    | AccessFlags::REMOTE_WRITE
                    | AccessFlags::REMOTE_READ,
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
            cap: QueuePairCapabilities {
                max_send_wr: self.max_send_wr,
                max_recv_wr: self.max_recv_wr,
                max_send_sge: self.max_send_sge,
                max_recv_sge: self.max_recv_sge,
                max_inline_data: self.max_inline_data,
            },
            qp_type: self.qp_type,
            sq_sig_all: 0,
        };

        let inner = self.pd.ctx.inner.clone().create_qp(self.pd.pd, &attr)?;

        Ok(PreparedQueuePair {
            ctx: self.pd.ctx,
            qp: QueuePair { inner },
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
    access: Option<AccessFlags>,
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
        QueuePairEndpoint {
            num: self.qp.number(),
            lid: self.ctx.port_attr.lid,
            gid: (self.ctx.gid.raw != [0u8; 16]).then_some(self.ctx.gid),
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
    pub fn handshake(mut self, remote: QueuePairEndpoint) -> io::Result<QueuePair> {
        // init and associate with port
        let mut attr = QueuePairAttr {
            qp_state: QueuePairtState::Init,
            pkey_index: 0,
            port_num: PORT_NUM,
            ..Default::default()
        };
        let mut mask = QueuePairAttrMask::IBV_QP_STATE
            | QueuePairAttrMask::IBV_QP_PKEY_INDEX
            | QueuePairAttrMask::IBV_QP_PORT;
        if let Some(access) = self.access {
            attr.qp_access_flags = access;
            mask |= QueuePairAttrMask::IBV_QP_ACCESS_FLAGS;
        }
        self.qp.modify(&attr, mask)?;

        // set ready to receive
        let mut attr = QueuePairAttr {
            qp_state: QueuePairtState::ReadyToReceive,
            // TODO: this is only valid for RC and UC
            dest_qp_num: remote.num,
            // TODO: this is only valid for RC and UC
            ah_attr: AddressHandleAttr {
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
        let mut mask = QueuePairAttrMask::IBV_QP_STATE
            | QueuePairAttrMask::IBV_QP_AV
            | QueuePairAttrMask::IBV_QP_DEST_QPN;
        if let Some(max_dest_rd_atomic) = self.max_dest_rd_atomic {
            attr.max_dest_rd_atomic = max_dest_rd_atomic;
            mask |= QueuePairAttrMask::IBV_QP_MAX_DEST_RD_ATOMIC;
        }
        if let Some(min_rnr_timer) = self.min_rnr_timer {
            attr.min_rnr_timer = min_rnr_timer;
            mask |= QueuePairAttrMask::IBV_QP_MIN_RNR_TIMER;
        }
        if let Some(path_mtu) = self.path_mtu {
            attr.path_mtu = path_mtu;
            mask |= QueuePairAttrMask::IBV_QP_PATH_MTU;
        }
        if let Some(rq_psn) = self.rq_psn {
            attr.rq_psn = rq_psn;
            mask |= QueuePairAttrMask::IBV_QP_RQ_PSN;
        }
        self.qp.modify(&attr, mask)?;

        // set ready to send
        let mut attr = QueuePairAttr {
            qp_state: QueuePairtState::ReadyToSend,
            sq_psn: 0,
            ..Default::default()
        };
        let mut mask = QueuePairAttrMask::IBV_QP_STATE | QueuePairAttrMask::IBV_QP_SQ_PSN;
        if let Some(timeout) = self.timeout {
            attr.timeout = timeout;
            mask |= QueuePairAttrMask::IBV_QP_TIMEOUT;
        }
        if let Some(retry_count) = self.retry_count {
            attr.retry_cnt = retry_count;
            mask |= QueuePairAttrMask::IBV_QP_RETRY_CNT;
        }
        if let Some(rnr_retry) = self.rnr_retry {
            attr.rnr_retry = rnr_retry;
            mask |= QueuePairAttrMask::IBV_QP_RNR_RETRY;
        }
        if let Some(max_rd_atomic) = self.max_rd_atomic {
            attr.max_rd_atomic = max_rd_atomic;
            mask |= QueuePairAttrMask::IBV_QP_MAX_QP_RD_ATOMIC;
        }
        self.qp.modify(&attr, mask)?;

        Ok(self.qp)
    }
}

/// A fully initialized and ready `QueuePair`.
///
/// A queue pair is the actual object that sends and receives data in the RDMA architecture
/// (something like a socket). It's not exactly like a socket, however. A socket is an abstraction,
/// which is maintained by the network stack and doesn't have a physical resource behind it. A QP
/// is a resource of an RDMA device and a QP number can be used by one process at the same time
/// (similar to a socket that is associated with a specific TCP or UDP port number)
pub struct QueuePair {
    inner: Arc<RwLock<dyn IbvQueuePair>>,
}

impl QueuePair {
    pub fn number(&self) -> u32 {
        self.inner.read().number()
    }

    pub fn modify(&mut self, attr: &QueuePairAttr, attr_mask: QueuePairAttrMask) -> io::Result<()> {
        self.inner.write().modify(attr, attr_mask)
    }

    /// Posts a list of Work Requests (WRs) to the Send Queue of this Queue Pair.
    ///
    /// Generates a HW-specific Send Request for the memory at `mr[range]`, and adds it to the tail
    /// of the Queue Pair's Send Queue without performing any context switch. The RDMA device will
    /// handle it (later) in asynchronous way. If there is a failure in one of the WRs because the
    /// Send Queue is full or one of the attributes in the WR is bad, it stops immediately and
    /// return the pointer to that WR.
    ///
    /// See also [RDMAmojo's `ibv_post_send` documentation][1].
    ///
    /// # Safety
    ///
    /// The memory region can only be safely reused or dropped after the request is fully executed
    /// and a work completion has been retrieved from the corresponding completion queue (i.e.,
    /// until `CompletionQueue::poll` returns a completion for this send). Except if memory was
    /// send inline, then the data was copied in the WR and the buffer can be reused.
    ///
    /// # Errors
    ///
    ///  - `EINVAL`: Invalid value provided in the Work Request.
    ///  - `ENOMEM`: Send Queue is full or not enough resources to complete this operation.
    ///  - `EFAULT`: Invalid value provided in `QueuePair`.
    ///
    /// [1]: http://www.rdmamojo.com/2013/01/26/ibv_post_send/
    #[inline]
    pub unsafe fn post_send(&mut self, wrs: &[&SendWorkRequest]) -> io::Result<()> {
        unsafe { self.inner.write().post_send(wrs) }
    }

    /// Posts a list of Work Requests (WRs) to the Receive Queue of this Queue Pair.
    ///
    /// Generates a HW-specific Receive Request out of it and add it to the tail of the Queue
    /// Pair's Receive Queue without performing any context switch. The RDMA device will take one
    /// of those Work Requests as soon as an incoming opcode to that QP will consume a Receive
    /// Request (RR). If there is a failure in one of the WRs because the Receive Queue is full or
    /// one of the attributes in the WR is bad, it stops immediately and return the pointer to that
    /// WR.
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
    pub unsafe fn post_receive(&mut self, wrs: &[&ReceiveWorkRequest]) -> io::Result<()> {

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

        unsafe { self.inner.write().post_receive(wrs) }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct SendWorkRequest<'a> {
    pub wr_id: u64,
    pub payload: Payload<'a>,
    pub send_flags: SendFlags,
    pub op: SendOperation,
}

impl<'a> SendWorkRequest<'a> {
    #[inline]
    pub fn send(wr_id: u64, payload: impl Into<Payload<'a>>, send_flags: SendFlags) -> Self {
        Self {
            wr_id,
            payload: payload.into(),
            send_flags,
            op: SendOperation::Send,
        }
    }

    #[inline]
    pub fn rdma_write(wr_id: u64, payload: impl Into<Payload<'a>>, remote_memory_slice: RemoteMemorySlice, send_flags: SendFlags) -> Option<Self> {
        let payload = payload.into();
        let mut local_total_byte_len: usize = 0;

        match payload {
            Payload::Sges(sges) => {
                for sge in sges {
                    local_total_byte_len += sge.length as usize;
                }
            }
            Payload::Inline(bytes) => local_total_byte_len = bytes.len()
        }

        if local_total_byte_len != remote_memory_slice.len {
            None
        } else {
            Some(Self {
                wr_id,
                payload,
                send_flags,
                op: SendOperation::RdmaWrite(RemoteMemoryHeader {
                    remote_addr: remote_memory_slice.addr,
                    rkey: remote_memory_slice.rkey,
                }),
            })
        }
    }

    #[inline]
    pub fn rdma_read(wr_id: u64, sges: &'a [ScatterGatherEntry], remote_memory_slice: RemoteMemorySlice, send_flags: SendFlags) -> Option<Self> {
        let mut local_total_byte_len: usize = 0;

        for sge in sges {
            local_total_byte_len += sge.length as usize;
        }

        if local_total_byte_len != remote_memory_slice.len {
            None
        } else {
            Some(Self {
                wr_id,
                payload: Payload::Sges(sges),
                send_flags,
                op: SendOperation::RdmaRead(RemoteMemoryHeader {
                    remote_addr: remote_memory_slice.addr,
                    rkey: remote_memory_slice.rkey,
                }),
            })
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub enum Payload<'a> {
    Sges(&'a [ScatterGatherEntry]),
    Inline(&'a [u8]),
}

impl<'a> From<&'a [ScatterGatherEntry]> for Payload<'a> {
    fn from(sges: &'a [ScatterGatherEntry]) -> Self {
        Payload::Sges(sges)
    }
}

impl<'a, const N: usize> From<&'a [ScatterGatherEntry; N]> for Payload<'a> {
    fn from(sges: &'a [ScatterGatherEntry; N]) -> Self {
        Payload::Sges(sges)
    }
}


#[derive(Debug, Copy, Clone)]
pub enum SendOperation {
    Send,
    RdmaRead(RemoteMemoryHeader),
    RdmaWrite(RemoteMemoryHeader),
    Atomic {
        remote_mem_header: RemoteMemoryHeader,
        /// Compare operand
        compare_add: u64,
        /// Swap operand
        swap: u64,
    },
    UD(DatagramHeader) ,
}

#[derive(Copy, Clone, Debug)]
pub struct RemoteMemoryHeader {
    /// Start address of remote memory buffer
    pub remote_addr: u64,
    /// Key of the remote Memory Region
    pub rkey: u32,
}

#[derive(Copy, Clone, Debug)]
pub struct DatagramHeader {
    /// Address handle for the remote node address
    pub ah: SendWorkRequestAddressHandle,
    pub remote_qpn: u32,
    pub remote_qkey: u32,
}

#[derive(Copy, Clone, Debug)]
pub struct ReceiveWorkRequest<'a> {
    pub wr_id: u64,
    pub sges: &'a [ScatterGatherEntry],
}

