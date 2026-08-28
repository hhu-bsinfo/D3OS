use super::{session, handshake, integrity};
use mm::{MmapFlags, mmap};
use ibverbs::devices;
use ibverbs::ffi::ibv_send_flags;
use super::bench;
use super::*;
use alloc::{vec};
use cpu_core::{flush_cache};
use core::{arch::x86_64::_mm_mfence, net::SocketAddr};
use concurrent::thread::sleep;

pub fn invoke(config: RunConfig) {
    let min_cq_entries = 1000;
    let alloc_mem = ALLOC_MEM;
    let context_buffer = mmap(
        30 * 1024 * 1024 * 1024 * 1024,
        CONTEXT_BUFFER_SIZE,
        MmapFlags::ANONYMOUS | MmapFlags::POPULATE,
    )
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
            }
        }
    };

    println!("obtained device context");

    let pd = ctx.alloc_pd().expect("failed to allocate protection domain");

    let mut rdma_session = session::RdmaSession::new(&ctx, &pd, alloc_mem, min_cq_entries);
    let tcp_stream  = network::TcpStream::connect(SocketAddr::new(core::net::IpAddr::V4(config.target_ip), config.target_port)).unwrap();

    sleep(1000); // give some time for the memory regions

    let payload_f = integrity::PAYLOAD_FUNCTIONS.lcg;

    if config.is_sender {
        println!("Starting as SENDER");
        let max_send_wr = 10;
        let max_send_sge = 1;
        let allocated_qp = session::RdmaSession::create_qp(
            rdma_session.pd,
            &rdma_session.cq_send,
            &rdma_session.cq_recv,
            false,
            max_send_wr,
            0,
            max_send_sge,
            0,
        )
        .set_timeout(20)
        .set_min_rnr_timer(30)
        .build()
        .expect("build of allocated QP was not successful");

        handshake::wait_ready(&tcp_stream);
        handshake::send_ack(&tcp_stream);

        let endpoint = allocated_qp.endpoint();
        let local_mr = rdma_session.mr.remote();

        let remote_qp_endpoint = handshake::exchange_endpoints(&tcp_stream, endpoint);
        println!("Successfully received remote endpoint : {:?}", remote_qp_endpoint);

        let mut remote_mr = handshake::exchange_memory_region(&tcp_stream, local_mr);
        println!("Successfully received remote memory region");
        println!("Remote memory region\n: {:?}", remote_mr);

        let mut qp = allocated_qp.handshake(remote_qp_endpoint).expect("failed handshake");

        handshake::wait_ack(&tcp_stream);

        println!("Performing RDMA read...");

        if config.only_test {
            let _ = unsafe { qp.rdma_read(
                &mut remote_mr,
                vec![0..alloc_mem as u64],
                &mut rdma_session.mr,
                vec![vec![0..alloc_mem]],
                vec![1],
                vec![ibv_send_flags::SIGNALED]
            ).expect("ups ... something went wrong!") };

            session::RdmaSession::poll_cq::<10>(&rdma_session.cq_send, 1);

            println!("Checking data integrity...");

            unsafe { flush_cache(&rdma_session.mr) };

            unsafe { _mm_mfence() };

            let packet = session::RdmaSession::read(&rdma_session.mr, 0..alloc_mem);

            let _ = integrity::validate_packet(packet)
                .map_err(|e| {
                    hit_wo_fault(packet, context_buffer, payload_f);
                    println!("Data integrity failed due to {:?}", e);
                    e
                });
        } else {
            let payload = integrity::build_payload(ALLOC_MEM - META_DATA_SIZE, payload_f);
            let packet_len = integrity::build_packet(&payload[..], context_buffer).expect("failed to create packet");
            let packet = &context_buffer[..packet_len];

            bench::rdma_bench(
                bench::SpecRdmaType::RdmaRead,
                config.benchmark,
                alloc_mem,
                &mut qp,
                &mut rdma_session.mr,
                &mut remote_mr,
                &rdma_session.cq_send,
                Some(packet)
            );
        }

        handshake::send_ack(&tcp_stream);
    } else {
        println!("Starting as RECEIVER");
        let allocated_qp = session::RdmaSession::create_qp(rdma_session.pd, &rdma_session.cq_send, &rdma_session.cq_recv, true, 0, 0, 0, 0)
            .build()
            .expect("build of allocated QP was not successful");

        // this has to be optimized since
        // otherwise we would fire to many ready messages and fill up the buffer to fast!
        handshake::send_ready_and_wait_ack(&tcp_stream, 10, 3000);

        let endpoint = allocated_qp.endpoint();
        let local_mr = rdma_session.mr.remote();

        let remote_qp_endpoint = handshake::exchange_endpoints(&tcp_stream, endpoint);
        println!("Successfully received remote endpoint : {:?}", remote_qp_endpoint);

        let _remote_mr = handshake::exchange_memory_region(&tcp_stream, local_mr);

        let _qp = allocated_qp.handshake(remote_qp_endpoint).expect("failed handshake");

        let payload = integrity::build_payload(ALLOC_MEM - META_DATA_SIZE, payload_f);
        let packet_len = integrity::build_packet(&payload[..], context_buffer).expect("failed to create packet");
        let packet = &context_buffer[..packet_len];

        session::RdmaSession::write(&mut rdma_session.mr, packet, 0..alloc_mem);

        unsafe { _mm_mfence() };

        unsafe { flush_cache(&rdma_session.mr) };

        handshake::send_ack(&tcp_stream);

        println!("Receiver finished sending data");

        handshake::wait_ack(&tcp_stream);

        println!("end - rdma read");
        //loop {}
    }
}
