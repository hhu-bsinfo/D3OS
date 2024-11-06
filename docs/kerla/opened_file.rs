use super::inode::{Directory, FileLike, INode};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::any::Any;
use spin::{Mutex, Once};
use syscall::return_vals::Errno;

const FD_MAX: i32 = 1024;

static NS_OPEN_FILES: Once<Arc<Mutex<OpenedFileTable>>> = Once::new();

pub fn open_file_table_init() {
    NS_OPEN_FILES.call_once(|| Arc::new(Mutex::new(OpenedFileTable::new())));
}

/// Helper function returning safe access to the open object table (oot)
pub fn get_oft() -> Arc<Mutex<OpenedFileTable>> {
    let oft = NS_OPEN_FILES.get();
    return oft.unwrap().clone();
}

/// A opened file with process-local fields.
#[derive(Clone)]
struct LocalOpenedFile {
    opened_file: Arc<OpenedFile>,
    close_on_exec: bool,
}

/// The opened file table.
#[derive(Clone)]
pub struct OpenedFileTable {
    files: Vec<Option<LocalOpenedFile>>,
    prev_fd: i32,
}

impl OpenedFileTable {
    pub fn new() -> OpenedFileTable {
        OpenedFileTable {
            files: Vec::new(),
            prev_fd: 1,
        }
    }

    /// Resolves the opened file by the file descriptor.
    pub fn get(&self, fd: usize) -> Result<&Arc<OpenedFile>, Errno> {
        match self.files.get(fd) {
            Some(Some(LocalOpenedFile { opened_file, .. })) => Ok(opened_file),
            _ => Err(Errno::EUNKN),
        }
    }

    /// Opens a file.
    pub fn open(&mut self, inode: INode, options: OpenOptions) -> Result<Fd, Errno> {
        self.alloc_fd(None).and_then(|fd| {
            self.open_with_fixed_fd(fd, Arc::new(OpenedFile { inode }), options)
                .map(|_| fd)
        })
    }


        /// Opens a file with the given file descriptor.
    ///
    /// Returns `EBADF` if the file descriptor is already in use.
    pub fn open_with_fixed_fd(
        &mut self,
        fd: Fd,
        mut opened_file: Arc<OpenedFile>,
        options: OpenOptions,
    ) -> Result<(), Errno> {
        if let INode::FileLike(file) = &opened_file.inode {
            if let Some(new_inode) = file.open(&options)? {
                // Replace inode if FileLike::open returned Some. Currently it's
                // used only for /dev/ptmx.
                opened_file = Arc::new(OpenedFile { inode: new_inode.into(), } );
            }
        }

        match self.files.get_mut(fd.as_usize()) {
            Some(Some(_)) => {
                return Err(Errno::EUNKN);
            }
            Some(entry @ None) => {
                *entry = Some(LocalOpenedFile {
                    opened_file,
                    close_on_exec: options.close_on_exec,
                });
            }
            None if fd.as_int() >= FD_MAX => {
                return Err(Errno::EUNKN);
            }
            None => {
                self.files.resize(fd.as_usize() + 1, None);
                self.files[fd.as_usize()] = Some(LocalOpenedFile {
                    opened_file,
                    close_on_exec: options.close_on_exec,
                });
            }
        }

        Ok(())
    }


    /// Allocates an unused fd. Note that this method does not any reservations
    /// for the fd: the caller must register it before unlocking this table.
    fn alloc_fd(&mut self, gte: Option<i32>) -> Result<Fd, Errno> {
        let (mut i, gte) = match gte {
            Some(gte) => (gte, gte),
            None => ((self.prev_fd + 1) % FD_MAX, 0),
        };

        while i != self.prev_fd && i >= gte {
            if matches!(self.files.get(i as usize), Some(None) | None) {
                // It looks the fd number is not in use. Open the file at that fd.
                return Ok(Fd::new(i));
            }

            i = (i + 1) % FD_MAX;
        }

        Err(Errno::EUNKN)
    }
}

/// A file descriptor.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
pub struct Fd(i32);

impl Fd {
    pub const fn new(value: i32) -> Fd {
        Fd(value)
    }

    pub const fn as_int(self) -> i32 {
        self.0
    }

    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

pub struct OpenedFile {
    pub inode: INode,
}

impl OpenedFile {
    pub fn new(inode: INode) -> OpenedFile {
        OpenedFile { inode }
    }

    pub fn as_file(&self) -> Result<&Arc<dyn FileLike>, Errno> {
        self.inode.as_file()
    }

    pub fn as_dir(&self) -> Result<&Arc<dyn Directory>, Errno> {
        self.inode.as_dir()
    }
}

pub trait Downcastable: Any + Send + Sync {
    fn as_any(&self) -> &dyn Any;
}

impl<T: Any + Send + Sync> Downcastable for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub fn downcast<S, T>(arc: &Arc<S>) -> Option<&Arc<T>>
where
    S: Downcastable + ?Sized,
    T: Send + Sync + 'static,
{
    arc.as_any().downcast_ref::<Arc<T>>()
}

#[derive(Debug, Copy, Clone)]
pub struct OpenOptions {
    pub nonblock: bool,
    pub close_on_exec: bool,
}
