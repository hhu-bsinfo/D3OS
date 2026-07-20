use core::{ptr, sync::atomic::{AtomicU8, AtomicU64, Ordering}};

use alloc::boxed::Box;
use syscall::{SystemCall, return_vals::SyscallResult, syscall};
use uuid::Uuid;

// Schnittstelle für Interprozesskommunikation

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
    message: IpcMessage,
    pid: Uuid,
    tid: usize,
}

#[derive(Debug, Copy, Clone)]
pub enum IpcMessage {
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

impl IpcMessage {
    pub fn from_arr(arr: &[usize]) -> IpcMessage {
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

pub fn send(pid: Uuid, tid: usize, message: &IpcMessage) -> SyscallResult {
    
    let (pid1, pid2) = pid.as_u64_pair();
    let message_ptr = ptr::from_ref(message);

    syscall(SystemCall::IpcSend, &[pid1 as usize, pid2 as usize, tid, message_ptr as usize])
}

pub fn receive_next() -> Option<IpcMessage> {
    /*
    let handles = unsafe { &*(USER_SPACE_MAILBOX_START as *const IpcQueueHandles) };
    handles.receiver.lock().dequeue()
     */
    None
}

pub fn receive_next_blocking(timeout_ms: usize) -> Result<IpcMessage, syscall::return_vals::Errno> {
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