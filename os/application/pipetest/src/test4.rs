//! TEST4: naming-handle ownership and reclamation on process exit.
//!
//! These cases need a second *process* (ownership is per process), so `pipetest`
//! doubles as its own child: `main` dispatches to [`child_dispatch`] when argv[1]
//! names a child role, otherwise it runs [`test4_run`] as the parent.
//!
//! Leak detection avoids exhausting the 4096-slot table (every `open`/`close`
//! logs at `Info`, so a full sweep would emit thousands of serial lines).
//! Instead it uses the allocator's lowest-free-slot policy: [`probe_slot`] opens
//! one handle and returns the id it received, which is the lowest free slot at
//! that instant. If a child leaks N handles, the next probe id jumps by ~N; if
//! the kernel reclaims them on exit, the id stays put.

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use naming::shared_types::{OpenOptions, SeekOrigin};
use naming::{close, mkfifo, open, read, seek, touch, write};
use syscall::return_vals::Errno;

use concurrent::thread;
#[allow(unused_imports)]
use runtime::*;
use terminal::println;

const PROBE_PATH: &str = "/probe4";
const EOF_FIFO_PATH: &str = "/mypipe4_eof";
const EPIPE_FIFO_PATH: &str = "/mypipe4_epipe";
const HANDLE_ENV_KEY: &str = "D3OS_TEST_HANDLE";

/// Number of handles a child leaks; must be well above measurement noise.
const LEAK_COUNT: usize = 64;
/// Max id drift attributable to unrelated concurrent opens.
const LEAK_THRESHOLD: usize = 16;

/// Open one handle on the probe file and return the slot id it was given (the
/// lowest free slot at this instant), closing it again immediately.
fn probe_slot() -> Option<usize> {
    match open(PROBE_PATH, OpenOptions::READONLY) {
        Ok(handle) => {
            let _ = close(handle);
            Some(handle)
        }
        Err(_) => None,
    }
}

/// Dispatch a child-process role selected via argv. Returns `true` if `role`
/// matched a child role (and was handled), `false` if this is the parent.
pub fn child_dispatch(role: &str) -> bool {
    match role {
        "child-leak" => child_leak(),
        "child-writer" => child_writer(),
        "child-reader" => child_reader(),
        "child-foreign" => child_foreign(),
        _ => return false,
    }
    true
}

/// Child: open `LEAK_COUNT` handles and exit without closing them. If the kernel
/// reclaims owned handles on exit, none of these slots stay occupied.
fn child_leak() {
    let _ = touch(PROBE_PATH);
    let mut count = 0;
    for _ in 0..LEAK_COUNT {
        if open(PROBE_PATH, OpenOptions::READONLY).is_ok() {
            count += 1;
        }
    }
    println!("TEST4: child-leak opened {} handles, exiting without close", count);
    // Return -> runtime calls process::exit() -> kernel reclaim sweep.
}

/// Child: open the EOF FIFO for writing, send a marker, and exit without
/// closing. Reclaim must drop `writer_count` so the parent's reader sees EOF.
fn child_writer() {
    let writer = match open(EOF_FIFO_PATH, OpenOptions::WRITEONLY) {
        Ok(handle) => handle,
        Err(e) => {
            println!("TEST4: child-writer open failed: {:?}", e);
            return;
        }
    };
    let _ = write(writer, b"M");
    // Exit without close: reclaim drops the write endpoint.
}

/// Child: open the EPIPE FIFO for reading and exit without closing. Reclaim must
/// drop `reader_count` so the parent's writer gets EPIPE.
fn child_reader() {
    let reader = match open(EPIPE_FIFO_PATH, OpenOptions::READONLY) {
        Ok(handle) => handle,
        Err(e) => {
            println!("TEST4: child-reader open failed: {:?}", e);
            return;
        }
    };
    // Let the parent's WRITEONLY open rendezvous before we drop the endpoint.
    thread::sleep(200);
    let _ = reader;
    // Exit without close: reclaim drops the read endpoint.
}

/// Child: try to use a handle owned by the parent (passed via env). Every naming
/// operation must be rejected with `EINVALH`.
fn child_foreign() {
    let handle = match runtime::env::var(HANDLE_ENV_KEY).and_then(|s| s.parse::<usize>().ok()) {
        Some(handle) => handle,
        None => {
            println!("TEST4: FAIL child-foreign missing {} env", HANDLE_ENV_KEY);
            return;
        }
    };

    let mut buf = [0u8; 4];
    let read_res = read(handle, &mut buf);
    let seek_res = seek(handle, 0, SeekOrigin::Start);
    let close_res = close(handle);

    let rejected = matches!(read_res, Err(Errno::EINVALH))
        && matches!(seek_res, Err(Errno::EINVALH))
        && matches!(close_res, Err(Errno::EINVALH));

    if rejected {
        println!("TEST4: child rejected on foreign read/seek/close OK");
    } else {
        println!(
            "TEST4: FAIL foreign not rejected: read={:?} seek={:?} close={:?}",
            read_res, seek_res, close_res
        );
    }
}

/// Case 1: a process's owned handles are freed when it exits.
fn test_exit_frees_handles() {
    let _ = touch(PROBE_PATH);
    let Some(base) = probe_slot() else {
        println!("TEST4: FAIL could not probe base slot");
        return;
    };

    let child = match thread::start_application("pipetest", vec!["child-leak"]) {
        Some(child) => child,
        None => {
            println!("TEST4: FAIL could not launch child-leak");
            return;
        }
    };
    let _ = child.join();
    thread::sleep(50);

    let Some(after) = probe_slot() else {
        println!("TEST4: FAIL could not probe slot after child exit");
        return;
    };

    let delta = after.saturating_sub(base);
    if delta <= LEAK_THRESHOLD {
        println!("TEST4: exit frees owned handles OK (base={}, after={}, delta={})", base, after, delta);
    } else {
        println!("TEST4: FAIL exit leaked handles (base={}, after={}, delta={})", base, after, delta);
    }
}

/// Case 2a: after a writer process exits, its endpoint is reclaimed and the
/// reader observes EOF (`Ok(0)`) instead of blocking forever.
fn test_pipe_eof_on_writer_exit() {
    let _ = mkfifo(EOF_FIFO_PATH);

    let child = match thread::start_application("pipetest", vec!["child-writer"]) {
        Some(child) => child,
        None => {
            println!("TEST4: FAIL could not launch child-writer");
            return;
        }
    };

    // Blocking read side; rendezvous with the child's WRITEONLY open.
    let reader = match open(EOF_FIFO_PATH, OpenOptions::READONLY) {
        Ok(handle) => handle,
        Err(e) => {
            println!("TEST4: FAIL parent reader open failed: {:?}", e);
            let _ = child.join();
            return;
        }
    };

    let mut buf = [0u8; 16];
    let marker = read(reader, &mut buf).unwrap_or(0);
    let _ = child.join();
    thread::sleep(50);

    // Writer endpoint is gone via reclaim: this must return EOF, not hang.
    match read(reader, &mut buf) {
        Ok(0) => println!("TEST4: reader EOF after writer exit OK (marker={} bytes)", marker),
        Ok(n) => println!("TEST4: FAIL unexpected {} bytes after writer exit", n),
        Err(e) => println!("TEST4: FAIL read error after writer exit: {:?}", e),
    }
    let _ = close(reader);
}

/// Case 2b: after a reader process exits, its endpoint is reclaimed and the
/// writer gets `EPIPE`.
fn test_pipe_epipe_on_reader_exit() {
    let _ = mkfifo(EPIPE_FIFO_PATH);

    let child = match thread::start_application("pipetest", vec!["child-reader"]) {
        Some(child) => child,
        None => {
            println!("TEST4: FAIL could not launch child-reader");
            return;
        }
    };

    // Blocking write-side open; rendezvous with the child's READONLY open.
    let writer = match open(EPIPE_FIFO_PATH, OpenOptions::WRITEONLY) {
        Ok(handle) => handle,
        Err(e) => {
            println!("TEST4: FAIL parent writer open failed: {:?}", e);
            let _ = child.join();
            return;
        }
    };

    let _ = child.join();
    thread::sleep(50);

    // Reader endpoint is gone via reclaim: write must return EPIPE.
    match write(writer, b"x") {
        Err(Errno::EPIPE) => println!("TEST4: writer EPIPE after reader exit OK"),
        Ok(n) => println!("TEST4: FAIL wrote {} bytes after reader exit", n),
        Err(e) => println!("TEST4: FAIL unexpected error after reader exit: {:?}", e),
    }
    let _ = close(writer);
}

/// Case 3: another process cannot use one of our handles, and its rejected
/// operations do not disturb our handle.
fn test_foreign_handle_rejected() {
    let _ = touch(PROBE_PATH);
    let handle = match open(PROBE_PATH, OpenOptions::READONLY) {
        Ok(handle) => handle,
        Err(e) => {
            println!("TEST4: FAIL parent open failed: {:?}", e);
            return;
        }
    };


    let handle_entry = format!("{}={}", HANDLE_ENV_KEY, handle);
    let session_entry = runtime::env::var(terminal::session::SESSION_DESCRIPTOR_KEY)
        .map(|id| format!("{}={}", terminal::session::SESSION_DESCRIPTOR_KEY, id));
    let env: Vec<&str> = match session_entry.as_deref() {
        Some(session) => vec![handle_entry.as_str(), session],
        None => vec![handle_entry.as_str()],
    };
    let child = match thread::start_application_with_env("pipetest", vec!["child-foreign"], env) {
        Some(child) => child,
        None => {
            println!("TEST4: FAIL could not launch child-foreign");
            let _ = close(handle);
            return;
        }
    };
    let _ = child.join();
    thread::sleep(50);

    // Our handle must still work: the foreign read/seek/close must not have
    // touched it, and the foreign close in particular must not have freed it.
    let mut buf = [0u8; 4];
    match read(handle, &mut buf) {
        Ok(_) => println!("TEST4: own handle still readable after foreign attempt OK"),
        Err(e) => println!("TEST4: FAIL own handle read error: {:?}", e),
    }
    match close(handle) {
        Ok(_) => println!("TEST4: own handle close OK (foreign close did not free it)"),
        Err(e) => println!("TEST4: FAIL own handle close error: {:?}", e),
    }
}

/// Case 5 (automated part): repeatedly launch and join a short-lived app that
/// opens a session `out` writer; owned-handle reclaim must keep the slot usage
/// bounded across many spawns.
fn test_restart_soak() {
    let Some(base) = probe_slot() else {
        println!("TEST4: FAIL could not probe soak base slot");
        return;
    };

    let mut launched = 0;
    for _ in 0..50 {
        if let Some(child) = thread::start_application("hello", Vec::new()) {
            let _ = child.join();
            launched += 1;
        }
    }
    thread::sleep(50);

    let Some(after) = probe_slot() else {
        println!("TEST4: FAIL could not probe soak slot after spawns");
        return;
    };

    let delta = after.saturating_sub(base);
    if delta <= LEAK_THRESHOLD {
        println!("TEST4: restart soak bounded OK ({} spawns, base={}, after={}, delta={})", launched, base, after, delta);
    } else {
        println!("TEST4: FAIL restart soak leaked ({} spawns, base={}, after={}, delta={})", launched, base, after, delta);
    }
}

pub fn test4_run() {
    println!("TEST4: naming-handle ownership and reclamation");
    test_exit_frees_handles();
    test_pipe_eof_on_writer_exit();
    test_pipe_epipe_on_reader_exit();
    test_foreign_handle_rejected();
    test_restart_soak();
    println!("TEST4: OK");
}
