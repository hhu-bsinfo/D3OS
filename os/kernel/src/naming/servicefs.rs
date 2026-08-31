use alloc::{string::{String, ToString}, sync::Arc, vec::Vec};
use concurrent::ipc::IpcQueue;
use naming::shared_types::DirEntry;
use spin::rwlock::RwLock;
use syscall::return_vals::Errno;

use crate::{consts, naming::{stat::Mode, traits::{DirectoryObject, FileObject, FileSystem, NamedObject}}, process::core_local_storage::scheduler};

#[derive(Debug)]
struct Service {
    name: String,
    ipc_queue_addr: usize,
}

pub struct ServiceFs {
    service_root_dir: Arc<ServiceDir>,
}

impl ServiceFs {
    pub fn new() -> ServiceFs {
        ServiceFs {
            service_root_dir: Arc::new(ServiceDir(RwLock::new(ServiceDirInner { services: Vec::new() }))),
        }
    }
}

impl FileSystem for ServiceFs {
    fn root_dir(&self) -> Arc<dyn DirectoryObject> {
        self.service_root_dir.clone()
    }
}

#[derive(Debug)]
struct ServiceDirInner {
    services: Vec<Service>,
}

#[derive(Debug)]
pub struct ServiceDir(RwLock<ServiceDirInner>);

impl DirectoryObject for ServiceDir {
    
    fn lookup(&self, name: &str) -> Result<super::traits::NamedObject, Errno> {
        // the current directory is /svc
        // check for /svc/<name>
        match self.0.read().services.iter().find(|svc| svc.name == name) {
            Some(svc) => Ok(NamedObject::FileObject(Arc::new(ServiceObject { ipc_queue_addr: svc.ipc_queue_addr }))),
            None => Err(Errno::ENOENT)
        }
    }

    fn readdir(&self, index: usize) -> Result<Option<DirEntry>, Errno> {
        match self.0.read().services.get(index) {
            Some(svc) => Ok(Some(DirEntry { file_type: naming::shared_types::FileType::Regular, name: svc.name.clone() })),
            None => Ok(None),
        }
    }

    fn create_file(&self, _name: &str, _mode: Mode) -> Result<NamedObject, Errno> {
        let ipc_queue_addr = scheduler().current_thread()
            .process()
            .virtual_address_space
            .translate_ptr(consts::USER_SPACE_MAILBOX_START as *const IpcQueue)
            .expect("IpcQueue isnt mapped into virt addr space") as usize;
        self.0.write().services.push(Service { name: _name.to_string(), ipc_queue_addr });
        Ok(NamedObject::FileObject(Arc::new(ServiceObject { ipc_queue_addr })))
    }

    fn create_dir(&self, _name: &str, _mode: Mode) -> Result<NamedObject, Errno> {
        Err(Errno::ENOTSUP)
    }

    fn create_pipe(&self, _name: &str, _mode: Mode) -> Result<NamedObject, Errno> {
        Err(Errno::ENOTSUP)
    }

    fn stat(&self) -> Result<super::stat::Stat, Errno> {
        Err(Errno::ENOTSUP)
    }
}

#[derive(Debug)]
pub struct ServiceObject {
    ipc_queue_addr: usize, // phys addr of ipc queue
}

impl FileObject for ServiceObject {
    
    /// read phys addr of IpcQueue into _buf
    fn read(&self, _buf: &mut [u8], _offset: usize, _options: naming::shared_types::OpenOptions) -> Result<usize, syscall::return_vals::Errno> {
        _buf.copy_from_slice(&self.ipc_queue_addr.to_le_bytes());
        Ok(0)
    }

    fn write(&self, _buf: &[u8], _offset: usize, _options: naming::shared_types::OpenOptions) -> Result<usize, Errno> {
        Err(Errno::ENOTSUP)
    }

    fn stat(&self) -> Result<super::stat::Stat, Errno> {
        Err(Errno::ENOTSUP)
    }
}