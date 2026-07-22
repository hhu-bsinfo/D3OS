use core::{any::Any, ptr::{self, NonNull}, sync::atomic::{AtomicBool, AtomicI32, AtomicU8, AtomicU16, AtomicU64, AtomicUsize, Ordering}};

use alloc::{boxed::Box, vec::Vec};
use syscall::{SystemCall, return_vals::SyscallResult, syscall};
use uuid::Uuid;

// Schnittstelle für Interprozesskommunikation

const MESSAGE_SIZE: usize = 0x100;
const MAX_NUM_MESSAGES: usize = 8;
const MESSAGE_RINGBUFFER_SIZE: usize = MESSAGE_SIZE * MAX_NUM_MESSAGES;
const MESSAGE_RINGBUFFER_PTR: *mut u8 = USER_SPACE_MAILBOX_START as _;

pub struct IpcQueue {
    writer_idx: AtomicU16,
    reader_idx: AtomicU16,
    mailbox: [AtomicI32; MAX_NUM_MESSAGES]
}

impl IpcQueue {
    
    pub fn send(&self, data: &[u8]) -> bool {

        // 1. check if there is space in the queue
        if self.space_left() <= 0 {
            return false;
        }

        // 2. increment writer idx (wrap around)
        let writer_idx = self.writer_idx().fetch_add(1, Ordering::SeqCst) as usize;
        self.writer_idx().compare_exchange(MAX_NUM_MESSAGES as u16, 0, Ordering::SeqCst, Ordering::SeqCst);
        
        // 3. check if space is free? -> reset writer idx if true
        if self.mailbox[writer_idx].load(Ordering::SeqCst) != 0 {
            self.writer_idx().store(writer_idx as u16, Ordering::SeqCst);
            return false;
        }

        // 4. write message into memory
        let msg_size = core::cmp::min(data.len(), MESSAGE_SIZE);
        self.mailbox[writer_idx].store(-1, Ordering::SeqCst);
        unsafe {
            let message_ptr = MESSAGE_RINGBUFFER_PTR.add(writer_idx);
            core::ptr::copy_nonoverlapping(data.as_ptr(), message_ptr, msg_size);
        }
        self.mailbox[writer_idx].store(msg_size as i32, Ordering::SeqCst);
        true
    }

    pub fn receive(&self) -> Option<Vec<u8>> {

        // 1. check for any messages
        if self.messages_left() <= 0 {
            return None;
        }

        // 2. increment reader idx (wrap around)
        let writer_idx = self.writer_idx().load(Ordering::SeqCst) as usize % MAX_NUM_MESSAGES;
        let reader_idx = self.reader_idx().fetch_add(1, Ordering::SeqCst) as usize % MAX_NUM_MESSAGES;
        self.reader_idx().compare_exchange(MAX_NUM_MESSAGES as u16, 0, Ordering::SeqCst, Ordering::SeqCst);

        // 3. compare indices
        let msg_size = self.mailbox[writer_idx].load(Ordering::SeqCst) as usize;
        if writer_idx != reader_idx {
            // 4a. read message
            unsafe {
                let message_ptr = MESSAGE_RINGBUFFER_PTR.add(reader_idx);
                let mut v = Vec::with_capacity(msg_size);
                v.copy_from_slice(alloc::slice::from_raw_parts(message_ptr, msg_size));
            }
            self.mailbox[writer_idx].store(0, Ordering::SeqCst);
        }


        let atomic = self.get_message(reader_idx);
        if writer_idx != reader_idx {
            Some(atomic.swap(0, Ordering::SeqCst))
        } else {
            let msg = atomic.load(Ordering::SeqCst);
            if msg != 0 {
                Some(msg)
            } else {
                self.reader_idx().store(reader_idx, Ordering::SeqCst);
                None
            }
        }
    }

    pub fn writer_idx(&self) -> &AtomicU16 {
        &self.writer_idx
    }

    pub fn reader_idx(&self) -> &AtomicU16 {
        &self.reader_idx
    }

    pub fn messages_left(&self) -> u16 {
        let writer_idx = self.writer_idx.load(Ordering::SeqCst);
        let reader_idx = self.reader_idx.load(Ordering::SeqCst);
        writer_idx.wrapping_sub(reader_idx)
    }

    pub fn space_left(&self) -> u16 {
        let writer_idx = self.writer_idx.load(Ordering::SeqCst);
        let reader_idx = self.reader_idx.load(Ordering::SeqCst);
        reader_idx.wrapping_sub(writer_idx)
    }

    pub fn new() -> Self {
        Self { writer_idx: AtomicU16::new(0), reader_idx: AtomicU16::new(0), mailbox: [0; _] }
    }
}

impl Default for IpcQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct IpcQueueEntry {
    data: [u8; MESSAGE_SIZE],
}

#[derive(Debug)]
pub struct IpcSmallMessageRingbuffer {
    writer_idx: AtomicU8,
    reader_idx: AtomicU8,
    buffer: [AtomicU64; 256]
}

impl IpcSmallMessageRingbuffer {
    pub fn send(&self, message: u64) -> Result<(), ()> {
        let writer_idx = self.writer_idx().fetch_add(1, Ordering::SeqCst);
        let atomic = self.get_message(writer_idx);
        let res = atomic.compare_exchange(0, message, Ordering::SeqCst, Ordering::SeqCst);
        if res.is_ok() {
            Ok(())
        } else {
            self.writer_idx().store(writer_idx, Ordering::SeqCst);
            Err(())
        }
    }
    pub fn receive(&self) -> Option<u64> {
        let writer_idx = self.writer_idx().load(Ordering::SeqCst);
        let reader_idx = self.reader_idx().fetch_add(1, Ordering::SeqCst);
        let atomic = self.get_message(reader_idx);
        if writer_idx != reader_idx {
            Some(atomic.swap(0, Ordering::SeqCst))
        } else {
            let msg = atomic.load(Ordering::SeqCst);
            if msg != 0 {
                Some(msg)
            } else {
                self.reader_idx().store(reader_idx, Ordering::SeqCst);
                None
            }
        }
    }


    pub fn writer_idx(&self) -> &AtomicU8 {
        &self.writer_idx
    }

    pub fn reader_idx(&self) -> &AtomicU8 {
        &self.reader_idx
    }

    pub fn get_message(&self, idx: u8) -> &AtomicU64 {
        &self.buffer[idx as usize]
    }

    pub fn messages_left(&self) -> u8 {
        let writer_idx = self.writer_idx.load(Ordering::SeqCst);
        let reader_idx = self.reader_idx.load(Ordering::SeqCst);
        writer_idx.wrapping_sub(reader_idx)
    }

    pub fn space_left(&self) -> u8 {
        let writer_idx = self.writer_idx.load(Ordering::SeqCst);
        let reader_idx = self.reader_idx.load(Ordering::SeqCst);
        reader_idx.wrapping_sub(writer_idx)
    }
    
    fn new() -> Self {
        Self { writer_idx: AtomicU8::new(0), reader_idx: AtomicU8::new(0), buffer: [const { AtomicU64::new(0) }; _] }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct IpcMessageWrapper {
    message: IpcMessage2,
    pid: Uuid,
    tid: usize,
}

#[derive(Debug, Copy, Clone)]
pub enum IpcMessage2 {
    Small {
        a: u64,
    },
    Simple {
        a: usize,
        b: usize,
        c: usize,
        d: usize,
    },
}

impl IpcMessage2 {
    pub fn from_arr(arr: &[usize]) -> IpcMessage2 {
        let a = arr.get(0).copied().unwrap_or_default();
        let b = arr.get(1).copied().unwrap_or_default();
        let c = arr.get(2).copied().unwrap_or_default();
        let d = arr.get(3).copied().unwrap_or_default();
        Self::Simple { a, b, c, d }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct IpcMailboxDistributer {
    pub small_msg_ringbuffer: *const IpcSmallMessageRingbuffer
}

const USER_SPACE_MAILBOX_START: usize = 0x10100000000;

// TODO dont store messages themselves in the queue, instead store pointers to message ==> avoid one copy (currently 2-copy)
// TODO far future: use rdma to allow zero-copy ==> store large data structures in specific pages; when sending the message, unmap the page and remap into receiver

pub fn init_queue() {
    /*
    let (receiver, sender) = jiffy::queue();
    let handles = Box::new(IpcQueueHandles { sender, receiver: Mutex::new(receiver), num_messages: AtomicU64::new(0) });
    let handles_ptr = ptr::from_ref(Box::as_ref(&handles)); 
    let handles_ptr_dest = USER_SPACE_MAILBOX_START as *mut *const IpcQueueHandles;
    unsafe { handles_ptr_dest.write(handles_ptr); }
    */


    let mailbox_start = USER_SPACE_MAILBOX_START as *const u8;

    // use box to allocate the struct on the heap
    let boxed_small_msg_rb = Box::new(IpcSmallMessageRingbuffer::new());

    // leak the box so it doesnt get deleted
    let box_ref = Box::leak(boxed_small_msg_rb);

    let mailbox_start_ptr = mailbox_start as *mut &mut IpcSmallMessageRingbuffer;
    unsafe { mailbox_start_ptr.write(box_ref) };
    

    /*

    // manually write struct to memory
    let small_msg_rb_struct_start = USER_SPACE_MAILBOX_START + 0x1000;
    {
        let buffer_start = small_msg_rb_struct_start as *mut AtomicU64;
        for i in 0..256 {
            unsafe { buffer_start.add(i).write(AtomicU64::new(i as u64)); }
        }
        let writer_addr = (small_msg_rb_struct_start + 0x800) as *mut AtomicU8;
        unsafe { writer_addr.write(AtomicU8::new(0)) };
        let reader_addr = (small_msg_rb_struct_start + 0x801) as *mut AtomicU8;
        unsafe { reader_addr.write(AtomicU8::new(0)) };
    }

    let small_msg_ringbuffer = unsafe { ((USER_SPACE_MAILBOX_START + 0x1000) as *mut IpcSmallMessageRingbuffer).read() };
    
    let buffer = &small_msg_ringbuffer.buffer;
    buffer[3].store(123, Ordering::SeqCst);
    unsafe { small_msg_ringbuffer.get_entry(3).read().store(321, Ordering::SeqCst) };

    //.
    // Zugriff auf den array funktioniert irgendwie nicht -> array wird kopiert?
    // änderungen an den atomics werden nicht durchgeschrieben --> man manipuliert nur eine kopie?
    // ==> pointer / reference speichern um wirklichen atomic im speicher zu bearbeiten
    
    let distributer = IpcMailboxDistributer {
        small_msg_ringbuffer: (small_msg_rb_struct_start as *const _)
    };
    unsafe { (USER_SPACE_MAILBOX_START as *mut IpcMailboxDistributer).write(distributer) };
    */
}

pub fn send_small(pid: Uuid, tid: usize, message: u64) -> SyscallResult {
    let (pid1, pid2) = pid.as_u64_pair();
    syscall(SystemCall::IpcSendSmall, &[pid1 as usize, pid2 as usize, tid, message as usize])
}

pub fn receive_small() -> Option<u64> {
    //let distributer = unsafe { (USER_SPACE_MAILBOX_START as *mut IpcMailboxDistributer).read() };
    let small_msg_ringbuffer = unsafe { (USER_SPACE_MAILBOX_START as *const &mut IpcSmallMessageRingbuffer).read() };
    small_msg_ringbuffer.receive()
}

pub fn send(pid: Uuid, tid: usize, message: &IpcMessage2) -> SyscallResult {
    
    let (pid1, pid2) = pid.as_u64_pair();
    let message_ptr = ptr::from_ref(message);

    syscall(SystemCall::IpcSend, &[pid1 as usize, pid2 as usize, tid, message_ptr as usize])
}

pub fn receive_next() -> Option<IpcMessage2> {
    /*
    let handles = unsafe { &*(USER_SPACE_MAILBOX_START as *const IpcQueueHandles) };
    handles.receiver.lock().dequeue()
     */
    None
}

pub fn receive_next_blocking(timeout_ms: usize) -> Result<IpcMessage2, syscall::return_vals::Errno> {
    /*
    match syscall(SystemCall::IpcReceiveBlocking, &[timeout_ms]) {
        Ok(_) => {
            let handles = unsafe { &*(USER_SPACE_MAILBOX_START as *const IpcQueueHandles) };
            handles.receiver.lock().dequeue().ok_or(syscall::return_vals::Errno::EEXIST)
        },
        Err(e) => Err(e),
    }
     */
    Err((-1).into())
}