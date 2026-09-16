use alloc::vec::Vec;
use core::marker::PhantomData;
use core::mem;
use core::ops::{Deref, DerefMut};
use core3::io;
use rdma::AccessFlags;
use crate::pd::ProtectionDomain;

#[cfg(feature = "serialize")]
use bincode::{Decode, Encode};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};


/// A (local) memory region that has been registered for use with RDMA.
pub struct LocalMemoryRegion<'pd, T> {
    pd: &'pd ProtectionDomain<'pd>,
    metadata: MemoryRegionMetadata,
    data: Vec<T>,
}

pub struct MemoryRegionMetadata {
    pub(crate) handle: u32,
    pub(crate) lkey: u32,
    pub(crate) rkey: u32,
}

unsafe impl<'pd, T> Send for LocalMemoryRegion<'pd, T> {}
unsafe impl<'pd, T> Sync for LocalMemoryRegion<'pd, T> {}


impl<'pd, T> Deref for LocalMemoryRegion<'pd, T> {
    type Target = [T];
    fn deref(&self) -> &Self::Target {
        &self.data[..]
    }
}

impl<'pd, T> DerefMut for LocalMemoryRegion<'pd, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data[..]
    }
}

impl<'pd, T> LocalMemoryRegion<'pd, T> {
    pub fn new(pd: &'pd ProtectionDomain, mut data: Vec<T>, access_flags: AccessFlags) -> io::Result<Self>{
        let metadata = pd.ctx.inner.reg_mr(pd.pd, data.as_mut_ptr() as *mut _,
            data.len() * mem::size_of::<T>(),
            access_flags,
        )?;
        Ok(Self {
            pd,
            metadata,
            data,
        })
    }

    /// Get the remote authentication used to allow direct remote access to this memory region.
    pub fn remote(&mut self) -> RemoteMemoryRegion<T> {
        RemoteMemoryRegion {
            addr: self.data.as_mut_ptr() as u64,
            len: self.data.len() * mem::size_of::<T>(),
            rkey: self.metadata.rkey,
            phantom: PhantomData {},
        }
    }
    
    pub fn lkey(&self) -> u32 {
        self.metadata.lkey
    }
}

/// A (remote) memory region that has been registered for use with RDMA.
///
/// Having this information authorizes direct memory access to a memory region.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[cfg_attr(feature = "serialize", derive(Encode, Decode))]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct RemoteMemoryRegion<T> {
    /// the remote pointer
    pub addr: u64,
    /// the length
    pub len: usize,
    /// the remote key
    pub rkey: u32,
    /// This holds the type.
    pub phantom: PhantomData<T>,
}

