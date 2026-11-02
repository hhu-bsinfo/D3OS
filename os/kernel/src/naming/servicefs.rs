use alloc::{string::{String, ToString}, sync::Arc, vec::Vec};
use ipc::queues::IpcQueueType;
use naming::shared_types::DirEntry;
use spin::rwlock::RwLock;
use syscall::return_vals::Errno;
use uuid::Uuid;

use crate::{consts, naming::{stat::Mode, traits::{DirectoryObject, FileObject, FileSystem, NamedObject}}, process::core_local_storage::scheduler};

#[derive(Debug)]
pub struct Service {
    pub name: String,
    pub ipc_endpoints_addr: usize,
    pub owning_pid: Uuid,
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
    services: Vec<Arc<Service>>,
}

#[derive(Debug)]
pub struct ServiceDir(RwLock<ServiceDirInner>);

impl DirectoryObject for ServiceDir {
    
    fn lookup(&self, name: &str) -> Result<super::traits::NamedObject, Errno> {
        // the current directory is /svc
        // check for /svc/<name>
        match self.0.read().services.iter().find(|svc| svc.name == name) {
            Some(svc) => Ok(NamedObject::FileObject(Arc::new(ServiceObject { service: Arc::clone(svc) }))),
            None => Err(Errno::ENOENT)
        }
    }
    
    fn list(&self) -> Result<Vec<(NamedObject, String)>, Errno> {
        let mut res = Vec::new();
        for svc in self.0.read().services.iter() {
            res.push((NamedObject::FileObject(Arc::new(ServiceObject { service: Arc::clone(svc) })), svc.name.clone()));
        }
        Ok(res)
    }

    fn readdir(&self, index: usize) -> Result<Option<DirEntry>, Errno> {
        match self.0.read().services.get(index) {
            Some(svc) => Ok(Some(DirEntry { file_type: naming::shared_types::FileType::Regular, name: svc.name.clone() })),
            None => Ok(None),
        }
    }

    fn create_file(&self, name: &str, _mode: Mode) -> Result<NamedObject, Errno> {
        let process = scheduler().current_thread().process();
        let ipc_queue_addr = process
            .virtual_address_space
            .translate_ptr(consts::USER_SPACE_MAILBOX_START as *const IpcQueueType)
            .expect("IpcQueue isnt mapped into virt addr space") as usize;
        let svc = Arc::new(Service { name: name.to_string(), ipc_endpoints_addr: ipc_queue_addr, owning_pid: process.id() });
        self.0.write().services.push(Arc::clone(&svc));
        Ok(NamedObject::FileObject(Arc::new(ServiceObject { service: svc })))
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
    
    fn delete_file(&self, name: &str) -> Result<(), Errno> {
        let mut svc_lock = self.0.write();
        // check if service with name exists
        if let Some(pos) = svc_lock.services.iter().position(|svc| svc.name == name) {
            // remove service
            svc_lock.services.swap_remove(pos);
            Ok(())
        } else {
            // Error if service does not exist
            Err(Errno::EEXIST)
        }
    }
    
    fn delete_dir(&self, _name: &str) -> Result<(), Errno> {
        Err(Errno::ENOTSUP)
    }
    
    fn delete_pipe(&self, _name: &str) -> Result<(), Errno> {
        Err(Errno::ENOTSUP)
    }
}

#[derive(Debug)]
pub struct ServiceObject {
    pub service: Arc<Service>,
}

impl FileObject for ServiceObject {
    
    /// read `ServiceObject` into `buf`
    fn read(&self, buf: &mut [u8], _offset: usize, _options: naming::shared_types::OpenOptions) -> Result<usize, syscall::return_vals::Errno> {
        let selfslice = unsafe { core::slice::from_raw_parts((self as *const Self) as *const u8, core::mem::size_of::<Self>()) };
        d3os::copy_from_slice_sizechecked!(buf, selfslice);
        Ok(0)
    }

    fn write(&self, _buf: &[u8], _offset: usize, _options: naming::shared_types::OpenOptions) -> Result<usize, Errno> {
        Err(Errno::ENOTSUP)
    }

    fn stat(&self) -> Result<super::stat::Stat, Errno> {
        Err(Errno::ENOTSUP)
    }
}