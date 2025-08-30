/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: tmpfs                                                           ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Temporary file system running storing everything in main memory. It     ║
   ║ supports directories, files, and named pipes.                           ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, Univ. Duesseldorf, 28.8.2025                ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use super::stat::Mode;
use super::stat::Stat;
use super::traits::{DirectoryObject, FileObject, FileSystem, NamedObject, PipeObject};
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt::{Debug, Formatter};
use core::result::Result;
use core::{fmt, ptr};
use log::info;
use naming::shared_types::{DirEntry, FileType, OpenOptions};
use nolock::queues::mpmc;
use nolock::queues::mpmc::bounded;
use spin::rwlock::RwLock;
use syscall::return_vals::Errno;

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

struct Pipe {
    buffer: (mpmc::bounded::scq::Receiver<u8>, mpmc::bounded::scq::Sender<u8>),
    stat: RwLock<Stat>,
}

impl Pipe {
    pub fn new() -> Pipe {
        Pipe {
            buffer: mpmc::bounded::scq::queue(PIPE_SIZE),
            stat: RwLock::new(Stat {
                mode: Mode::new(0),
                ..Stat::zeroed()
            }),
        }
    }
}

impl PipeObject for Pipe {
    fn stat(&self) -> Result<Stat, Errno> {
        Ok(*self.stat.read())
    }

    /// Read from pipe buffer, `offset` is ignored
    fn read(&self, buf: &mut [u8], _offset: usize, _options: OpenOptions) -> Result<usize, Errno> {
        let mut total_read = 0;
        loop {
            match self.buffer.0.try_dequeue() {
                Ok(byte) => {
                    if total_read < buf.len() {
                        buf[total_read] = byte;
                        total_read += 1;
                    } else {
                        break; // Buffer is full
                    }
                }
                Err(_) => {
                    if total_read == 0 {
                        return Err(Errno::EAGAIN); // Buffer empty, no data read
                    } else {
                        break; // Buffer empty, some data read
                    }
                } // No more data to read
            }
        }
        Ok(total_read)
    }



    /// Write to pipe buffer, `offset` is ignored
    fn write(&self, buf: &[u8], _offset: usize, _options: OpenOptions) -> Result<usize, Errno> {
        let mut total_written = 0;
        for byte in buf {
            match self.buffer.1.try_enqueue(*byte) {
                Ok(()) => {
                    total_written += 1;
                },
                Err(_) => {
                    if total_written == 0 {
                        return Err(Errno::EAGAIN); // Buffer full, no data written
                    } else {
                        break; // Buffer full, some data written
                    }
                }
            }
        }
        //        info!("   written: {}", total_written);
        Ok(total_written)
    }
}

impl Debug for Pipe {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("NamedPipe").finish()
    }
}
