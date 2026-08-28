use super::ALLOC_MEM;
use super::session::RdmaSession;
use alloc::{vec, vec::Vec};
use core::ops::Range;
use cpu_core::flush_cache;
use ibverbs::ffi::ibv_send_flags;
use ibverbs::{CompletionQueue, LocalMemoryRegion, QueuePair, RemoteMemoryRegion};
use spin::Once;
use terminal::println;
use time::get_time_in_us;

const ITERATIONS: usize = 1000;
const MAX_OUTSTANDING_BATCHES: usize = ITERATIONS + 24;
const WARMUP: usize = 5; // for better measurements we can warmup for N steps
const LOG_STEP: usize = 5; // debug only, otherwise logging would interfere with measurement
const BATCHES: usize = 3;
static LOCAL_RANGES: Once<[Vec<Vec<Range<usize>>>; BATCHES]> = Once::new();
static REMOTE_RANGES: Once<[Vec<Range<u64>>; BATCHES]> = Once::new();
static WORK_IDS: Once<[Vec<u64>; BATCHES]> = Once::new();
static SEND_FLAGS: Once<[Vec<ibv_send_flags>; BATCHES]> = Once::new();

#[derive(Copy, Clone, Debug)]
pub enum SpecRdmaType {
    RdmaRead,
    RdmaWrite,
}

#[derive(Copy, Clone, Debug)]
pub enum Benchmark {
    Latency,
    Throughput,
    Hit,
}

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

    for (i, parts) in [1, 2, 4, 8, 16, 32, 64, 128, 256].into_iter().enumerate().take(BATCHES) {
        let partitions = generate_partitions(ALLOC_MEM, parts);
        result[i] = partitions.into_iter().map(|part| vec![part]).collect();
    }

    result
}

fn remote_range_init() -> [Vec<Range<u64>>; BATCHES] {
    let mut result = [(); BATCHES].map(|_| Vec::new());

    for (i, parts) in [1, 2, 4, 8, 16, 32, 64, 128, 256].into_iter().enumerate().take(BATCHES) {
        let partitions_usize = generate_partitions(ALLOC_MEM, parts);
        let partitions_u64 = partitions_usize.into_iter().map(|r| (r.start as u64)..(r.end as u64));
        result[i].extend(partitions_u64);
    }

    result
}

fn work_id_init() -> [Vec<u64>; BATCHES] {
    core::array::from_fn(|i| {
        let parts = 1 << i;
        (1..=parts).map(|x| x as u64).collect()
    })
}

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
    rdma_type: SpecRdmaType, benchmark_type: Benchmark, alloc_mem: usize, qp: &mut QueuePair, mr: &mut LocalMemoryRegion<'_, u8>,
    remote_mr: &mut RemoteMemoryRegion<u8>, cq_send: &CompletionQueue, expected_packet: Option<&[u8]>,
) {
    LOCAL_RANGES.call_once(local_range_init);
    REMOTE_RANGES.call_once(remote_range_init);
    WORK_IDS.call_once(work_id_init);
    SEND_FLAGS.call_once(send_flags_init);

    let mut start_us = 0;

    let mut data_collect_per_batch: [[u64; ITERATIONS]; BATCHES] = [[0; ITERATIONS]; BATCHES];
    for batch_idx in 0..BATCHES {
        let r_ranges_ref = unsafe { &REMOTE_RANGES.get_unchecked()[batch_idx] };
        let l_ranges_ref = unsafe { &LOCAL_RANGES.get_unchecked()[batch_idx] };
        let w_ranges_ref = unsafe { &WORK_IDS.get_unchecked()[batch_idx] };
        let s_ranges_ref = unsafe { &SEND_FLAGS.get_unchecked()[batch_idx] };

        start_us = match benchmark_type {
            Benchmark::Throughput => get_time_in_us(),
            _ => start_us,
        };

        for i in 0..ITERATIONS {
            let r_ranges = r_ranges_ref.clone();
            let l_ranges = l_ranges_ref.clone();
            let w_ranges = w_ranges_ref.clone();
            let s_ranges = s_ranges_ref.clone();

            start_us = match benchmark_type {
                Benchmark::Latency => get_time_in_us(),
                _ => start_us,
            };

            let _result = match rdma_type {
                SpecRdmaType::RdmaRead => unsafe {
                    qp.rdma_read(remote_mr, r_ranges, mr, l_ranges, w_ranges, s_ranges)
                        .expect("problems during rdma read!")
                },
                SpecRdmaType::RdmaWrite => unsafe {
                    qp.rdma_write(mr, l_ranges, remote_mr, r_ranges, w_ranges, s_ranges)
                        .expect("problems during rdma write!")
                },
            };

            match benchmark_type {
                Benchmark::Latency => {
                    RdmaSession::poll_cq::<10>(cq_send, 1);
                    let end_us = get_time_in_us();
                    let elapsed_time_us = end_us - start_us;
                    data_collect_per_batch[batch_idx][i] = elapsed_time_us as u64;
                },
                Benchmark::Hit => {
                    RdmaSession::poll_cq::<10>(cq_send, 1);
                    let correct_bytes = get_correct_bytes_per_batch(mr, alloc_mem, expected_packet.unwrap());
                    data_collect_per_batch[batch_idx][i] = correct_bytes;
                },
                _ => (),
            }
        }

        match benchmark_type {
            Benchmark::Throughput => {
                RdmaSession::poll_cq::<MAX_OUTSTANDING_BATCHES>(cq_send, ITERATIONS);
                let end_us = get_time_in_us();
                let elapsed_time_us = end_us - start_us;
                data_collect_per_batch[batch_idx][0] = elapsed_time_us as u64;
            },
            _ => (),
        }
    }

    match benchmark_type {
        Benchmark::Latency => latency(&data_collect_per_batch[..]),
        Benchmark::Throughput => throughput(&data_collect_per_batch[..], alloc_mem),
        Benchmark::Hit => data_hit_rate(&data_collect_per_batch[..], alloc_mem),
    }
}

pub fn get_correct_bytes_per_batch(mr: &mut LocalMemoryRegion<'_, u8>, alloc_mem: usize, expected_packet: &[u8]) -> u64 {
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

fn data_hit_rate(data_buffer: &[[u64; ITERATIONS]], packet_size_bytes: usize) {
    for (batch_idx, batch) in data_buffer.iter().enumerate() {
        let total_correct_bytes: u64 = batch.iter().sum();
        let max_possible_bytes = (ITERATIONS * packet_size_bytes) as u64;
        let hit_rate = ((total_correct_bytes as f64) / (max_possible_bytes as f64)) * 100.0;

        println!("Batch {} hit rate: {:.2}%", batch_idx, hit_rate);
    }
}

fn latency(data_buffer: &[[u64; ITERATIONS]]) {
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

fn throughput(data_buffer: &[[u64; ITERATIONS]], packet_size_bytes: usize) {
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
