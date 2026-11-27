use alloc::string::String;
use core::{
    ptr::NonNull,
    sync::atomic::{AtomicU8, AtomicU32, AtomicU64, Ordering::SeqCst},
};

use concurrent::{shm, thread};
use syscall::{SystemCall, syscall};

use crate::{DecodedKey, wire};

const NAME: &str = "d3os-imed-pinyin-v1";
const READY: u64 = 0x4453_494d_4544_0001;
const REPLY_CAPACITY: usize = 192;
const REPLY_TIMEOUT_MS: usize = 80;

#[repr(C)]
struct Shared {
    ready: AtomicU64,
    next_serial: AtomicU32,
    request: AtomicU64,
    response_serial: AtomicU32,
    response_flags: AtomicU8,
    commit_len: AtomicU8,
    preview_len: AtomicU8,
    response: [AtomicU8; REPLY_CAPACITY],
}

/// Single-request mailbox. The active keyboard reader is the client and imed
/// is the sole responder. Fields are atomic so a timed-out client can leave
/// while the daemon finishes its reply.
pub struct Mailbox(NonNull<Shared>);

pub struct Request {
    serial: u32,
    pub flags: u8,
    pub key: Option<DecodedKey>,
}

impl Mailbox {
    pub fn create() -> Option<Self> {
        let mailbox = Self::attach(true)?;
        let shared = mailbox.shared();
        shared.ready.store(0, SeqCst);
        shared.next_serial.store(0, SeqCst);
        shared.request.store(0, SeqCst);
        shared.response_serial.store(0, SeqCst);
        shared.ready.store(READY, SeqCst);
        Some(mailbox)
    }

    pub fn open() -> Option<Self> {
        let mailbox = Self::attach(false)?;
        (mailbox.shared().ready.load(SeqCst) == READY).then_some(mailbox)
    }

    fn attach(create: bool) -> Option<Self> {
        let id = shm::shm_open(NAME, size_of::<Shared>(), create).ok()?;
        let pointer = shm::shm_attach(id, false).ok()?.cast::<Shared>();
        Some(Self(NonNull::new(pointer)?))
    }

    fn shared(&self) -> &Shared {
        // ShmAttach maps the entire region at a page-aligned address. Every
        // field admits any integer bit pattern and is accessed atomically.
        unsafe { self.0.as_ref() }
    }

    pub fn receive(&self) -> Option<Request> {
        let packet = self.shared().request.load(SeqCst);
        (packet != 0).then(|| Request {
            serial: (packet >> 32) as u32,
            flags: (packet >> 24) as u8,
            key: wire::decode_key(packet as u32 & 0xff_ffff),
        })
    }

    pub fn reply(&self, request: Request, flags: u8, commit: &str, preview: &str) {
        let shared = self.shared();
        shared.response_serial.store(0, SeqCst);
        if commit.len() + preview.len() <= REPLY_CAPACITY {
            shared.response_flags.store(flags, SeqCst);
            shared.commit_len.store(commit.len() as u8, SeqCst);
            shared.preview_len.store(preview.len() as u8, SeqCst);
            for (slot, byte) in shared.response.iter().zip(commit.bytes().chain(preview.bytes())) {
                slot.store(byte, SeqCst);
            }
            shared.request.store(0, SeqCst);
            shared.response_serial.store(request.serial, SeqCst);
        } else {
            shared.request.store(0, SeqCst);
        }
    }

    pub fn exchange(&self, flags: u8, key: Option<DecodedKey>) -> Option<(u8, String, String)> {
        let shared = self.shared();
        let serial = loop {
            let serial = shared.next_serial.fetch_add(1, SeqCst).wrapping_add(1);
            if serial != 0 {
                break serial;
            }
        };
        let key = u64::from(key.map_or(0, wire::encode_key));
        let packet = u64::from(serial) << 32 | u64::from(flags) << 24 | key;
        let start = syscall(SystemCall::GetSystemTime, &[]).ok()?;
        shared.request.compare_exchange(0, packet, SeqCst, SeqCst).ok()?;
        loop {
            if shared.response_serial.load(SeqCst) == serial {
                let flags = shared.response_flags.load(SeqCst);
                let commit_len = usize::from(shared.commit_len.load(SeqCst));
                let length = commit_len + usize::from(shared.preview_len.load(SeqCst));
                let mut bytes = [0; REPLY_CAPACITY];
                for (byte, slot) in bytes.get_mut(..length)?.iter_mut().zip(&shared.response) {
                    *byte = slot.load(SeqCst);
                }
                if shared.response_serial.load(SeqCst) != serial {
                    return None;
                }
                let committed = core::str::from_utf8(&bytes[..commit_len]).ok()?;
                let preview = core::str::from_utf8(&bytes[commit_len..length]).ok()?;
                return Some((flags, String::from(committed), String::from(preview)));
            }
            if syscall(SystemCall::GetSystemTime, &[]).ok()?.saturating_sub(start) >= REPLY_TIMEOUT_MS {
                return None;
            }
            thread::sleep(1);
        }
    }
}

impl Drop for Mailbox {
    fn drop(&mut self) {
        let _ = shm::shm_detach(self.0.as_ptr().cast());
    }
}
