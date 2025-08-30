/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: lookup                                                          ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Lookup functions.                                                       ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, Univ. Duesseldorf, 25.8.2025                ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use alloc::string::String;
use alloc::vec::Vec;
use alloc::sync::Arc;
use super::api::ROOT;
use super::traits;
use super::traits::{NamedObject, DirectoryObject};
use syscall::return_vals::Errno;

/// Resolves an absolute path into an `DirectoryLike`
pub(super) fn lookup_dir(path: &String) -> Result<Arc<dyn DirectoryObject>, Errno> {
    match lookup_named_object(path)? {
        NamedObject::DirectoryObject(dir) => Ok(dir),
        NamedObject::FileObject(_) => Err(Errno::ENOTDIR),
        NamedObject::PipeObject(_) => Err(Errno::ENOTDIR),
    }
}

/// Resolves absolute `path` into a named object. \
/// Returns `Ok(NamedObject)` or `Err`
pub(super) fn lookup_named_object(path: &str) -> Result<NamedObject, Errno> {
    let mut found_named_object;

    if check_absolute_path(path) {
        if path == "/" {
            found_named_object = traits::as_named_object(ROOT.get().unwrap().root_dir());
            return Ok(found_named_object);
        }
        let mut components: Vec<&str> = path.split("/").collect();
        components.remove(0); // remove empty string at position 0

        // get root directory and open the desired file
        let mut current_dir = ROOT.get().unwrap().root_dir();
        let mut len = components.len();
        let mut found;
        for component in &components {
            found = current_dir.lookup(component);
            if found.is_err() {
                return Err(Errno::ENOENT);
            }
            found_named_object = found.unwrap();

            // if not last component, this must be a directory
            if len > 1 {
                if !found_named_object.is_dir() {
                    return Err(Errno::ENOENT);
                }
                current_dir = found_named_object.as_dir().unwrap().clone();
            // if this is the last component, this must be a file or directory (see flags)
            } else {
                return Ok(found_named_object.clone());
            }
            len -= 1;
        }
    }
    Err(Errno::ENOENT)
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
