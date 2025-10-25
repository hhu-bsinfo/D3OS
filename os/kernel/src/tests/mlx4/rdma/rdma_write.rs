use super::{session, handshake, integrity};
use crate::build_constants;
use log::{debug};
use crate::infiniband::ibverbs::{
    devices
};
use crate::infiniband::ib_core::ibv_send_flags;
use super::*;
#[cfg(kernel_bench)]
use crate::tests::mlx4::bench;
use alloc::{vec, vec::Vec};
use crate::cpu;
use core::arch::x86_64::{_mm_clflush, _mm_sfence, _mm_mfence};

pub fn invoke() {
    let min_cq_entries = 64;
    let alloc_mem = ALLOC_MEM;
    let mut context_buffer = [0u8; CONTEXT_BUFFER_SIZE];

    let ctx = loop {
        let res_ctx = devices()
            .expect("failed to get device list")
            .iter()
            .next()
            .expect("failed to get device")
            .open();
        match res_ctx {
            Ok(ctx) => break ctx,
            Err(_) => {
                debug!("failed to get device context => most likely due to port not being ready yet ...");
                crate::timer().wait(5000);
            },
        }
    };

    debug!("obtained device context");

    let pd = ctx.alloc_pd().expect("failed to allocate protection domain");

    let mut rdma_session = session::RdmaSession::new(&ctx, &pd, alloc_mem, min_cq_entries);
    let udp_session  = session::UdpSession::new();

    crate::timer().wait(1000); // give some time for the memory regions

    let payload_f = integrity::PAYLOAD_FUNCTIONS.seq;

    if build_constants::IS_SENDER {
        debug!("Starting as SENDER");

        let payload = integrity::build_payload(ALLOC_MEM - META_DATA_SIZE, payload_f);

        let packet_len = integrity::build_packet(&payload[..], &mut context_buffer).expect("failed to create packet");
        let packet = &context_buffer[..packet_len];

        session::RdmaSession::write(&mut rdma_session.mr, packet, 0..alloc_mem);

        unsafe { _mm_mfence() };

        unsafe { cpu().flush_cache(&rdma_session.mr) };

        let max_send_wr = 1024;
        let max_send_sge = 1;

        let allocated_qp = session::RdmaSession::create_qp(
            rdma_session.pd, 
            &rdma_session.cq_send, 
            &rdma_session.cq_recv, 
            false,
            max_send_wr,
            0,
            max_send_sge,
            0
        )
        .set_timeout(10)
        .set_min_rnr_timer(30)
        .build().expect("build of allocated QP was not successful");

        handshake::wait_ready(&udp_session);
        handshake::send_ack(&udp_session);

        let endpoint = allocated_qp.endpoint();
        let local_mr = rdma_session.mr.remote();

        let remote_qp_endpoint = handshake::exchange_endpoints(&udp_session, endpoint);
        debug!("Successfully received remote endpoint : {:?}", remote_qp_endpoint);

        let mut remote_mr = handshake::exchange_memory_region(&udp_session, local_mr);
        debug!("Successfully received remote memory region");
        debug!("Remote memory region\n: {:?}", remote_mr);

        let mut qp = allocated_qp.handshake(remote_qp_endpoint).expect("failed handshake");

        handshake::wait_ack(&udp_session);

        #[cfg(kernel_test)]
        {
            debug!("Performing RDMA write...");
        
            let _result = unsafe { qp.rdma_write(
                &mut rdma_session.mr, 
                vec![vec![0..alloc_mem]], 
                &mut remote_mr, 
                vec![0..(alloc_mem as u64)], 
                vec![1],
                vec![ibv_send_flags::SIGNALED]
            ).expect("ups ... something went wrong!") };
        
            session::RdmaSession::poll_cq::<10>(&rdma_session.cq_send, 1);
        }

        #[cfg(kernel_bench)]
        {
            bench::rdma_bench(
                bench::SPEC_RDMA_TYPE::RDMA_WRITE,
                alloc_mem, 
                &mut qp, 
                &mut rdma_session.mr,
                &mut remote_mr,
                &rdma_session.cq_send,
                None
            );
        }
    
        handshake::send_ack(&udp_session);
    }
    else {
        debug!("Starting as RECEIVER");
        let allocated_qp = session::RdmaSession::create_qp(
            rdma_session.pd, 
            &rdma_session.cq_send, 
            &rdma_session.cq_recv, 
            true,
            0,
            0,
            0,
            0
        )
        .build().expect("build of allocated QP was not successful");

        handshake::send_ready_and_wait_ack(&udp_session, 10, 3000);  
        
        let endpoint = allocated_qp.endpoint();
        let local_mr = rdma_session.mr.remote();

        let remote_qp_endpoint = handshake::exchange_endpoints(&udp_session, endpoint);
        debug!("Successfully received remote endpoint : {:?}", remote_qp_endpoint);

        let _remote_mr = handshake::exchange_memory_region(&udp_session, local_mr);

        let _qp = allocated_qp.handshake(remote_qp_endpoint).expect("failed handshake");

        handshake::send_ack(&udp_session);

        debug!("Receiver finished sending data");

        handshake::wait_ack(&udp_session);

        #[cfg(kernel_test)]
        {
            debug!("Checking data integrity...");

            unsafe { cpu().flush_cache(&rdma_session.mr) };

            unsafe { _mm_mfence() }; 

            let packet = session::RdmaSession::read(&rdma_session.mr, 0..alloc_mem);
            
            let _ = integrity::validate_packet(packet)
                .map_err(move |e| {
                    unsafe { hit_wo_fault(packet, &mut context_buffer, payload_f) }
                    println!("Data integrity failed due to {:?}", e);
                    e
                });
        }
        
        #[cfg(kernel_bench)]
        {
            let payload = integrity::build_payload(ALLOC_MEM - META_DATA_SIZE, payload_f);

            let packet_len = integrity::build_packet(&payload[..], &mut context_buffer).expect("failed to create packet");
            let packet = &context_buffer[..packet_len];

            let total_correct_bytes = bench::get_correct_bytes_per_batch(
                &mut rdma_session.mr,
                alloc_mem,
                packet
            );

            let hit_rate = ((total_correct_bytes as f64) / (alloc_mem as f64)) * 100.0;

            println!("last rdma write hit rate: {:.2}%", hit_rate);
        }

        debug!("end - rdma write")
        //loop {}
    }
}