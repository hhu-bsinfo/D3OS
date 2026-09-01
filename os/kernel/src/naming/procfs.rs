/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: procfs                                                          ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Public functions for the procFS. Existing file objects:                 ║
   ║   - ProcPidDir        directory for a pid                               ║
   ║   - ProcStatusFile    information about one process                     ║
   ║   - ProcPsFile        information about all active processes            ║
   ║   - ProcTicksFile     contains the current ticks of the system          ║
   ║   - ProcMemInfoFile   information about global memory usage             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Alexander Lopushkov, Univ. Duesseldorf, 2.1.2026                ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use alloc::string::{String, ToString};
use alloc::sync::Arc;
use uuid::Uuid;
use core::result::Result;

use core::{fmt};
use naming::shared_types::{DirEntry, FileType, OpenOptions};
use syscall::return_vals::Errno;

use super::stat::Mode;
use super::stat::Stat;
use super::traits::{DirectoryObject, FileObject, FileSystem, NamedObject};

use crate::process_manager;
use crate::device::apic::now_ticks;
use crate::memory::dram;

use alloc::format;


pub struct ProcFs {
    proc_root_dir: Arc<ProcDir>,
}
impl ProcFs {
    pub fn new() -> ProcFs {
        ProcFs {
            proc_root_dir: Arc::new(ProcDir),
        }
    }
}

impl FileSystem for ProcFs {
    fn root_dir(&self) -> Arc<dyn DirectoryObject> {
        self.proc_root_dir.clone()
    }
}

//-----------------------------------------------------------------------------------------------//
// ProcDir
pub struct ProcDir;

impl DirectoryObject for ProcDir {
    fn lookup(&self, name: &str) -> Result<NamedObject, Errno> {
        // the current Directory is /proc
        // /proc/<pid>
        if let Ok(pid) = Uuid::parse_str(name) {
            if process_manager().read().is_active_process(pid) {
                Ok(NamedObject::DirectoryObject(Arc::new(ProcPidDir { pid })))
            } else {
                Err(Errno::ENOENT) // not active_id
            }
        } else if name == "ps"{
            // /proc/ps, return stats of all active processes as a CSV-Like file
            Ok(NamedObject::FileObject(
                Arc::new(ProcPsFile {})
            ))
        } else if name == "ticks"{
            // /proc/ticks, return the ticks as a file
            Ok(NamedObject::FileObject(
                Arc::new(ProcTicksFile {})
            ))
        } else if name == "meminfo"{
            // /proc/meminfo, return the meminfo as a file
            Ok(NamedObject::FileObject(
                Arc::new(ProcMemInfoFile {})
            ))
        } else {
            Err(Errno::ENOENT) // Return error if the file is not found
        }
    }

    fn readdir(&self, index: usize) -> Result<Option<DirEntry>, Errno> {

        // collect the possible Files
        const STATIC: &[(&str, FileType)] = &[
            ("ps", FileType::Regular),
            ("ticks", FileType::Regular),
            ("meminfo", FileType::Regular),
        ];

        // first return all Files
        if index < STATIC.len() {
            let (name, file_type) = STATIC[index];
            return Ok(Some(DirEntry {
                file_type,
                name: name.to_string(),
            }));
        }

        // stop creating Directories if all pids are handled
        let pids =  process_manager().read().active_process_ids();
        let pid_index = index - STATIC.len();

        if pid_index >= pids.len() {
            return Ok(None);
        }
        
        // create a Directory for each Process
        Ok(Some(DirEntry {
                file_type: FileType::Directory,
                name: pids[pid_index].to_string(),
            }))
    }

    fn create_file(&self, _name: &str, _mode: Mode) -> Result<NamedObject, Errno> {
        Err(Errno::ENOTSUP)
    }

    fn create_dir(&self, _name: &str, _mode: Mode) -> Result<NamedObject, Errno> {
        Err(Errno::ENOTSUP)
    }

    fn create_pipe(&self, _name: &str, _mode: Mode) -> Result<NamedObject, Errno> {
        Err(Errno::ENOTSUP)
    }

    fn stat(&self) -> Result<Stat, Errno> {
        Err(Errno::ENOTSUP)
    }
}

impl fmt::Debug for ProcDir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProcDir").finish()
    }
}

//-----------------------------------------------------------------------------------------------//
// ProcPidDir

pub struct ProcPidDir {
    pid: Uuid,
}

impl DirectoryObject for ProcPidDir {
    fn lookup(&self, _name: &str) -> Result<NamedObject, Errno> {
        match _name {
            "status" => Ok(NamedObject::FileObject(
                Arc::new(ProcStatusFile { pid: self.pid })
            )),
            _ => Err(Errno::ENOENT),
        }
        
    }

    fn readdir(&self, index: usize) -> Result<Option<DirEntry>, Errno> {
        match index {
            // always create a status File
            0 => Ok(Some(DirEntry {
                file_type: FileType::Regular,
                name: "status".to_string(),
            })),
            _ => Ok(None),
        }
    }

    fn create_file(&self, _name: &str, _mode: Mode) -> Result<NamedObject, Errno> {
        Err(Errno::ENOTSUP)
    }

    fn create_dir(&self, _name: &str, _mode: Mode) -> Result<NamedObject, Errno> {
        Err(Errno::ENOTSUP)
    }

    fn create_pipe(&self, _name: &str, _mode: Mode) -> Result<NamedObject, Errno> {
        Err(Errno::ENOTSUP)
    }

    fn stat(&self) -> Result<Stat, Errno> {
        Err(Errno::ENOTSUP)
    }
}

impl fmt::Debug for ProcPidDir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProcPidDir").finish()
    }
}

//-----------------------------------------------------------------------------------------------//
// ProcStatusFile

pub struct ProcStatusFile {
    pid: Uuid,
}

impl FileObject for ProcStatusFile {
    fn read(&self, buf: &mut [u8], offset: usize, _options: OpenOptions) -> Result<usize, Errno> { 
        if let Some(proc) = process_manager().read().get_stats(self.pid) {
            // fill the File with Process Data
            let mut content = String::new();

            content.push_str(&format!("  PID: {:>10}\n", proc.pid()));
            content.push_str(&format!("UTIME: {:>10}\n", proc.utime()));
            content.push_str(&format!("STIME: {:>10}\n", proc.stime()));
            content.push_str(&format!("  RSS: {:>10}\n", proc.rss_in_bytes()));

            let data = content.as_bytes();

            if offset > data.len() {
                return Ok(0);
            }

            let len = if data.len() - offset < buf.len() { data.len() - offset } else { buf.len() };

            buf[0..len].clone_from_slice(&data[offset..offset + len]);
            return Ok(len);
        }
        Err(Errno::EAGAIN)
    }

    fn stat(&self) -> Result<Stat, Errno> {
        Err(Errno::ENOTSUP)
    }

    fn write(&self, _buf: &[u8], _offset: usize, _options: OpenOptions) -> Result<usize, Errno> {
        Err(Errno::ENOTSUP)
    }
}

impl fmt::Debug for ProcStatusFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProcStatusFile").finish()
    }
}

//-----------------------------------------------------------------------------------------------//
// ProcPsFile
pub struct ProcPsFile {}

impl FileObject for ProcPsFile {
    fn read(&self, buf: &mut [u8], offset: usize, _options: OpenOptions) -> Result<usize, Errno> { 
        let processes = process_manager().read().get_all_stats();
        // fill the File with Process Data
        let mut content = String::new();
        // first line
        content.push_str(&format!("{:>36}{:>20}{:>10}{:>10}{:>10}\n", "PID","NAME","UTIME","STIME","RSS"));
        // dynamically fill rest
        for proc in processes {
            content.push_str(&format!("{:>36}{:>20}{:>10}{:>10}{:>10}\n",
                proc.pid(),
                proc.name(),
                proc.utime(),
                proc.stime(),
                proc.rss_in_bytes()));
        }

        let data = content.as_bytes();

        if offset > data.len() {
            return Ok(0);
        }

        let len = if data.len() - offset < buf.len() { data.len() - offset } else { buf.len() };

        buf[0..len].clone_from_slice(&data[offset..offset + len]);
        return Ok(len);
    }

    fn stat(&self) -> Result<Stat, Errno> {
        Err(Errno::ENOTSUP)
    }

    fn write(&self, _buf: &[u8], _offset: usize, _options: OpenOptions) -> Result<usize, Errno> {
        Err(Errno::ENOTSUP)
    }
}

impl fmt::Debug for ProcPsFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProcPsFile").finish()
    }
}

//-----------------------------------------------------------------------------------------------//
// ProcTicksFile
pub struct ProcTicksFile {}

impl FileObject for ProcTicksFile {
    fn read(&self, buf: &mut [u8], offset: usize, _options: OpenOptions) -> Result<usize, Errno> { 
        
        // fill the File with Data
        let mut content = String::new();

        content.push_str(&format!("{} \n", now_ticks()));

        let data = content.as_bytes();

        if offset > data.len() {
            return Ok(0);
        }

        let len = if data.len() - offset < buf.len() { data.len() - offset } else { buf.len() };

        buf[0..len].clone_from_slice(&data[offset..offset + len]);
        return Ok(len);
    }

    fn stat(&self) -> Result<Stat, Errno> {
        Err(Errno::ENOTSUP)
    }

    fn write(&self, _buf: &[u8], _offset: usize, _options: OpenOptions) -> Result<usize, Errno> {
        Err(Errno::ENOTSUP)
    }
}

impl fmt::Debug for ProcTicksFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProcTicksFile").finish()
    }
}

//-----------------------------------------------------------------------------------------------//
// ProcMemInfoFile
pub struct ProcMemInfoFile {}

impl FileObject for ProcMemInfoFile {
    fn read(&self, buf: &mut [u8], offset: usize, _options: OpenOptions) -> Result<usize, Errno> { 
        
        // fill the File with Data
        let mut content = String::new();

        content.push_str(&format!("{} \n", dram::limit()));

        let data = content.as_bytes();

        if offset > data.len() {
            return Ok(0);
        }

        let len = if data.len() - offset < buf.len() { data.len() - offset } else { buf.len() };

        buf[0..len].clone_from_slice(&data[offset..offset + len]);
        return Ok(len);
    }

    fn stat(&self) -> Result<Stat, Errno> {
        Err(Errno::ENOTSUP)
    }

    fn write(&self, _buf: &[u8], _offset: usize, _options: OpenOptions) -> Result<usize, Errno> {
        Err(Errno::ENOTSUP)
    }
}

impl fmt::Debug for ProcMemInfoFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProcMemInfoFile").finish()
    }
}
