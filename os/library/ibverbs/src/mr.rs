use crate::pd::ProtectionDomain;
use alloc::vec::Vec;
use core::mem;
use core::ops::{Bound, Deref, DerefMut, RangeBounds};
use core3::io;
use rdma::{AccessFlags, ScatterGatherEntry};

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

    /// Create a slice of the whole region that can be accessed remotely
    pub fn remote(&mut self) -> RemoteMemorySlice {
        RemoteMemorySlice {
            addr: self.data.as_mut_ptr() as u64,
            len: self.data.len() * size_of::<T>(),
            rkey: self.metadata.rkey,
        }
    }

    /// Create a slice to the region that can be accessed remotely
    #[inline]
    pub fn slice(&self, bounds: impl RangeBounds<usize>) -> ScatterGatherEntry {
        let (addr, length) = calc_addr_len(bounds, self.data.as_ptr() as u64,self.data.len() * size_of::<T>());
        assert!(length < 1 << 31, "The slice was larger than 2Gb, which is the max that fits in a single SGE");
        ScatterGatherEntry {
            addr,
            length: length as u32,
            lkey: self.metadata.lkey,
        }
    }

    #[inline]
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
pub struct RemoteMemorySlice {
    /// the remote pointer
    pub addr: u64,
    /// the length
    pub len: usize,
    /// the remote key
    pub rkey: u32,
}

impl RemoteMemorySlice {
    pub fn slice(&self, bounds: impl RangeBounds<usize>) -> RemoteMemorySlice {
        let (addr, len) = calc_addr_len(bounds, self.addr, self.len);
        RemoteMemorySlice {
            addr,
            len,
            rkey: self.rkey
        }
    }
}

fn calc_addr_len(bounds: impl RangeBounds<usize>, addr: u64, bytes_len: usize) -> (u64, usize) {
    let start = match bounds.start_bound() {
        Bound::Included(i) => *i,
        Bound::Excluded(i) => *i + 1,
        Bound::Unbounded => 0,
    };
    let end = match bounds.end_bound() {
        Bound::Included(i) => *i + 1,
        Bound::Excluded(i) => *i,
        Bound::Unbounded => bytes_len,
    };
    assert!(start < end);
    assert!(start <= bytes_len);
    assert!(end <= bytes_len);
    let addr = addr + start as u64;
    let len = end - start;
    (addr, len)
}

