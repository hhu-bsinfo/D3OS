/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: ns_file                                                         ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Implementation of files stored in the heap (<Vec<u8>).          ║
   ║         Following structs are defined and implemented:                  ║
   ║            - NsFile                                                     ║
   ║         Following traits are implemented:                               ║
   ║            - NsNode                                                     ║
   ║            - NsNodeFile                                                 ║
   ║            - NsOpenFile                                                 ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, Univ. Duesseldorf, 15.9.2024                ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering::SeqCst;
use spin::RwLock;

use syscall::return_vals::{Errno, OpenOptions, SeekOrigin};
use crate::naming::traits::*;

/// NsFile is the actual file with its data
#[derive(Debug)]
pub(super) struct NsFile {
    /// Position within the file
    pos: AtomicUsize,
    /// File content
    data: Arc<RwLock<Vec<u8>>>,
}

impl Clone for NsFile {
    fn clone(&self) -> Self {
        NsFile {
            pos: AtomicUsize::new(self.pos.load(core::sync::atomic::Ordering::SeqCst)),
            data: self.data.clone(),
        }
    }
}

impl NsFile {
    pub fn new() -> Self {
        NsFile {
            pos: AtomicUsize::new(0),
            data: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

/// Implement `NsNode` operations for `NsFile`
impl NsNode for NsFile {
    fn get_type(&self) -> NsNodeType {
        NsNodeType::File
    }
}

/// Implement `NsNodeFile` operations for `NsFile`
impl NsNodeFile for NsFile {
    fn get_handle(&self, _opt: OpenOptions) -> Result<Box<dyn NsOpenFile>, Errno> {
        Ok(Box::new(NsFile {
            pos: AtomicUsize::new(0),
            data: self.data.clone(),
        }))
    }
}

/// Implement `FilePointer` operations for `NsFile`
impl NsOpenFile for NsFile {

    fn read(&self, buf: &mut [u8]) -> Result<usize, Errno> {
        let guard = self.data.read();
        let vec: &[u8] = guard.as_ref();
        let pos: usize = self.pos.load(SeqCst);

        if pos >= vec.len() {
            return Ok(0);
        }

        let len;
        if vec.len() - pos < buf.len() {
            len = vec.len() - pos
        } else {
            len = buf.len()
        }

        buf[0..len].clone_from_slice(&vec[pos..pos + len]);
        self.pos.fetch_add(len, SeqCst);
        Ok(len)
    }

    fn write(&self, buf: &[u8]) -> Result<usize, Errno> {
        let mut guard = self.data.write();
        let vec: &mut Vec<u8> = guard.as_mut();
        let pos = self.pos.load(SeqCst);

        if pos + buf.len() > vec.len() {
            vec.resize(pos + buf.len(), 0);
        }

        vec[pos..pos + buf.len()].clone_from_slice(buf);

        self.pos.fetch_add( buf.len(), SeqCst);

        Ok(buf.len())
    }

    fn size(&self) -> usize {
        let guard = self.data.read();
        let vec: &[u8] = guard.as_ref();
        vec.len() as usize
    }

    fn seek(&self, offset: usize, origin: SeekOrigin) -> Result<usize, Errno> {
        match origin {
            SeekOrigin::Start => {
                self.pos.store(offset, SeqCst);
                Ok(offset)
            }
            SeekOrigin::End => {
                let guard = self.data.read();
                let ref vec: &Vec<u8> = guard.as_ref();
                let data = vec.len() + offset;
                self.pos.store(data, SeqCst);
                Ok(data)
            }
            SeekOrigin::Current => {
                let pos: i64 = self.pos.load(SeqCst) as i64 + offset as i64;
                if pos >= 0 {
                    self.pos.store(pos as usize, SeqCst);
                    Ok(pos as usize)
                } else {
                    Err(Errno::EINVAL)
                }
            }
        }
    }
}