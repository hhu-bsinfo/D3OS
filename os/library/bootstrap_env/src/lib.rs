/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: bootstrap_env                                                   ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Shared userspace reader for process bootstrap environment vars. ║
   ║                                                                         ║
   ║ The kernel writes a small bootstrap page for each launched process at a ║
   ║ fixed userspace address. That page contains argc, argv pointers, envp   ║
   ║ pointers, and the backing NUL-terminated strings. This crate is the     ║
   ║ single userspace parser for the envp portion of that layout.            ║
   ║                                                                         ║
   ║ Keeping this logic in a leaf crate lets `runtime` expose env lookup and ║
   ║ lets `terminal` resolve `D3OS_TERM_SESSION` without introducing a       ║
   ║ dependency cycle between those two libraries or duplicating unsafe      ║
   ║ fixed-address pointer walking.                                          ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Aymen Sellami                                                   ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
#![no_std]

extern crate alloc;

use alloc::string::{String, ToString};

// Duplicated from `kernel/src/consts.rs`.
const USER_SPACE_START: usize = 0x10000000000;
const USER_SPACE_BOOTSTRAP_START: usize = USER_SPACE_START + 0x40000000;

const ARGC_PTR: *const usize = USER_SPACE_BOOTSTRAP_START as *const usize;
const ARGV_PTR: *const *const u8 =
    (USER_SPACE_BOOTSTRAP_START + core::mem::size_of::<usize>()) as *const *const u8;

/// Look up an environment variable by key in the process bootstrap page.
///
/// The bootstrap page stores `envp` directly after the `argv` array and its
/// NULL terminator: `[argc][argv[0..argc]][NULL][envp..][NULL]`. Each `envp`
/// entry is a `KEY=VALUE` C string. Returns the value for `key`, or `None` if
/// unset.
pub fn var(key: &str) -> Option<String> {
    unsafe {
        let argc = *ARGC_PTR;
        // envp begins after the argv array (argc entries) and its NULL terminator.
        let envp = ARGV_PTR.add(argc + 1);

        let mut index = 0;
        loop {
            let entry = *envp.add(index);
            if entry.is_null() {
                return None;
            }

            let len = c_string_len(entry);
            let bytes = core::slice::from_raw_parts(entry, len);
            if let Ok(text) = core::str::from_utf8(bytes) {
                if let Some((k, v)) = text.split_once('=') {
                    if k == key {
                        return Some(v.to_string());
                    }
                }
            }
            index += 1;
        }
    }
}

/// Implement own strlen function to avoid using the libc crate
/// for a tiny leaf crate.
unsafe fn c_string_len(ptr: *const u8) -> usize {
    let mut len = 0usize;
    while unsafe { *ptr.add(len) } != 0 {
        len += 1;
    }
    len
}
