use super::{session, handshake, integrity};
use crate::build_constants;
use mm::{MmapFlags, mmap};
use rdma_core::{
    devices, LocalMemoryRegion
};
use rdma::ibv_send_flags;
#[cfg(user_bench)]
use super::bench;
use super::*;
use alloc::{vec};
use cpu_core::{flush_cache};
use core::arch::x86_64::{_mm_mfence};
use concurrent::thread::sleep;
use terminal::{println, print};

pub fn invoke() {
    let min_cq_entries = 1000;
    let alloc_mem = ALLOC_MEM;
    let context_buffer = mmap(30 * 1024 * 1024 * 1024 * 1024, CONTEXT_BUFFER_SIZE,
        MmapFlags::ANONYMOUS | MmapFlags::POPULATE | MmapFlags::ALLOC_AT)
        .expect("mmap failed");

    println!("waiting for device context");

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
                println!("failed to get device context => most likely due to port not being ready yet ...");
                sleep(1000);
            },
        }
    };

    println!("obtained device context");

    let pd = ctx.alloc_pd().expect("failed to allocate protection domain");

    let mut rdma_session = session::RdmaSession::new(&ctx, &pd, alloc_mem, min_cq_entries);
    let udp_session  = session::UdpSession::new();

    // sleep(1000); // give some time for the memory regions

    let payload_f = integrity::PAYLOAD_FUNCTIONS.lcg;

    if build_constants::IS_SENDER {
        println!("Starting as SENDER");
        let max_send_wr = 4000;
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
        .build()
        .expect("build of allocated QP was not successful");

        let mr = &mut rdma_session.mr as *mut LocalMemoryRegion<'_, u8>;

        handshake::wait_ready(&udp_session);
        handshake::send_ack(&udp_session);

        let endpoint = allocated_qp.endpoint();
        let local_mr = unsafe { (*mr).remote() };

        let remote_qp_endpoint = handshake::exchange_endpoints(&udp_session, endpoint);
        println!("Successfully received remote endpoint : {:?}", remote_qp_endpoint);

        let mut remote_mr = handshake::exchange_memory_region(&udp_session, local_mr);
        println!("Successfully received remote memory region");
        println!("Remote memory region\n: {:?}", remote_mr);

        let mut qp = allocated_qp.handshake(remote_qp_endpoint).expect("failed handshake");

        handshake::wait_ack(&udp_session);

        println!("Performing RDMA read...");
        
        #[cfg(user_test)]
        {
            let result = unsafe { qp.rdma_read(
                &mut remote_mr, 
                vec![0..alloc_mem as u64], 
                &mut *mr, 
                vec![vec![0..alloc_mem]], 
                vec![1],
                vec![ibv_send_flags::SIGNALED]
            ).expect("ups ... something went wrong! ") };

            session::RdmaSession::poll_cq::<10>(&rdma_session.cq_send, 1);

            println!("Checking data integrity...");

            unsafe { flush_cache(& *mr) };

            unsafe { _mm_mfence() }; 

            let packet = unsafe { session::RdmaSession::read(&mut *mr, 0..alloc_mem) };
            
            let _ = integrity::validate_packet(packet)
                .map_err(|e| {
                    hit_wo_fault(packet, context_buffer, payload_f);
                    println!("Data integrity failed due to {:?}", e);
                    e
                });
        }

        #[cfg(user_bench)]
        {
            let payload = integrity::build_payload(ALLOC_MEM - META_DATA_SIZE, payload_f);

            let packet_len = integrity::build_packet(&payload[..], context_buffer).expect("failed to create packet");
            let packet = &context_buffer[..packet_len];
            
            unsafe { bench::rdma_bench(
                bench::SPEC_RDMA_TYPE::RDMA_READ, 
                alloc_mem, 
                &mut qp, 
                &mut *mr,
                &mut remote_mr,
                &rdma_session.cq_send,
                Some(packet)
            ) };
        }

        handshake::send_ack(&udp_session);
    }
    else {
        println!("Starting as RECEIVER");

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
        .build()
        .expect("build of allocated QP was not successful");

        handshake::send_ready_and_wait_ack(&udp_session, 10, 3000); // this has to be optimized since
        // otherwise we would fire to many ready messages and fill up the buffer to fast !
        
        let mr = &mut rdma_session.mr;

        let endpoint = allocated_qp.endpoint();
        let local_mr = mr.remote();

        let remote_qp_endpoint = handshake::exchange_endpoints(&udp_session, endpoint);
        println!("Successfully received remote endpoint : {:?}", remote_qp_endpoint);

        let _remote_mr = handshake::exchange_memory_region(&udp_session, local_mr);

        let _qp = allocated_qp.handshake(remote_qp_endpoint).expect("failed handshake");

        let payload = integrity::build_payload(ALLOC_MEM - META_DATA_SIZE, payload_f);

        let packet_len = integrity::build_packet(&payload[..], context_buffer).expect("failed to create packet");
        let packet = &context_buffer[..packet_len];

        session::RdmaSession::write(mr, packet, 0..alloc_mem);

        unsafe { _mm_mfence() }; 

        unsafe { flush_cache(mr) };

        handshake::send_ack(&udp_session);

        println!("Receiver finished sending data");

        handshake::wait_ack(&udp_session);

        println!("end - rdma read");
        //loop {}
    }

    udp_session.terminate();
}