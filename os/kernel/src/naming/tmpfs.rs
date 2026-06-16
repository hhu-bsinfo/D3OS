/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: tmpfs                                                           ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Temporary file system running storing everything in main memory. It     ║
   ║ supports directories, files, and named pipes.                           ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, Univ. Duesseldorf, 17.1.2026                ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use super::stat::Mode;
use super::stat::Stat;
use super::traits::{DirectoryObject, FileObject, FileSystem, NamedObject, PipeObject};
use crate::sync::wait_queue::WaitQueue;
use crate::scheduler;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt::{Debug, Formatter};
use core::result::Result;
use core::sync::atomic::AtomicBool;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering;
use core::{fmt, ptr};
use naming::shared_types::{DirEntry, FileType, OpenOptions};
use nolock::queues::mpmc;
use spin::rwlock::RwLock;
use syscall::return_vals::Errno;
use log::{info, warn};

pub struct TmpFs {
    root_dir: Arc<Dir>,
}

impl TmpFs {
    pub fn new() -> TmpFs {
        TmpFs {
            root_dir: Arc::new(Dir::new()),
        }
    }

    pub fn create_static_file(&self, path: &str, buffer: &'static [u8]) -> Result<NamedObject, Errno> {
        let mut dir = self.root_dir.as_ref();

        let (path, filename) = match path.rsplit_once("/") {
            None => ("", path),
            Some((path, name)) => (path, name),
        };

        for component in path.split("/").filter(|s| !s.is_empty()) {
            let name = component.to_string();
            let new_dir = match dir.lookup(component) {
                Ok(new_dir) => new_dir,
                Err(Errno::ENOENT) => dir.create_dir(name.as_str(), Mode::new(0)).expect("Failed to create directory"),
                Err(_) => panic!("Failed to lookup or create directory: {}", component),
            };

            dir = unsafe { (ptr::from_ref(new_dir.as_dir()?.as_ref()) as *const Dir).as_ref().unwrap() };
        }

        dir.create_static_file(filename, buffer)
    }
}

impl FileSystem for TmpFs {
    fn root_dir(&self) -> Arc<dyn DirectoryObject> {
        self.root_dir.clone()
    }
}

enum TmpFsINode {
    File(Arc<dyn FileObject>),
    Pipe(Arc<dyn PipeObject>),
    Directory(Arc<Dir>),
}

struct DirInner {
    files: Vec<(String, TmpFsINode)>,
    stat: Stat,
}

pub struct Dir(RwLock<DirInner>);

impl Dir {
    pub fn new() -> Dir {
        Dir(RwLock::new(DirInner {
            files: Vec::new(),
            stat: Stat {
                mode: Mode::new(0),
                ..Stat::zeroed()
            },
        }))
    }

    pub fn create_static_file(&self, name: &str, buffer: &'static [u8]) -> Result<NamedObject, Errno> {
        let mut dir_lock = self.0.write();

        // Check if the file already exists in the directory
        if dir_lock.files.iter().any(|(file_name, _)| file_name == name) {
            return Err(Errno::EEXIST); // Return an error if the file exists
        }

        // Create a new file and add it to the directory
        let inode = Arc::new(StaticFile::new(buffer));
        dir_lock.files.push((name.to_string(), TmpFsINode::File(inode.clone())));

        // Return the created file as a NamedObject
        Ok((inode as Arc<dyn FileObject>).into())
    }
}

impl DirectoryObject for Dir {
    // check if an object with the given name exists in the directory
    fn lookup(&self, name: &str) -> Result<NamedObject, Errno> {
        let guard = self.0.read(); // Lock the mutex to access the inner data
        if let Some((_, tmpfs_inode)) = guard.files.iter().find(|(file_name, _)| file_name == name) {
            // Match on the TmpFsINode type
            match tmpfs_inode {
                TmpFsINode::File(file) => Ok(file.clone().into()), // Clone and convert to NamedObject
                TmpFsINode::Pipe(pipe) => Ok(pipe.clone().into()), // Clone and convert to NamedObject
                TmpFsINode::Directory(dir) => Ok((dir.clone() as Arc<dyn DirectoryObject>).into()), // Clone and cast directory
            }
        } else {
            Err(Errno::ENOENT) // Return error if the file is not found
        }
    }

    fn create_pipe(&self, name: &str, _mode: Mode) -> Result<NamedObject, Errno> {
        let mut dir_lock = self.0.write();

        // Check if the pipe already exists in the directory
        if dir_lock.files.iter().any(|(file_name, _)| file_name == name) {
            return Err(Errno::EEXIST); // Return an error if the file exists
        }

        // Create a new pipe and add it to the directory
        let inode = Arc::new(Pipe::new());
        dir_lock.files.push((name.to_string(), TmpFsINode::Pipe(inode.clone())));

        // Return the created file as a NamedObject
        Ok((inode as Arc<dyn PipeObject>).into())
    }

    fn create_file(&self, name: &str, _mode: Mode) -> Result<NamedObject, Errno> {
        let mut dir_lock = self.0.write();

        // Check if the file already exists in the directory
        if dir_lock.files.iter().any(|(file_name, _)| file_name == name) {
            return Err(Errno::EEXIST); // Return an error if the file exists
        }

        // Create a new file and add it to the directory
        let inode = Arc::new(File::new());
        dir_lock.files.push((name.to_string(), TmpFsINode::File(inode.clone())));

        // Return the created file as a NamedObject
        Ok((inode as Arc<dyn FileObject>).into())
    }

    fn create_dir(&self, name: &str, _mode: Mode) -> Result<NamedObject, Errno> {
        let mut dir_lock = self.0.write();

        // Check if a file or directory with the same name already exists
        if dir_lock.files.iter().any(|(file_name, _)| file_name == name) {
            return Err(Errno::EEXIST); // Return an error if the name exists
        }

        // Create a new directory and add it to the directory's entries
        let inode = Arc::new(Dir::new());
        dir_lock.files.push((name.to_string(), TmpFsINode::Directory(inode.clone())));

        // Return the created directory as a NamedObject
        Ok((inode as Arc<dyn DirectoryObject>).into())
    }

    fn stat(&self) -> Result<Stat, Errno> {
        Ok(self.0.read().stat)
    }

    fn readdir(&self, index: usize) -> Result<Option<DirEntry>, Errno> {
        let dir_lock = self.0.read();
        let (name, inode) = match dir_lock.files.get(index) {
            Some(entry) => entry,
            None => {
                return Ok(None);
            }
        };

        let entry = match inode {
            TmpFsINode::Directory(_dir) => DirEntry {
                file_type: FileType::Directory,
                name: name.clone(),
            },
            TmpFsINode::File(_file) => DirEntry {
                file_type: FileType::Regular,
                name: name.clone(),
            },
            TmpFsINode::Pipe(_pipe) => DirEntry {
                file_type: FileType::NamedPipe,
                name: name.clone(),
            },
        };
        Ok(Some(entry))
    }
}

impl fmt::Debug for Dir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TmpFsDir").finish()
    }
}

struct File {
    data: RwLock<Vec<u8>>,
    stat: RwLock<Stat>,
}

impl File {
    pub fn new() -> File {
        File {
            data: RwLock::new(Vec::new()),
            stat: RwLock::new(Stat {
                mode: Mode::new(0),
                ..Stat::zeroed()
            }),
        }
    }
}

impl FileObject for File {
    fn stat(&self) -> Result<Stat, Errno> {
        Ok(*self.stat.read())
    }

    fn read(&self, buf: &mut [u8], offset: usize, _options: OpenOptions) -> Result<usize, Errno> {
        let data = self.data.write();
        if offset > data.len() {
            return Ok(0);
        }

        let len = if data.len() - offset < buf.len() { data.len() - offset } else { buf.len() };

        buf[0..len].clone_from_slice(&data[offset..offset + len]);
        Ok(len)
    }

    fn write(&self, buf: &[u8], offset: usize, _options: OpenOptions) -> Result<usize, Errno> {
        let mut data = self.data.write();

        if offset + buf.len() > data.len() {
            let mut stat = self.stat.write();
            stat.size = offset + buf.len();

            data.resize(stat.size, 0);
        }

        data[offset..offset + buf.len()].clone_from_slice(buf);
        Ok(buf.len())
    }
}

impl Debug for File {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("TmpFsFile").finish()
    }
}

struct StaticFile {
    data: &'static [u8],
    stat: Stat,
}

impl StaticFile {
    pub fn new(data: &'static [u8]) -> StaticFile {
        StaticFile {
            data,
            stat: Stat {
                size: data.len(),
                ..Stat::zeroed()
            },
        }
    }
}

impl FileObject for StaticFile {
    fn stat(&self) -> Result<Stat, Errno> {
        Ok(self.stat)
    }

    fn read(&self, buf: &mut [u8], offset: usize, _options: OpenOptions) -> Result<usize, Errno> {
        if offset > self.data.len() {
            return Ok(0);
        }

        let len = if self.data.len() - offset < buf.len() {
            self.data.len() - offset
        } else {
            buf.len()
        };

        buf[0..len].clone_from_slice(&self.data[offset..offset + len]);
        Ok(len)
    }

    fn write(&self, _buf: &[u8], _offset: usize, _options: OpenOptions) -> Result<usize, Errno> {
        Err(Errno::ERDONLY)
    }
}

impl Debug for StaticFile {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("TmpFsStaticFile").finish()
    }
}

const PIPE_SIZE: usize = 0x1000;

struct PipeQueue {
    rx: mpmc::bounded::scq::Receiver<u8>,
    wx: mpmc::bounded::scq::Sender<u8>,
}

struct Pipe {
    stat: RwLock<Stat>,
    pq: RwLock<PipeQueue>,
    count: AtomicUsize,      // number of bytes currently in the pipe
    open_wq: WaitQueue,      // block open calls as needed by POSIX
    open_epoch: AtomicUsize, // avoid lost wakeups for open calls
    open_close_mutex: spin::Mutex<()>,  // protects critical sections in open/closer
    rx_wq: WaitQueue,        // readers block when pipe is empty
    wx_wq: WaitQueue,        // writers block when pipe is full
    has_reader: AtomicBool,  // true if opened for reading
    has_writer: AtomicBool,  // true if opened for writing
}

impl Pipe {
    pub fn new() -> Pipe {
        let (rx, wx) = mpmc::bounded::scq::queue(PIPE_SIZE);
        Self {
            stat: RwLock::new(Stat {
                mode: Mode::new(0),
                ..Stat::zeroed()
            }),
            pq: RwLock::new(PipeQueue { rx, wx }),

            // data plane
            count: AtomicUsize::new(0),
            rx_wq: WaitQueue::new(),
            wx_wq: WaitQueue::new(),

            // open rendezvous (fix lost wakeups / open race)
            open_wq: WaitQueue::new(),
            open_epoch: AtomicUsize::new(0),
            open_close_mutex: spin::Mutex::new(()),

            // single-reader/single-writer enforcement
            has_reader: AtomicBool::new(false),
            has_writer: AtomicBool::new(false),
        }
    }

    #[inline]
    fn wait_open<F: Fn() -> bool>(&self, cond: F, why: &'static str) {
        loop {
            if cond() {
                return;
            }
            // Sleep until either the condition becomes true OR the epoch changed.
            // Epoch change means "some open/close transition happened, re-check".
           let e = self.epoch();
           self.open_wq.wait(|| cond() || self.epoch() != e, why);
            // loop to re-check; handles spurious wakes and epoch-only wakes.
        }
    }

    #[inline]
    fn has_data(&self) -> bool {
        self.count.load(Ordering::SeqCst) > 0
    }

    #[inline]
    fn has_space(&self) -> bool {
        self.count.load(Ordering::SeqCst) < PIPE_SIZE
    }

    #[inline]
    fn has_reader(&self) -> bool {
        self.has_reader.load(Ordering::SeqCst)
    }

    #[inline]
    fn has_writer(&self) -> bool {
        self.has_writer.load(Ordering::SeqCst)
    }

    // required to check if we have a lost wakeup for open calls
    #[inline]
    fn epoch(&self) -> usize {
        self.open_epoch.load(Ordering::SeqCst)
    }

    // required to check if we have a lost wakeup for open calls
    #[inline]
    fn bump_epoch_and_wake_open(&self) {
        self.open_epoch.fetch_add(1, Ordering::SeqCst);
        self.open_wq.notify_all();
    }
}

impl PipeObject for Pipe {
    fn open(&self, flags: OpenOptions) -> Result<usize, Errno> {
        let (pid, tid) = scheduler().current_ids();

        match flags {
            OpenOptions::READONLY => {
                let _g = self.open_close_mutex.lock();
                //info!("PipeObject::open: READONLY, handle = {}, pid = {}, tid = {}, name = '{}'", handle, pid, tid, name);

                if self.has_reader.load(Ordering::SeqCst) {
                    return Err(Errno::EBUSY);
                }

                // publish reader-present
                self.has_reader.store(true, Ordering::SeqCst);
                self.bump_epoch_and_wake_open();
                drop(_g);

                // block until a writer is present.
                self.wait_open(|| self.has_writer.load(Ordering::SeqCst), "open: reader waiting for writer");
                Ok(0)
            }

            OpenOptions::WRITEONLY => {
                let _g = self.open_close_mutex.lock();
                //info!("PipeObject::open: WRITEONLY, handle = {}, pid = {}, tid = {}, name = '{}'", handle, pid, tid, name);

                if self.has_writer.load(Ordering::SeqCst) {
                    return Err(Errno::EBUSY);
                }

                // publish writer-present
                self.has_writer.store(true, Ordering::SeqCst);
                self.bump_epoch_and_wake_open();
                drop(_g);

                // block until a reader is present
                self.wait_open(|| self.has_reader.load(Ordering::SeqCst), "open: writer waiting for reader");
                Ok(0)
            }

            _ => Err(Errno::EINVAL),
        }
    }

    fn stat(&self) -> Result<Stat, Errno> {
        Ok(*self.stat.read())
    }

    /// Read from pipe buffer, `offset` is ignored
    fn read(&self, buf: &mut [u8], _offset: usize, options: OpenOptions) -> Result<usize, Errno> {
        // Debug output
        let (pid, tid) = scheduler().current_ids();
        //info!("read: pid={}, tid={}", pid, tid);

        // check if pipe was opened for reading
        if options == OpenOptions::WRITEONLY {
            return Err(Errno::EBADF);
        }

        // buf has len = 0 ?
        if buf.len() == 0 {
            return Ok(0);
        }

        // Block until data is available or writer has gone
        self.rx_wq.wait(|| self.has_data() || !self.has_writer(), "read: blocks");

        // EOF if no writer is present and no data available
        if !self.has_data() && !self.has_writer() {
            return Ok(0);
        }

        // From here we read data
        // We have data but the writer might have gone or leaves concurrently

        let total_to_read = buf.len();
        let mut total_read = 0;
        let pq = self.pq.read();
        loop {
            // Are we done?
            if total_read >= total_to_read {
                break;
            }

            // Read one byte
            match pq.rx.try_dequeue() {
                Ok(byte) => {
                    // We consumed a byte
                    self.count.fetch_sub(1, Ordering::SeqCst);
                    buf[total_read] = byte;
                    total_read += 1;
                }
                Err(_) => {
                    // We consumed all available data but need more
                    // We block until more data is available or the writer has gone (-> EOF)
                    self.rx_wq.wait(|| self.has_data() || !self.has_writer(), "read: blocks");
                    if !self.has_data() {
                        break;
                    }
                }
            }
        }

        // If we read at least one byte we freed space
        // -> wake potentially blocked writer
        if total_read > 0 {
            self.wx_wq.notify_one();
        }

        Ok(total_read)
    }

    /// Write to pipe buffer, `offset` is ignored
    fn write(&self, buf: &[u8], _offset: usize, options: OpenOptions) -> Result<usize, Errno> {
        // Debug output
        let (pid, tid) = scheduler().current_ids();
        //info!("write: pid={}, tid={}", pid, tid);

        // check if pipe was opened for reading
        if options == OpenOptions::READONLY {
            return Err(Errno::EBADF);
        }

        // buf has len = 0 ?
        if buf.len() == 0 {
            return Ok(0);
        }

        // Block until space is available or reader has gone
        self.wx_wq.wait(|| self.has_space() || !self.has_reader(), "write: blocks");

        // EOF if no writer is present and no data available
        if !self.has_reader() {
            return Err(Errno::EPIPE);
        }

        // From here we write data
        // We have space but the reader might leave concurrently
        let total_to_write: usize = buf.len();
        let mut total_written = 0;
        let pq = self.pq.read();
        loop {
            // Are we done?
            if total_written >= total_to_write {
                break;
            }

            // Write one byte
            match pq.wx.try_enqueue(buf[total_written]) {
                Ok(byte) => {
                    // We wrote a byte
                    self.count.fetch_add(1, Ordering::SeqCst);
                    total_written += 1;
                }
                Err(_) => {
                    // We consumed all available space but need more
                    // We block until more space is available or the reader has gone (-> EOF)
                    self.wx_wq.wait(|| self.has_space() || !self.has_reader(), "write: blocks");
                    if !self.has_reader() {
                        return Err(Errno::EPIPE);
                    }
                }
            }
        }

        // If we wrote at least one byte we wake up potentially blocked reader
        if total_written > 0 {
            info!("PipeObject::write: done, total_written={}, notify_one, pid={}, tid={}", total_written, pid, tid);
            self.rx_wq.notify_one();
        }
        Ok(total_written)
    }

    fn close(&self, flags: OpenOptions) {
        let (pid, tid) = scheduler().current_ids();
        let _g = self.open_close_mutex.lock();

        //info!("PipeObject::close: handle = {}, flags={:?}, pid={}, tid={}", fh, flags, pid, tid);

        match flags {
            OpenOptions::READONLY => {
                self.has_reader.store(false, Ordering::SeqCst);
                self.bump_epoch_and_wake_open(); // wake open waiters
                self.wx_wq.notify_all(); // writers blocked on full/space or EPIPE checks
            }
            OpenOptions::WRITEONLY => {
                self.has_writer.store(false, Ordering::SeqCst);
                self.bump_epoch_and_wake_open(); // wake open waiters
                self.rx_wq.notify_all(); // readers blocked on empty/EOF checks
            }
            _ => {}
        }

        // If we have no readers and no writers, we can reset the pipe buffer to avoid keeping data around indefinitely.
        if !self.has_reader.load(Ordering::SeqCst) && !self.has_writer.load(Ordering::SeqCst) {
            //info!("PipeObject::close: resetting pipe buffer, pid={}, tid={}", pid, tid);
            let (rx, wx) = mpmc::bounded::scq::queue(PIPE_SIZE);
            let mut pq = self.pq.write();
            pq.rx = rx;
            pq.wx = wx;
            self.count.store(0, Ordering::SeqCst);
        }
    }
}

impl Debug for Pipe {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("NamedPipe").finish()
    }
}
