/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: mount                                                           ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Public functions for the mount table.                                   ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Alexander Lopushkov, Univ. Duesseldorf, 2.1.2026                ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use alloc::{collections::BTreeMap, string::{String, ToString}, sync::Arc};
use super::traits::FileSystem;

pub struct MountTable {
    map: BTreeMap<String, Arc<dyn FileSystem>>,
}

impl MountTable {
    pub fn new() -> Self {
        Self { map: BTreeMap::new() }
    }

    // create a Mount Point
    pub fn mount(&mut self, path: &str, fs: Arc<dyn FileSystem>) {
        self.map.insert(path.to_string(), fs);
    }

    // retrieve the FileSystem of the path, if it is a Mount Point
    pub fn get(&self, path: &str) -> Option<Arc<dyn FileSystem>> {
        self.map.get(path).cloned()
    }
}

