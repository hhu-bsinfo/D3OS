use rdma_core::{
    LocalMemoryRegion, RemoteMemoryRegion, QueuePair, CompletionQueue
};
use rdma::ibv_send_flags;
use super::session::RdmaSession;
use alloc::{vec, vec::Vec};
use core::ops::Range;
use super::ALLOC_MEM;
use spin::Once;
use time::get_time_in_us;
use cpu_core::{flush_cache};
use terminal::{println, print};

const ITERATIONS: usize = 10;
const MAX_OUTSTANDING_BATCHES: usize = ITERATIONS + 24;
const WARMUP: usize = 5; // for better measurements we can warmup for N steps
const LOG_STEP: usize = 5; // debug only, otherwise logging would interfere with measurement
const BATCHES: usize = 3;
static LOCAL_RANGES: Once<[Vec<Vec<Range<usize>>>; BATCHES]> = Once::new();
static REMOTE_RANGES: Once<[Vec<Range<u64>>; BATCHES]> = Once::new();
static WORK_IDS: Once<[Vec<u64>; BATCHES]> = Once::new();
static SEND_FLAGS: Once<[Vec<ibv_send_flags>; BATCHES]> = Once::new();

#[derive(Copy, Clone)]
pub enum SPEC_TRANSFER_TYPE {
    SEND,
    RECV
}

#[derive(Copy, Clone)]
pub enum SPEC_RDMA_TYPE {
    RDMA_READ,
    RDMA_WRITE
}

pub enum BENCHMARK {
    LATENCY,
    THROUGHPUT,
    HIT
}

pub struct bench_mark_ops {
    hit_rate_op: fn(&[[u64; ITERATIONS]], usize),
    latency_op: fn(&[[u64; ITERATIONS]]),
    throughput_op: fn(&[[u64; ITERATIONS]], usize)
}

const BENCH_MARK_OPS: bench_mark_ops = bench_mark_ops {
    hit_rate_op: data_hit_rate,
    latency_op: latency,
    throughput_op: throughput
};

// benchmarks:
// 1st : batch-size 1
// 2nd : batch-size 2
// 3rd : batch-size 4

// To control amount to send we have to manually set the alloc_mem variables to ALLOC_MEM_* in the
// corresponding modules

fn generate_partitions(total: usize, parts: usize) -> Vec<Range<usize>> {
    assert!(parts > 0 && ((total % parts) == 0), "must divide evenly");
    let chunk = total / parts;

    (0..parts)
        .map(|i| {
            let start = i * chunk;
            let end = start + chunk;
            start..end
        })
        .collect()
}

fn local_range_init() -> [Vec<Vec<Range<usize>>>; BATCHES] {
    let mut result = [(); BATCHES].map(|_| Vec::new());

    for (i, parts) in [1, 2, 4, 8, 16, 32, 64, 128, 256]
        .into_iter()
        .enumerate()
        .take(BATCHES) 
    {
        let partitions = generate_partitions(ALLOC_MEM, parts);
        result[i] = partitions.into_iter()
            .map(|part| vec![part])
            .collect();
    }

    result
}

/*fn local_range_init() -> [Vec<Vec<Range<usize>>>; BATCHES] {
    [
        vec![vec![0..ALLOC_MEM]], 
        vec![vec![0..(ALLOC_MEM/2)], vec![(ALLOC_MEM/2)..ALLOC_MEM]],
        vec![vec![0..(ALLOC_MEM/4)], 
            vec![(ALLOC_MEM/4)..(ALLOC_MEM/2)],
            vec![(ALLOC_MEM/2)..(ALLOC_MEM - (ALLOC_MEM/4))],
            vec![(ALLOC_MEM - (ALLOC_MEM/4))..ALLOC_MEM]]
    ]
} */

fn remote_range_init() -> [Vec<Range<u64>>; BATCHES] {
    let mut result = [(); BATCHES].map(|_| Vec::new());

    for (i, parts) in [1, 2, 4, 8, 16, 32, 64, 128, 256]
        .into_iter()
        .enumerate()
        .take(BATCHES)
    {
        let partitions_usize = generate_partitions(ALLOC_MEM, parts);
        let partitions_u64 = partitions_usize
            .into_iter()
            .map(|r| (r.start as u64)..(r.end as u64));
        result[i].extend(partitions_u64);
    }

    result
}

/* fn remote_range_init() -> [Vec<Range<u64>>; BATCHES] {
    [
        vec![0..(ALLOC_MEM as u64)], 
        vec![0..((ALLOC_MEM/2) as u64), ((ALLOC_MEM/2) as u64)..ALLOC_MEM as u64], 
        vec![0..((ALLOC_MEM / 4) as u64), 
        ((ALLOC_MEM/4) as u64)..((ALLOC_MEM/2) as u64), 
        ((ALLOC_MEM/2) as u64)..((ALLOC_MEM - (ALLOC_MEM/4)) as u64), 
        ((ALLOC_MEM - (ALLOC_MEM/4)) as u64)..(ALLOC_MEM as u64)]
    ]
} */

fn work_id_init() -> [Vec<u64>; BATCHES] {
    core::array::from_fn(|i| {
        let parts = 1 << i;
        (1..=parts).map(|x| x as u64).collect()
    })
}

/*fn work_id_init() -> [Vec<u64>; BATCHES] {
    [
        vec![1],
        vec![1,2],
        vec![1,2,3,4]
    ]
}*/

/*fn send_flags_init() -> [Vec<ibv_send_flags>; BATCHES] {
    [
        vec![ibv_send_flags::SIGNALED],
        vec![ibv_send_flags::empty(), ibv_send_flags::SIGNALED],
        vec![ibv_send_flags::empty(), ibv_send_flags::empty(),
            ibv_send_flags::empty(), ibv_send_flags::SIGNALED]
    ]
} */

fn send_flags_init() -> [Vec<ibv_send_flags>; BATCHES] {
    core::array::from_fn(|i| {
        let wr_count = 1 << i;
        let mut flags = vec![ibv_send_flags::empty(); wr_count];
        // set the last WR as signaled
        if let Some(last) = flags.last_mut() {
            *last = ibv_send_flags::SIGNALED;
        }
        flags
    })
}

// cloning generates a bit of overhead, but for now we'll leave it that way !
pub fn rdma_bench( 
    rdma_type: SPEC_RDMA_TYPE, 
    alloc_mem: usize, 
    qp: &mut QueuePair<'_>, 
    mr: &mut LocalMemoryRegion<'_, u8>,
    remote_mr: &mut RemoteMemoryRegion<u8>,
    cq_send: &CompletionQueue<'_>,
    expected_packet: Option<&[u8]>) {

    LOCAL_RANGES.call_once(local_range_init);
    REMOTE_RANGES.call_once(remote_range_init);
    WORK_IDS.call_once(work_id_init);
    SEND_FLAGS.call_once(send_flags_init);

    #[cfg(any(throughput, latency))]
    let mut start_us = 0;

    let mut data_collect_per_batch: [[u64; ITERATIONS]; BATCHES] = [[0; ITERATIONS]; BATCHES];
    for batch_idx in 0..BATCHES {
        let r_ranges_ref = unsafe { &REMOTE_RANGES.get_unchecked()[batch_idx] };
        let l_ranges_ref = unsafe { &LOCAL_RANGES.get_unchecked()[batch_idx] };
        let w_ranges_ref = unsafe { &WORK_IDS.get_unchecked()[batch_idx] };
        let s_ranges_ref = unsafe { &SEND_FLAGS.get_unchecked()[batch_idx] };

        #[cfg(throughput)]
        {
            start_us = get_time_in_us();
        }

        for i in 0..ITERATIONS {
            let r_ranges = r_ranges_ref.clone();
            let l_ranges = l_ranges_ref.clone();
            let w_ranges = w_ranges_ref.clone();
            let s_ranges = s_ranges_ref.clone();
            
            #[cfg(latency)]
            {
                start_us = get_time_in_us();
            }
            
            
            let _result = match rdma_type {
                SPEC_RDMA_TYPE::RDMA_READ => unsafe { qp.rdma_read(
                    remote_mr, 
                    r_ranges, 
                    mr, 
                    l_ranges, 
                    w_ranges,
                    s_ranges).expect("problems during rdma read!")
                },
                SPEC_RDMA_TYPE::RDMA_WRITE => unsafe { qp.rdma_write(
                    mr, 
                    l_ranges, 
                    remote_mr, 
                    r_ranges, 
                    w_ranges,
                    s_ranges).expect("problems during rdma write!")
                }
            };

            #[cfg(any(hit, latency))]
            RdmaSession::poll_cq::<10>(cq_send, 1);

            #[cfg(latency)]
            {
                let end_us = get_time_in_us();
                let elapsed_time_us = end_us - start_us;
                data_collect_per_batch[batch_idx][i] = elapsed_time_us as u64;
            }

            #[cfg(all(hit, read))]
            {
                let correct_bytes = get_correct_bytes_per_batch(
                    mr,
                    alloc_mem,
                    expected_packet.unwrap()
                );
                data_collect_per_batch[batch_idx][i] = correct_bytes;
            }
        }

        #[cfg(throughput)]
        {
            RdmaSession::poll_cq::<MAX_OUTSTANDING_BATCHES>(cq_send, ITERATIONS);

            let end_us = get_time_in_us();
            let elapsed_time_us = end_us - start_us;
            data_collect_per_batch[batch_idx][0] = elapsed_time_us as u64;
        }
    }

    #[cfg(latency)]
    (BENCH_MARK_OPS.latency_op)(&data_collect_per_batch[..]);

    #[cfg(throughput)]
    (BENCH_MARK_OPS.throughput_op)(&data_collect_per_batch[..], alloc_mem);

    #[cfg(all(hit, read))]
    (BENCH_MARK_OPS.hit_rate_op)(&data_collect_per_batch[..], alloc_mem);
}

pub fn get_correct_bytes_per_batch(mr: &mut LocalMemoryRegion<'_, u8>, 
    alloc_mem: usize, expected_packet: &[u8]) -> u64 {
    let mut correct_bytes = 0u64;

    unsafe { flush_cache(mr) };

    let packet = RdmaSession::read(mr, 0..alloc_mem);

    for (b, &expected) in packet.iter().zip(expected_packet.iter()) {
        if *b == expected {
            correct_bytes += 1;
        }
    }

    correct_bytes
}

/*pub fn transfer_bench() {
    for batch_size in (0..BATCHES) {
        for i in (0..ITERATIONS) {
            #[cfg(hit)]
            (BENCH_MARK_OPS.hit_rate_op)();

            #[cfg(latency)]
            (BENCH_MARK_OPS.latency_op)();

            #[cfg(throughput)]
            (BENCH_MARK_OPS.throughput_op)();
        }
    }
} */

fn data_hit_rate(data_buffer : &[[u64; ITERATIONS]], packet_size_bytes: usize) {
    for (batch_idx, batch) in data_buffer.iter().enumerate() {
        let total_correct_bytes: u64 = batch.iter().sum();
        let max_possible_bytes = (ITERATIONS * packet_size_bytes) as u64;
        let hit_rate = ((total_correct_bytes as f64) / (max_possible_bytes as f64)) * 100.0;

        println!("Batch {} hit rate: {:.2}%", batch_idx, hit_rate);
    }
}

fn latency(data_buffer : &[[u64; ITERATIONS]]) {
    for (batch_idx, batch) in data_buffer.iter().enumerate() {
        println!("--- Batch {} ---", batch_idx);

        // Print each latency in the batch
        for (i, &lat) in batch.iter().enumerate() {
            println!("Iteration {} latency: {} us", i, lat);
        }

        let mut sorted = *batch;
        sorted.sort_unstable();

        let sum: u64 = batch.iter().sum();
        let average = sum as f64 / ITERATIONS as f64;
        let median = sorted[ITERATIONS / 2];
        let min_val = sorted[0];
        let max_val = sorted[ITERATIONS - 1];

        println!(
            "Batch {} latency -> avg: {:.2} us, median: {} us, min: {} us, max: {} us",
            batch_idx, average, median, min_val, max_val
        );
    }
}

fn throughput(data_buffer : &[[u64; ITERATIONS]], packet_size_bytes: usize) {
    for (batch_idx, batch) in data_buffer.iter().enumerate() {
        let total_time_us: u64 = batch.iter().sum(); // we could sub this with batch[0] its the same
        let total_bytes = (ITERATIONS * packet_size_bytes) as f64;

        // Convert to bytes per second
        let bandwidth_bytes_per_sec = (total_bytes / (total_time_us as f64)) * 1_000_000.0;

        let bandwidth_mb_per_sec = bandwidth_bytes_per_sec / 1_000_000.0;
        let bandwidth_gb_per_sec = bandwidth_bytes_per_sec / 1_000_000_000.0;
        let bandwidth_gbps = bandwidth_gb_per_sec * 8.0;

        println!(
            "Batch {} bandwidth: {:.2} MB/s | {:.2} GB/s | {:.2} Gbps",
            batch_idx, bandwidth_mb_per_sec, bandwidth_gb_per_sec, bandwidth_gbps
        );
    }
}