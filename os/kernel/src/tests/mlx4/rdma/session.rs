use crate::infiniband::ibverbs::*;
use crate::infiniband::ibverbs::sliceindex::SliceIndex;
use crate::build_constants;
use crate::network::{self, SocketType};
use smoltcp::iface::SocketHandle;
use smoltcp::wire::Ipv4Address;
use smoltcp::socket::udp::{self, RecvError};
use log::debug;
use crate::infiniband::ib_core::{ibv_qp_type::Type, ibv_wc, ibv_qp_cap};
use core::ops;
use core::slice::from_raw_parts_mut;

pub struct RdmaSession<'ctx, 'pd> {
    pub ctx: &'ctx Context,
    pub pd: &'pd ProtectionDomain<'ctx>,
    pub mr: LocalMemoryRegion<'pd, u8>,
    pub cq_send: CompletionQueue<'ctx>,
    pub cq_recv: CompletionQueue<'ctx>,
}

pub struct UdpSession {
    pub src_port: u16,
    pub tgt_port: u16,
    pub socket: SocketHandle,
    pub ip: Ipv4Address,
}

impl<'ctx, 'pd> RdmaSession<'ctx, 'pd> {
    pub fn new(ctx: &'ctx Context, pd: &'pd ProtectionDomain<'ctx>, alloc_mem: usize, min_cq_entries: i32) -> Self
    {
        let port_stats = ctx.query_port();

        println!("State: {:?}, Max MTU: {:?}, Active MTU: {:?}", 
            port_stats.state, port_stats.max_mtu, port_stats.active_mtu);

        let mr = pd.allocate::<u8>(alloc_mem).expect("failed to pin memory");

        let cq_send = ctx.create_cq(min_cq_entries, 1).expect("failed to create send CQ");
        let cq_recv = ctx.create_cq(min_cq_entries, 2).expect("failed to create recv CQ");

        Self { ctx, pd, mr, cq_send, cq_recv }
    }
    
    pub fn create_qp(
        pd: &'pd ProtectionDomain<'ctx>,
        cq_send: &'ctx CompletionQueue<'ctx>,
        cq_recv: &'ctx CompletionQueue<'ctx>,
        allow_remote_rw: bool,
        max_send_wr: u32,
        max_recv_wr: u32,
        max_send_sge: u32,
        max_recv_sge: u32
    ) -> QueuePairBuilder<'ctx> where 'pd: 'ctx {
        let cap = ibv_qp_cap { 
            max_send_wr, 
            max_recv_wr, 
            max_send_sge, 
            max_recv_sge, 
            max_inline_data: 0 
        };
        let mut builder = pd.create_qp(cq_send, cq_recv, Type::IBV_QPT_RC, cap);
        if allow_remote_rw {
            builder.allow_remote_rw();
        }
        builder
    }

    /*pub fn create_qp(&'pd self, allow_remote_rw: bool) -> QueuePairBuilder<'ctx> where 'pd: 'ctx {
        let mut builder = self.pd.create_qp(&self.cq_send, &self.cq_recv, Type::IBV_QPT_RC);
        if allow_remote_rw {
            builder.allow_remote_rw();
        }

        builder
    } */

    pub fn poll_cq<const N: usize>(cq_send: &'ctx CompletionQueue<'ctx>, wait_until: usize) {
        let mut wc = [ibv_wc::default(); N];
        let mut completed = 0;

        while completed < wait_until {
            let completions = cq_send.poll(&mut wc).expect("failed to poll for completions");

            #[cfg(kernel_test)]
            for wr in completions.iter() {
                println!("Work request ID: {}", wr.wr_id());
                if !wr.is_valid() {
                    println!("Error occurred: {:#?}", wr.error().unwrap());
                }
                println!("Opcode: {:#?}, Bytes transferred: {}", wr.opcode(), wr.len());
            }

            completed += completions.len();
        }
    }

    pub fn read<I: SliceIndex<[u8], Output = [u8]>>(local_mr: &'pd LocalMemoryRegion<'pd, u8>, 
        local_range: I) -> &'pd [u8] {
        local_range.index(local_mr)
    }

    pub fn write(local_mr: *mut LocalMemoryRegion<'pd, u8>, packet: &[u8], 
        local_range: ops::Range<usize>) {
        let data_range = unsafe { from_raw_parts_mut((*local_mr).as_mut_ptr().add(local_range.start), 
            local_range.end - local_range.start) };
        data_range.copy_from_slice(packet);
    }
}

impl UdpSession {
    pub fn new() -> Self {
        let tgt_port = build_constants::TARGET_PORT.parse::<u16>().unwrap();
        let src_port = 1324;
        let socket = network::open_socket(SocketType::Udp);
        network::bind_udp(socket, src_port).expect("failed binding");

        let ip = build_constants::TARGET_IP.parse::<Ipv4Address>().unwrap();

        debug!("Target: {} ({})", build_constants::TARGET_HOST, ip);
        debug!("Local: {} ({})", build_constants::THIS_HOST, 
            build_constants::THIS_IP.parse::<Ipv4Address>().unwrap());

        Self { src_port, tgt_port, socket, ip }
    }

    pub fn send(&self, buffer: &[u8]) -> Result<(), udp::SendError> {
        network::send_datagram(self.socket, self.ip, self.tgt_port, buffer)
    }

    pub fn recv(&self, buffer: &mut [u8]) -> Result<(usize, udp::UdpMetadata), RecvError> {
        network::recv_datagram(self.socket, buffer)
    }
}
