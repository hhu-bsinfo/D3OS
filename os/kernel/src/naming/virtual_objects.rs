// kernel objects that exist only in memory, but are not part of the fs

use alloc::sync::Arc;
use syscall::return_vals::Errno;

use crate::naming::{NamedObject, PseudoFile, PseudoFileObject, create_open_table_entry, 
    free_open_table_entry, get_open_table_entry};

pub fn create_pseudo<T: PseudoFileObject + 'static>(pseudo: Arc<T>) -> Result<usize, Errno> {
    let p_obj = NamedObject::PseudoFileObject(Arc::new(PseudoFile::create(pseudo)));
    create_open_table_entry(p_obj)
}

pub fn close_pseudo(fh: usize) -> Result<usize, Errno> {
    free_open_table_entry(fh)
}

pub fn recover_pseudo<T: PseudoFileObject + 'static>(fh: usize) -> Result<Arc<T>, Errno> {
    let open_obj = get_open_table_entry(fh)?;
    
    let f = open_obj.inner_node().as_pseudo()?;

    Ok(PseudoFile::recover(f))
}

impl PseudoFile {

    fn create<T: PseudoFileObject + 'static>(pseudo: Arc<T>) -> Self {
        Self {
            ops: Arc::clone(&pseudo) as Arc<dyn PseudoFileObject>,
            private_data: Arc::into_raw(pseudo).cast()
        }
    }

    fn recover<T: PseudoFileObject + 'static>(pseudo: &Arc<Self>) -> Arc<T> {
        unsafe { Arc::from_raw(pseudo.private_data.cast::<T>()) }
    }
}