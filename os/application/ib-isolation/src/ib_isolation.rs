//! Two roles run in two separate processes that share a small memory segment:
//!
//! * The **victim** (the process started without arguments) opens the device and
//!   creates a full set of resources — a protection domain, a completion queue, a
//!   queue pair and a memory region — through the normal `ibverbs` API. It publishes
//!   their raw handle numbers into shared memory, spawns the attacker, and waits.
//! * The **attacker** (the process started with the `attack` argument) reads the
//!   victim's handles, including its context handle, out of shared memory and issues
//!   raw `uverbs` system calls that try to operate on them through that context. Because the attacker is a different process, every one
//!   of these calls must be rejected by the kernel.
//!
//! The attacker records the outcome of each attempt; the victim reads the results
//! back and prints a verdict. It also runs one positive control (the owner
//! performing the same kind of operation on its *own* resource) to prove that the
//! ownership check discriminates by owner rather than simply denying everyone.
//!
//! With the `leak` argument, the process instead creates the same resources and exits without
//! destroying any of them, the way a crashed application would. The kernel has to release them
//! when the process is cleaned up; run it more often than there are UAR pages to check that it
//! does (`released context of process ...` in the kernel log, and no "No UAR page available").
//!
//! Scope: this covers the *control path* (resource ownership). Data-path key
//! isolation — whether a stolen `rkey` can be used for a remote read/write against a
//! foreign memory region — needs a full RC connection between two QPs and is left as
//! future work.

#![no_std]

extern crate alloc;

use alloc::vec;
use core::mem::MaybeUninit;
use core::ptr::addr_of_mut;
use core::sync::atomic::{fence, Ordering};

use concurrent::{shm, thread};
use ibverbs::QueuePairType;
use rdma::ib_core::{AccessFlags, ContextHandle, PdHandle, QueuePairAttr, QueuePairAttrMask};
use rdma::uverbs_uapi::{
    CreateMrRequest, CreateMrResponse, DeallocPdRequest, DestroyRequest, ModifyQpRequest, UserSlice, UverbsCmd,
};
use runtime::*;
use syscall::return_vals::Errno;
use syscall::{syscall, SystemCall};
use terminal::println;

/// Name of the shared-memory segment the two processes rendezvous on.
const SHM_NAME: &str = "ib-iso-shm";
const SHM_SIZE: usize = 4096;

/// Number of distinct cross-process attacks the attacker attempts.
const N_ATTACKS: usize = 6;

/// Human-readable label for each attack index (kept in sync with `run_attacker`).
const ATTACK_LABELS: [&str; N_ATTACKS] = [
    "DeallocPd (foreign PD)",
    "RegMr     (foreign PD)",
    "DestroyCq (foreign CQ)",
    "ModifyQp  (foreign QP)",
    "DestroyQp (foreign QP)",
    "DeregMr   (foreign MR)",
];

/// Outcome of a single attempted verb, recorded by the attacker and read by the victim.
#[repr(C)]
#[derive(Clone, Copy)]
struct AttackOutcome {
    /// 1 once the attacker has attempted this verb.
    ran: u32,
    /// 1 if the `uverbs` call returned success. For a cross-process operation this
    /// means isolation was *breached*.
    allowed: u32,
    /// Raw errno returned on failure (negative), or 0 on success.
    errno: i64,
}

impl AttackOutcome {
    const EMPTY: Self = Self { ran: 0, allowed: 0, errno: 0 };
}

/// Layout shared between the victim and the attacker in the `ib-iso-shm` segment.
///
/// The victim fills in the handles and then publishes them by setting `ready`; the
/// attacker fills in `outcomes` and then publishes them by setting `done`. Ordering
/// between the two processes is established by these two flags (with fences) and by
/// the victim joining the attacker before reading the results.
#[repr(C)]
struct SharedState {
    device_handle: u64,
    outcomes: [AttackOutcome; N_ATTACKS],
    context: u32,
    pd: u32,
    cq_num: u32,
    qp_num: u32,
    mr_handle: u32,
    ready: u32,
    done: u32,
}

#[unsafe(no_mangle)]
pub fn main() {
    terminal::init_logger();

    // argv[0] is the binary name, argv[1..] are the arguments we were spawned with.
    let is_attacker = env::args().any(|arg| arg == "attack");
    let is_leaker = env::args().any(|arg| arg == "leak");

    if is_attacker {
        run_attacker();
    } else if is_leaker {
        run_leaker();
    } else {
        run_victim();
    }
}

// ───────────────────────────── leaker ─────────────────────────────

fn run_leaker() {
    let devices = match ibverbs::devices() {
        Ok(devices) => devices,
        Err(e) => { println!("  could not list RDMA devices: {:?}", e); return; }
    };
    let Some(device) = devices.get(0) else {
        println!("  no RDMA device found — this test needs a ConnectX-3 card");
        return;
    };
    let context = match device.open() {
        Ok(context) => context,
        Err(e) => { println!("  could not open device: {:?}", e); return; }
    };
    let pd = match context.alloc_pd() {
        Ok(pd) => pd,
        Err(e) => { println!("  alloc_pd failed: {:?}", e); return; }
    };
    let cq = match context.create_cq(16, 0) {
        Ok(cq) => cq,
        Err(e) => { println!("  create_cq failed: {:?}", e); return; }
    };
    let mr = match pd.allocate::<u8>(4096) {
        Ok(mr) => mr,
        Err(e) => { println!("  reg_mr failed: {:?}", e); return; }
    };
    let qp = match pd.create_qp(&cq, &cq, QueuePairType::RC).build() {
        Ok(qp) => qp,
        Err(e) => { println!("  create_qp failed: {:?}", e); return; }
    };
    println!("  leaking pd={} cq={} qp={} mr={}", pd.pd.0, cq.number(), qp.endpoint().num, mr.handle());
    // Skip every destructor, so that only the kernel can clean up.
    core::mem::forget(qp);
    core::mem::forget(mr);
    core::mem::forget(cq);
    core::mem::forget(pd);
    core::mem::forget(context);
}

// ───────────────────────────── victim ─────────────────────────────

fn run_victim() {
    println!("InfiniBand process-isolation test");

    // 1. Open the device and create a full set of resources through the normal API.
    let devices = match ibverbs::devices() {
        Ok(devices) => devices,
        Err(e) => {
            println!("  could not list RDMA devices: {:?}", e);
            return;
        }
    };
    let device = match devices.get(0) {
        Some(device) => device,
        None => {
            println!("  no RDMA device found — this test needs a ConnectX-3 card");
            return;
        }
    };
    let device_handle: usize = device.handle.into();

    let context = match device.open() {
        Ok(context) => context,
        Err(e) => {
            println!("  could not open device (is the port ACTIVE?): {:?}", e);
            return;
        }
    };

    let pd = match context.alloc_pd() {
        Ok(pd) => pd,
        Err(e) => { println!("  alloc_pd failed: {:?}", e); return; }
    };
    let cq = match context.create_cq(16, 0) {
        Ok(cq) => cq,
        Err(e) => { println!("  create_cq failed: {:?}", e); return; }
    };
    let mr = match pd.allocate::<u8>(4096) {
        Ok(mr) => mr,
        Err(e) => { println!("  reg_mr failed: {:?}", e); return; }
    };
    // build() creates (but does not connect) the QP, so no active fabric peer is needed.
    let qp = match pd.create_qp(&cq, &cq, QueuePairType::RC).build() {
        Ok(qp) => qp,
        Err(e) => { println!("  create_qp failed: {:?}", e); return; }
    };

    let context_handle = context.handle().0;
    let pd_handle = pd.pd.0;
    let cq_num = cq.number();
    let qp_num = qp.endpoint().num;
    let mr_handle = mr.handle();

    println!(
        "  victim   device={} context={} pd={} cq={} qp={} mr={}",
        device_handle, context_handle, pd_handle, cq_num, qp_num, mr_handle
    );

    // 2. Publish the handles into shared memory for the attacker.
    let shm_id = match shm::shm_open(SHM_NAME, SHM_SIZE, true) {
        Ok(id) => id,
        Err(e) => { println!("  shm_open failed: {:?}", e); return; }
    };
    let ptr = match shm::shm_attach(shm_id, false) {
        Ok(ptr) => ptr,
        Err(e) => { println!("  shm_attach failed: {:?}", e); return; }
    };

    unsafe {
        core::ptr::write_bytes(ptr, 0, SHM_SIZE);
        let st = ptr as *mut SharedState;
        addr_of_mut!((*st).device_handle).write(device_handle as u64);
        addr_of_mut!((*st).context).write(context_handle);
        addr_of_mut!((*st).pd).write(pd_handle);
        addr_of_mut!((*st).cq_num).write(cq_num);
        addr_of_mut!((*st).qp_num).write(qp_num);
        addr_of_mut!((*st).mr_handle).write(mr_handle);
        // Publish the handles before the attacker is allowed to read them.
        fence(Ordering::Release);
        core::ptr::write_volatile(addr_of_mut!((*st).ready), 1);
    }

    // 3. Spawn the attacker (same binary, "attack" argument) and wait for it.
    let child = match thread::start_application("ib-isolation", vec!["attack"]) {
        Some(child) => child,
        None => { println!("  failed to spawn attacker process"); cleanup(ptr); return; }
    };
    let _ = child.join();

    // Belt and suspenders: the attacker sets `done` as its last action. Give it a
    // moment in case join returns before the store is observable here.
    let st = ptr as *mut SharedState;
    let mut waited = 0;
    while unsafe { core::ptr::read_volatile(addr_of_mut!((*st).done)) } == 0 && waited < 1000 {
        thread::sleep(1);
        waited += 1;
    }
    fence(Ordering::Acquire);

    // 4. Read the attacker's results and render the verdict.
    let outcomes: [AttackOutcome; N_ATTACKS] = unsafe { addr_of_mut!((*st).outcomes).read() };
    report(&outcomes, &pd);

    cleanup(ptr);
}

/// Print the isolation verdict, and run the owner-side positive control.
fn report(outcomes: &[AttackOutcome; N_ATTACKS], pd: &ibverbs::ProtectionDomain) {
    println!("  cross-process attempts (every one must be DENIED):");

    let mut held = 0;
    let mut breaches = 0;
    for (i, outcome) in outcomes.iter().enumerate() {
        if outcome.ran == 0 {
            println!("   [ ?? ] {}   not attempted", ATTACK_LABELS[i]);
            continue;
        }
        if outcome.allowed == 0 {
            held += 1;
            println!("   [PASS] {}   denied ({})", ATTACK_LABELS[i], errno_name(outcome.errno));
        } else {
            breaches += 1;
            println!("   [FAIL] {}   ALLOWED — isolation breach!", ATTACK_LABELS[i]);
        }
    }

    // Positive control: the owner performing the same kind of PD-scoped operation on
    // its *own* protection domain must succeed. This proves the checks above reject
    // by owner, not unconditionally.
    println!("  positive control (owner must be allowed):");
    match pd.allocate::<u8>(256) {
        Ok(_) => println!("   [PASS] RegMr on own PD   allowed"),
        Err(e) => println!("   [FAIL] RegMr on own PD   denied ({:?}) — control failed", e),
    }

    println!(
        "  summary: {}/{} isolation checks held, {} breach(es)",
        held, N_ATTACKS, breaches
    );
}

/// Detach and unlink the shared-memory segment.
fn cleanup(ptr: *mut u8) {
    let _ = shm::shm_detach(ptr);
    let _ = shm::shm_unlink(SHM_NAME);
}

// ──────────────────────────── attacker ────────────────────────────

fn run_attacker() {
    let shm_id = match shm::shm_open(SHM_NAME, SHM_SIZE, false) {
        Ok(id) => id,
        Err(_) => return,
    };
    let ptr = match shm::shm_attach(shm_id, false) {
        Ok(ptr) => ptr,
        Err(_) => return,
    };
    let st = ptr as *mut SharedState;

    // Wait until the victim has published its handles.
    let mut waited = 0;
    while unsafe { core::ptr::read_volatile(addr_of_mut!((*st).ready)) } == 0 && waited < 5000 {
        thread::sleep(1);
        waited += 1;
    }
    fence(Ordering::Acquire);

    let device: usize = unsafe { addr_of_mut!((*st).device_handle).read() } as usize;
    let context = ContextHandle(unsafe { addr_of_mut!((*st).context).read() });
    let pd = PdHandle(unsafe { addr_of_mut!((*st).pd).read() });
    let cq_num: u32 = unsafe { addr_of_mut!((*st).cq_num).read() };
    let qp_num: u32 = unsafe { addr_of_mut!((*st).qp_num).read() };
    let mr_handle: u32 = unsafe { addr_of_mut!((*st).mr_handle).read() };

    let mut outcomes = [AttackOutcome::EMPTY; N_ATTACKS];

    // 0: deallocate the victim's protection domain.
    {
        let req = DeallocPdRequest { context, pd };
        outcomes[0] = attempt(device, UverbsCmd::DeallocPd, UserSlice::from_ref(&req), UserSlice::EMPTY);
    }

    // 1: register a (locally owned) buffer against the victim's protection domain.
    {
        let mut buffer = vec![0u8; 4096];
        let req = CreateMrRequest {
            context,
            pd,
            access_flags: AccessFlags::LOCAL_WRITE,
            data_ptr: buffer.as_mut_ptr() as u64,
            len: buffer.len() as u64,
        };
        let mut resp = MaybeUninit::<CreateMrResponse>::uninit();
        outcomes[1] = attempt(
            device, UverbsCmd::RegMr,
            UserSlice::from_ref(&req), UserSlice::from_mut(&mut resp),
        );
    }

    // 2: destroy the victim's completion queue.
    {
        let req = DestroyRequest { context, handle: cq_num };
        outcomes[2] = attempt(device, UverbsCmd::DestroyCq, UserSlice::from_ref(&req), UserSlice::EMPTY);
    }

    // 3: modify the victim's queue pair.
    {
        let req = ModifyQpRequest {
            context,
            qp_num,
            attr: QueuePairAttr::default(),
            attr_mask: QueuePairAttrMask::IBV_QP_STATE,
        };
        outcomes[3] = attempt(device, UverbsCmd::ModifyQp, UserSlice::from_ref(&req), UserSlice::EMPTY);
    }

    // 4: destroy the victim's queue pair.
    {
        let req = DestroyRequest { context, handle: qp_num };
        outcomes[4] = attempt(device, UverbsCmd::DestroyQp, UserSlice::from_ref(&req), UserSlice::EMPTY);
    }

    // 5: deregister the victim's memory region.
    {
        let req = DestroyRequest { context, handle: mr_handle };
        outcomes[5] = attempt(device, UverbsCmd::DeregMr, UserSlice::from_ref(&req), UserSlice::EMPTY);
    }

    // Publish the results, then signal completion.
    unsafe {
        addr_of_mut!((*st).outcomes).write(outcomes);
        fence(Ordering::Release);
        core::ptr::write_volatile(addr_of_mut!((*st).done), 1);
    }

    let _ = shm::shm_detach(ptr);
}

/// Issue one raw `uverbs` system call and classify the outcome.
fn attempt(device: usize, cmd: UverbsCmd, user_in: UserSlice, user_out: UserSlice) -> AttackOutcome {
    match raw_uverbs(device, cmd, user_in, user_out) {
        Ok(_) => AttackOutcome { ran: 1, allowed: 1, errno: 0 },
        Err(e) => {
            let code: isize = e.into();
            AttackOutcome { ran: 1, allowed: 0, errno: code as i64 }
        }
    }
}

/// The raw `Uverb` system call, bypassing the `ibverbs` wrapper so the exact errno
/// is preserved. Argument order matches `ibverbs::uverbs`.
fn raw_uverbs(
    device: usize, cmd: UverbsCmd, user_in: UserSlice, user_out: UserSlice,
) -> Result<usize, Errno> {
    syscall(SystemCall::Uverb, &[
        device,
        cmd as u64 as usize,
        user_in.address as usize,
        user_in.size,
        user_out.address as usize,
        user_out.size,
    ])
}

/// Best-effort name for an errno code, for the report.
fn errno_name(code: i64) -> &'static str {
    match code {
        -8 => "EINVAL",
        -9 => "EINVALH",
        -5 => "EACCES",
        -2 => "ENOENT",
        -22 => "EFAULT",
        _ => "denied",
    }
}
