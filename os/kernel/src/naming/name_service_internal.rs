/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: name_service_interal                                            ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Internal implementation of name service.                        ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, 29.8.2024, HHU                              ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use syscall::return_vals::Errno;
use crate::naming::stat;
use crate::naming::stat::Mode;
use crate::naming::stat::Stat;
use crate::naming::name_service::Result;

use alloc::boxed::Box;
use alloc::string::ToString;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::fmt::Debug;
use core::mem;

use spin::{Once, RwLock};

/// helper function returning `root_dir`
pub(super) fn get_root_dir() -> Arc<Directory> {
    let root = NAME_SERVICE.get();
    let root_dir = root.unwrap().root_dir();
    root_dir.clone()
}

pub(super) static NAME_SERVICE: Once<Arc<NameService>> = Once::new();

pub(super) struct NameService {
    root_dir: Arc<Directory>,
}

impl NameService {
    pub(super) fn new() -> NameService {
        NameService {
            root_dir: Arc::new(Directory::new()),
        }
    }

    fn root_dir(&self) -> &Arc<Directory> {
        &self.root_dir
    }
}

#[derive(Debug)]
pub(super) struct Directory(RwLock<DirectoryInner>);

#[derive(Debug, Clone)]
struct DirectoryInner {
    entries: Vec<DirEntry>,
}

#[derive(Debug, Clone)]
pub struct DirEntry {
    entry_type: EntryType,
    stat: Stat,
}

#[derive(Debug, Clone)]
enum EntryType {
    Container(Container),
    Directory(Box<Directory>),
}

#[derive(Debug, Clone)]
struct Container {
    content: Vec<u8>,
}

impl Directory {
    fn new() -> Self {
        Directory(RwLock::new(DirectoryInner {
            entries: Vec::new(),
        }))
    }

    ///
    /// Create directory (with all sub directories)
    ///  
    pub(super) fn mkdir(&self, path: &str) -> Result<()> {
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        self.mkdir_in_dir(&parts)
    }

    // Helper function to recursively add sub directories as needed
    fn mkdir_in_dir(&self, parts: &[&str]) -> Result<()> {
        let mut dir = self.0.write();

        // If no more path parts, we did not find the file
        if parts.is_empty() {
            return Ok(());
        }

        // Find the directory matching the current part
        let current_part = parts[0];
        let remaining_parts = &parts[1..];
        for entry in &dir.entries {
            if entry.stat.name == current_part {
                // entry found, continue recursively, if more to do
                if let EntryType::Directory(ref dir) = entry.entry_type {
                    if remaining_parts.is_empty() {
                        return Err(Errno::EEXIST); // we are done, dir already exists
                    } else {
                        return dir.mkdir_in_dir(remaining_parts); // continue recursively
                    }
                } else {
                    // found file with same name, abort
                    return Err(Errno::ENOTDIR);
                }
            }
        }

        // directory not found -> create it and continue recursively

        let new_entry = DirEntry::new_directory(Stat::new(
            current_part.to_string(),
            Mode::new(stat::MODE_DIR),
            0,
        ));
        dir.entries.push(new_entry);

        // Retrieve the newly created directory
        if let EntryType::Directory(ref mut new_dir) = dir.entries.last_mut().unwrap().entry_type {
            return new_dir.mkdir_in_dir(remaining_parts);
        } else {
            return Err(Errno::EEXIST); // This should not happen
        }
    }

    ///
    /// Register a new entry in the given `path`
    ///  
    pub(super) fn mkentry(&self, path: &str, name: &str, content: Vec<u8>) -> Result<()> {
        let element_size = mem::size_of::<u8>();
        let total_size = element_size * content.len();

        let stat = Stat::new(name.to_string(), Mode::new(stat::MODE_CONT), total_size);

        let new_entry = DirEntry::new_file(stat, content);
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        self.mkentry_in_dir(&parts, new_entry)
    }

    /// Recursive helper function for `mkentry`
    fn mkentry_in_dir(&self, parts: &[&str], new_entry: DirEntry) -> Result<()> {
        let mut dir = self.0.write();

        // If no more path parts, add the entry to the current directory
        if parts.is_empty() {
            for entry in &dir.entries {
                if entry.stat.name == new_entry.stat.name {
                    return Err(Errno::EEXIST); // file already exists
                }
            }
            dir.entries.push(new_entry);
            return Ok(());
        }

        // Recusively navigate to the right sub directory
        let current_part = parts[0];
        let remaining_parts = &parts[1..];
        for entry in &dir.entries {
            if entry.stat.name == current_part {
                if let EntryType::Directory(ref directory) = entry.entry_type {
                    return directory.mkentry_in_dir(remaining_parts, new_entry);
                } else {
                    return Err(Errno::ENOENT); // sub directory not found
                }
            }
        }
        return Err(Errno::ENOENT);
    }

    ///
    /// Retrieve `stat` info for a given entry
    ///
    pub(super) fn stat(&self, path: &str) -> Result<Stat> {
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        match self.get_dentry(&parts) {
            Ok(dentry) => Ok(dentry.stat),
            Err(e) => Err(e),
        }
    }

    ///
    /// Retrieve content for a given container
    ///
    pub(super) fn cont(&self, path: &str) -> Result<Vec<u8>> {
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        match self.get_dentry(&parts) {
            Ok(dentry) => {
                // check if this is a container
                if let EntryType::Container(ref entry) = dentry.entry_type {
                    // yes, return the content
                    return Ok(entry.content.clone());
                } else {
                    // no, error
                    return Err(Errno::ENOENT);
                }
            }
            Err(e) => Err(e),
        }
    }

    ///
    /// Retrieve content for a given directory
    ///
    pub(super) fn dir(&self, path: &str) -> Result<Vec<Stat>> {
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        match self.get_dentry(&parts) {
            Ok(dentry) => {
                // check if this is a directory
                if let EntryType::Directory(ref dentry) = dentry.entry_type {
                    // yes, return the content
                    let mut ret: Vec<Stat> = Vec::new();
                    for entry in &dentry.0.read().entries {
                        ret.push(entry.stat.clone());
                    }
                    return Ok(ret);
                } else {
                    // no, error
                    return Err(Errno::ENOENT);
                }
            }
            Err(e) => Err(e),
        }
    }

    /// Recursive helper function for looking up a dentry for read access (returns a clone)
    fn get_dentry(&self, parts: &[&str]) -> Result<DirEntry> {
        let dir = self.0.read();

        // If no more path parts, we did not find the file
        if parts.is_empty() {
            return Err(Errno::ENOENT);
        }

        // Recusively navigate to the right sub directory
        let current_part = parts[0];
        let remaining_parts = &parts[1..];
        for entry in &dir.entries {
            if entry.stat.name == current_part {
                if remaining_parts.is_empty() {
                    return Ok(entry.clone());
                } else if let EntryType::Directory(ref directory) = entry.entry_type {
                    return directory.get_dentry(remaining_parts);
                } else {
                    return Err(Errno::ENOENT);
                }
            }
        }
        return Err(Errno::ENOENT);
    }

    /// Rename given entry (any type)
    pub fn rename(&self, path: &str, new_name: &str) -> Result<()> {
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        self.rename_internal(&parts, new_name)
    }

    /// Recursive helper function for looking up an entry and rename it, if found
    fn rename_internal(&self, parts: &[&str], new_name: &str) -> Result<()> {
        let mut dir = self.0.write();

        // If no more path parts, we did not find the file
        if parts.is_empty() {
            return Err(Errno::ENOENT);
        }

        // Recusively navigate to the right sub directory
        let current_part = parts[0];
        let remaining_parts = &parts[1..];
        for entry in &mut dir.entries {
            if entry.stat.name == current_part {
                if remaining_parts.is_empty() {
                    entry.stat.name = new_name.to_string();
                    return Ok(());
                } else if let EntryType::Directory(ref directory) = entry.entry_type {
                    return directory.rename_internal(remaining_parts, new_name);
                } else {
                    return Err(Errno::ENOENT);
                }
            }
        }
        return Err(Errno::ENOENT);
    }

    /// Delete given entry (any type)
    pub fn del(&self, path: &str) -> Result<()> {
        // Check if we have a path to a directory which is empty
        match self.dir(path) {
            // Dir found?
            Ok(s) => {
                // Yes, is it empty?
                if s.len() == 0 {
                    let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
                    return self.del_internal(&parts);
                }
                // If we found a dire which is not empty, return an error
                else {
                    return Err(Errno::ENOTEMPTY);
                }
            }
            // No dir found?, we continue below
            Err(_e) => {
                // Check if we have a path to a container/symlink
                match self.stat(path) {
                    // Entry found?
                    Ok(_s) => {
                        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
                        return self.del_internal(&parts);
                    }
                    // No dir found?, we continue below
                    Err(_e) => {
                        return Err(Errno::ENOENT);
                    }
                }
            }
        }
    }

    /// Recursive helper function for looking up an entry and delete it, if found
    fn del_internal(&self, parts: &[&str]) -> Result<()> {
        let current_part = parts[0];
        let remaining_parts = &parts[1..];
        let mut dir = self.0.write();

        // Last part -> remove entry and return
        if remaining_parts.is_empty() {
            dir.entries.retain(|entry| entry.stat.name != current_part);
            return Ok(());
        }
        // Recusively navigate to the right sub directory
        else {
            for entry in &mut dir.entries {
                if entry.stat.name == current_part {
                    if let EntryType::Directory(ref directory) = entry.entry_type {
                        return directory.del_internal(remaining_parts);
                    } else {
                        return Err(Errno::ENOENT);
                    }
                }
            }
        }

        return Err(Errno::ENOENT);
    }

    ///
    /// Debug function dumping full naming service content
    ///  
    pub(super) fn dump(&self, depth: usize) {
        let indent = "   ".repeat(depth);
        for entry in &self.0.read().entries {
            match &entry.entry_type {
                EntryType::Container(_file) => {
                    println!("{}[F] {}, {:?}", indent, entry.stat.name, entry.stat);
                    //println!("{}  [F]: {}, size: {}", indent, file.name, entry.stat.size)
                }
                EntryType::Directory(dir) => {
                    println!("{}[D] {}", indent, entry.stat.name);
                    dir.dump(depth + 1);
                }
            }
        }
    }
}

// Implement Clone for Directory manually to handle the RwLock
impl Clone for Directory {
    fn clone(&self) -> Self {
        // Clone the inner DirectoryInner
        let inner_clone = self.0.read().clone();
        Directory(RwLock::new(inner_clone))
    }
}

impl DirEntry {
    fn new_file(stat: Stat, content: Vec<u8>) -> Self {
        let c = content.clone();
        DirEntry {
            entry_type: EntryType::Container(Container { content: c }),
            stat,
        }
    }

    fn new_directory(stat: Stat) -> Self {
        DirEntry {
            entry_type: EntryType::Directory(Box::new(Directory::new())),
            stat,
        }
    }
}
