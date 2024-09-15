/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: directory                                                       ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Implementation of a directoy in the naming service.             ║
   ║         Following structs are defined and implemented:                  ║
   ║            - NsDirectoryWrapper: wraps a directory with a RwLock        ║
   ║            - NsDirectory: directory storing named objects               ║
   ║         Following traits are implemented:                               ║
   ║            - NsNode                                                     ║
   ║            - NsNodeDirectory                                            ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, Univ. Duesseldorf, 7.9.2024                 ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::any::Any;
use spin::RwLock;

use syscall::return_vals::{OpenOptions, Errno};

use crate::naming::traits::*;
use crate::naming::file::NsFile;


/// Wraps a directory with a RwLock
#[derive(Debug)]
pub(super) struct NsDirectoryWrapper(pub(super) RwLock<NsDirectory>);

impl NsDirectoryWrapper {
    pub(super) fn new() -> Self {
        NsDirectoryWrapper(RwLock::new(NsDirectory {
            children: Vec::new(),
        }))
    }
}

impl NsNode for NsDirectoryWrapper {
    /// Returns the node type
    fn get_type(&self) -> NsNodeType {
        NsNodeType::Directory
    }
}

impl NsNodeDirectory for NsDirectoryWrapper {

    /// Create directories (including all sub  directories) in the given path
    fn mkdir(&self, components: &mut Vec<&str>) -> Result<(),Errno> {
        if let Some(component) = components.pop() {
            let node_name = String::from(component);

            // follow path recurively for existing sub directories
            for (name, node) in &self.0.read().children {
                if name == &node_name {
                    let opt_sub_dir = node.downcast_ref::<NsDirectoryWrapper>();
                    if let Some(sub_dir) = opt_sub_dir {
                        return sub_dir.mkdir(components);
                    }
                }
            }

            // otherweise create them
            let directory = Box::new(NsDirectoryWrapper::new());
            let result = directory.mkdir(components);
            self.0.write().children.push((node_name, directory));
            result
        } else {
            Ok(())
        }
    }

    fn dump(&self, mut tabs: String) {
        tabs.push_str("  ");
        for (name, node) in &self.0.read().children {
            if let Some(directory) = node.downcast_ref::<NsDirectoryWrapper>() {
                println!("{}{} ({:?})", tabs, name, self.get_type());
                directory.dump(tabs.clone());
            } else if let Some(file) = node.downcast_ref::<NsFile>() {
                println!("{}{} ({:?})", tabs, name, file.get_type());
            } else {
                println!("{}{} (Unknown))", tabs, name);
            }
        }
    }

    fn open(
        &self,
        path: &mut Vec<&str>,
        flags: OpenOptions,
    )  -> Result<Box<dyn NsOpenFile>, Errno> {
        if let Some(path_part) = path.pop() {
            let node_name = String::from(path_part);

            if path.is_empty() == true {
                // reach endpoint => reach file
                for (name, node) in &self.0.read().children {
                    if name == &node_name {
                        let opt_file = node.downcast_ref::<NsFile>();
                        if let Some(file) = opt_file {
                            return file.get_handle(flags);
                        }
                    }
                }
            }

            if path.is_empty() == true {
                if flags.contains(OpenOptions::CREATE) {
                    // Create file on demand
                    let file = Box::new(NsFile::new());
                    let result = file.get_handle(flags);
                    self.0.write().children.push((node_name, file));

                    result
                } else {
                    Err(Errno::EINVAL)
                }
            } else {
                // traverse to the directories to the endpoint
                for (name, node) in &self.0.read().children {
                    if name == &node_name {
                        let opt_dir = node.downcast_ref::<NsDirectoryWrapper>();
                        if let Some(directory) = opt_dir {
                            return directory.open(path, flags);
                        }
                    }
                }
                Err(Errno::EINVAL)
            }
        } else {
            Err(Errno::EINVAL)
        }
    }
}


#[derive(Debug)]
pub(super) struct NsDirectory {
    children: Vec<(
        String,
        Box<dyn Any + Send + Sync>,
    )>,
}


/// Helper function to check if the argument is an abolute path
pub fn check_absolute_path(path: &String) -> bool {
    if let Some(pos) = path.find('/') {
        if pos == 0 {
            return true;
        }
    }

    false
}
