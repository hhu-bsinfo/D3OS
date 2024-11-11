/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: main                                                            ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Main module of the implementation of the naming service (ns).   ║
   ║            - NS: global entry point                                     ║
   ║         Following traits are implemented:                               ║
   ║            - NsInterface:  all operations provided by the naming service║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, Univ. Duesseldorf, 15.9.2024                ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use alloc::boxed::Box;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt::Debug;
use spin::Once;

use syscall::return_vals::{Errno, OpenOptions};

use crate::naming::traits::*;
use crate::naming::directory::{NsDirectoryWrapper,check_absolute_path};
use crate::naming::open_objects::ns_open_object_table_init;


/// Entrypoint of the naming service
pub(super) static NS: Once<Arc<dyn NsInterface>> = Once::new();

/// Init `NS` once
pub(super) fn init() {
    NS.call_once(|| Arc::new(Ns::new()));
    ns_open_object_table_init();
}

/// Helper function returning safe access to the naming service
pub(super) fn ns_get() -> Arc<dyn NsInterface> {
    let ns = NS.get();
    return ns.unwrap().clone();
}

/// Entrypoint of the naming service
#[derive(Debug)]
pub(super) struct Ns {
    /// root directory
    root_dir: Arc<NsDirectoryWrapper>,
}

impl Ns {
    pub fn new() -> Ns {
        Ns {
            root_dir: Arc::new(NsDirectoryWrapper::new()),
        }
    }

    /// return root directory of the naming service
    pub fn root_dir(&self) -> &Arc<NsDirectoryWrapper> {
        &self.root_dir
    }
}

impl NsInterface for Ns {

    /// Create a directory (including all sub directories)
    fn mkdir(&self, path: &String) -> Result<(), Errno> {
        if check_absolute_path(path) {
            let mut components: Vec<&str> = path.split("/").collect();

            components.reverse();
            components.pop();

            // get root directory and create directories as needed
            self.root_dir().clone().mkdir(&mut components)
        } else {
            Err(Errno::ENOENT)
        }
    }

    /// Dump all nodes in the naming service (for debugging)
    fn dump(&self) {
        println!("/");

        // get root directory and create directories as needed
        self.root_dir().clone().dump(String::from(""));
    }

    fn open(&self, path: &String, flags: OpenOptions)-> Result<Box<dyn NsOpenFile>, Errno> {
        if check_absolute_path(path) {
            let mut components: Vec<&str> = path.split("/").collect();

            components.reverse();
            components.pop();

            // get root directory and open the desired file
            self
                .root_dir()
                .clone()
                .open(&mut components, flags)
        } else {
            Err(Errno::ENOENT)
        }
    }
}