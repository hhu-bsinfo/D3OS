use alloc::string::String;
use alloc::sync::Arc;
use core::sync::atomic::AtomicUsize;
use naming::shared_types::OpenOptions;
use crate::capabilities::capability::{Capability, CapabilityFlags};
use crate::naming::traits::NamedObject;

pub struct NamingObject {
    pub(crate)named_object: Arc<NamedObject>,
    pub(crate)access_rights: OpenOptions,
    pub(crate)path: String,
    pub(crate)position: AtomicUsize, // For files and pipes, tracks the current read/write position. For directories, tracks the current index for readdir.
}

#[derive(Copy, Clone)]
pub enum ObjectType {
    File,
    Directory,
    Pipe,
}

impl NamingObject {
    fn new(object: NamedObject, rights: OpenOptions, path: String) -> Self {
        Self {
            named_object: Arc::from(object),
            access_rights: rights,
            path,
            position: AtomicUsize::new(0),
        }
    }
}

pub fn create_naming_capability(object: NamedObject, rights: OpenOptions, path: String) -> Capability<NamingObject> {
    let mut flags = CapabilityFlags::empty();
    if rights.contains(OpenOptions::READONLY) || rights.contains(OpenOptions::READWRITE) {
        flags |= CapabilityFlags::READ;
    }
    if rights.contains(OpenOptions::WRITEONLY) || rights.contains(OpenOptions::READWRITE) {
        flags |= CapabilityFlags::WRITE;
    }
    if rights.contains(OpenOptions::SHARE) {
        flags |= CapabilityFlags::SHARE;
    }

    Capability::new(NamingObject::new(object, rights, path), flags)
}
