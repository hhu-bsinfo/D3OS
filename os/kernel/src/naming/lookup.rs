/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: lookup                                                          ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Lookup functions.                                                       ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, Univ. Duesseldorf, 25.8.2025                ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::sync::Arc;
use super::api::ROOT;
use super::api::MOUNTS;
use super::traits;
use super::traits::{NamedObject, DirectoryObject};
use syscall::return_vals::Errno;

/// Resolves an absolute path into an `DirectoryLike`
pub(super) fn lookup_dir(path: &String) -> Result<Arc<dyn DirectoryObject>, Errno> {
    match lookup_named_object(path)?.0 {
        NamedObject::DirectoryObject(dir) => Ok(dir),
        NamedObject::FileObject(_) => Err(Errno::ENOTDIR),
        NamedObject::PipeObject(_) => Err(Errno::ENOTDIR),
    }
}


/// Resolves absolute `path` into a named object. \
/// Returns `Ok(NamedObject, ParentDir)` or `Err`
pub(super) fn lookup_named_object(path: &str) -> Result<(NamedObject, Option<Arc<dyn DirectoryObject>>), Errno> {
    let mut found_named_object;

    if check_absolute_path(path) {
        if path == "/" {
            found_named_object = traits::as_named_object(ROOT.get().unwrap().root_dir());
            return Ok((found_named_object, None));
        }
        let components: Vec<&str> = path.split("/")
            .skip(1) // skip empty string at position 0
            .collect();

        // get root directory and open the desired file
        let mut current_dir = ROOT.get().unwrap().root_dir();
        let len = components.len();
        let mut index = 0;
        let mut found;
        
        let mut cur_path = "/".to_string();

        for component in &components {
            // using index instead of len, to get the information, if it is the last component
            let is_last = index == len - 1;

            found = current_dir.lookup(component);
            if found.is_err() {
                return Err(Errno::ENOENT);
            }
            found_named_object = found.unwrap();
            
            // build the Path as it gets resolved
            build_path(&mut cur_path, component);

            // check MOUNTS, if current_dir is mounted to a FS, switch to that FS
            if let Some(mount) = MOUNTS.get() {
                if let Some(fs) = mount.read().get(&cur_path) {
                    // replace the current root, with the according mounted_root
                    let mounted_root = fs.root_dir();
                    if is_last {
                        // return if the path reached its end, and it is a mount point
                        let parent_dir = Arc::clone(&mounted_root);
                        return Ok((traits::as_named_object(mounted_root), Some(parent_dir)));
                    } else {
                        // switch to the mount point, and resolve the rest of the path
                        current_dir = mounted_root;
                        index += 1;
                        continue;
                    }
                }
            }

            // if this is the last component, this must be a file or directory (see flags)
            if is_last {
                return Ok((found_named_object.clone(), Some(current_dir)));
            } else {
                // if not last component, this must be a directory
                if !found_named_object.is_dir() {
                    return Err(Errno::ENOENT);
                }
                current_dir = found_named_object.as_dir().unwrap().clone();
            }
            index += 1;
        }
    }
    Err(Errno::ENOENT)
}

/// Helper function for building a path (used for checking MOUNT)
fn build_path(cur: &mut String, component: &str) {
    if cur == "/" {
        cur.push_str(component);
    } else {
        cur.push('/');
        cur.push_str(component);
    }
}

/// Helper function for checking if `path` is an abolute path
fn check_absolute_path(path: &str) -> bool {
    if let Some(pos) = path.find('/') {
        if pos == 0 {
            return true;
        }
    }
    false
}

/// Helper function which returns a paths filename, i.e. its last component
pub fn get_filename_from_path(path: &str) -> &str {
    let components: Vec<&str> = path.split('/').collect();
    // check for trailing '/' -- if present, return second to last component instead
    let last = if path.chars().last() == Some('/') {
        *components.get(components.len() - 2).unwrap_or(&"")
    } else {
        *components.last().unwrap_or(&"")
    };
    last
}
