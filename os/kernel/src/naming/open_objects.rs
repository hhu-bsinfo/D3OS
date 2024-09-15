/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: open_objects                                                    ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Managing all open objects in the open object table (oot). This  ║
   ║         table stores tuples (usize, NsOpenFile).                        ║
   ║         The number of max. open objects is limited by MAX_OPEN_OBJECTS  ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, Univ. Duesseldorf, 15.9.2024                ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use alloc::sync::Arc;
use alloc::boxed::Box;
use spin::{Mutex, Once};
use alloc::vec::Vec;
use core::result::Result;

use syscall::return_vals::{SyscallResult, Errno};

use crate::naming::traits::NsOpenFile;


/// Max. number of open objetcs
const MAX_OPEN_OBJECTS: usize = 0x100;

/// Helper function returning safe access to the open object table (oot)
pub(super) fn ns_get_oot() -> Arc<Mutex<NsOpenObjectTable>> {
    let oot = NS_OPEN_OBJECTS.get();
    return oot.unwrap().clone();
}


static NS_OPEN_OBJECTS: Once<Arc<Mutex<NsOpenObjectTable>>> = Once::new();

pub fn ns_open_object_table_init() {
    NS_OPEN_OBJECTS.call_once(|| Arc::new(Mutex::new(NsOpenObjectTable::new())));
}


pub(super) struct NsOpenObjectTable {
    open_handles: Vec< (usize, Option<Arc<Box<dyn NsOpenFile>>>) >,
    free_handles: [usize; MAX_OPEN_OBJECTS],
}

impl NsOpenObjectTable {

    fn new() -> NsOpenObjectTable {
        NsOpenObjectTable {
            open_handles: Vec::new(),
            free_handles: [0; MAX_OPEN_OBJECTS],
        }
    }

    /// Get `NsOpenFile` for a given handle, if possible 
    pub fn get(&self, file_handle: usize) -> Result<&Arc<Box<dyn NsOpenFile>>, Errno> {
        for (fh,fptr) in &self.open_handles {
            if *fh == file_handle {
                match fptr {
                    Some(v) => return Ok(v),
                    _ => return Err(Errno::EINVALH),
                }
            }
        }
        return Err(Errno::EINVALH);
    }

    fn find_free_handle(&mut self) -> Option<usize> {
        for (index, &value) in self.free_handles.iter().enumerate() {
            if value == 0 {
                self.free_handles[index] = 1;
                return Some(index);
            }
        }
        None
    }
    
    pub(super) fn create_new_handle_for_filepointer(&mut self, fp: Box<dyn NsOpenFile>) -> SyscallResult {
        let opt_new_handle = self.find_free_handle();
        if opt_new_handle.is_none() {
            return Err(Errno::ENOHANDLES);
        }
        let new_handle = opt_new_handle.unwrap();
        self.open_handles.push( (new_handle, Some(Arc::new(fp)) ));
        Ok(new_handle)
    }

}

