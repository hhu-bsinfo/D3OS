use d3os::{array_from_slice, copy_from_slice_sizechecked};
use crate::ipc_message::{IpcMessage, IpcMessageContent, IpcMessageData, IpcMessageType};

macro_rules! data_as_any_number {
    ($msg:expr, $out_type:ty) => {
        match $msg.data() {
            IpcMessageData::Empty => Ok(0 as $out_type),
            IpcMessageData::Raw { data, size } => Ok(<$out_type>::from_ne_bytes(array_from_slice!(data[..*size]))),
            IpcMessageData::Usize(n) => Ok((*n).saturating_cast()),
            IpcMessageData::Isize(n) => Ok((*n).saturating_cast()),
            IpcMessageData::I32(n) => Ok((*n).saturating_cast()),
            IpcMessageData::F64(n) => Ok(((*n) as u64).saturating_cast()),
            IpcMessageData::UsizeSlice { data, size: _ } => Ok((data.first().copied().unwrap_or_default()).saturating_cast()),
            IpcMessageData::IsizeSlice { data, size: _ } => Ok((data.first().copied().unwrap_or_default()).saturating_cast()),
            IpcMessageData::I32Slice { data, size: _ } => Ok((data.first().copied().unwrap_or_default()).saturating_cast()),
            IpcMessageData::F64Slice { data, size: _ } => Ok((data.first().copied().unwrap_or_default() as u64).saturating_cast()),
            _ => Err($msg.data()),
        }
    };
}

impl IpcMessage {
    
    pub fn data_as_usize(&self) -> Result<usize, &IpcMessageData> {
        match self.data() {
            IpcMessageData::Usize(n) => Ok(*n),
            IpcMessageData::UsizeSlice { data, size: _ } => data.first().copied().ok_or(self.data()),
            _ => Err(self.data())
        }
    }
    
    pub fn data_as_isize(&self) -> Result<isize, &IpcMessageData> {
        match self.data() {
            IpcMessageData::Isize(n) => Ok(*n),
            IpcMessageData::IsizeSlice { data, size: _ } => data.first().copied().ok_or(self.data()),
            _ => Err(self.data())
        }
    }
    
    pub fn data_as_i32(&self) -> Result<i32, &IpcMessageData> {
        match self.data() {
            IpcMessageData::I32(n) => Ok(*n),
            IpcMessageData::I32Slice { data, size: _ } => data.first().copied().ok_or(self.data()),
            _ => Err(self.data())
        }
    }
    
    pub fn data_as_f64(&self) -> Result<f64, &IpcMessageData> {
        match self.data() {
            IpcMessageData::F64(n) => Ok(*n),
            IpcMessageData::F64Slice { data, size: _ } => data.first().copied().ok_or(self.data()),
            _ => Err(self.data()),
        }
    }
    
    pub fn data_as_usize_unchecked(&self) -> Result<usize, &IpcMessageData> {
        data_as_any_number!(self, usize)
    }
    
    pub fn data_as_isize_unchecked(&self) -> Result<isize, &IpcMessageData> {
        data_as_any_number!(self, isize)
    }
    
    pub fn data_as_i32_unchecked(&self) -> Result<i32, &IpcMessageData> {
        data_as_any_number!(self, i32)
    }
    
    pub fn data_as_f64_unchecked(&self) -> f64 {
        match self.data() {
            IpcMessageData::Empty => f64::NAN,
            IpcMessageData::Raw { data, size } => f64::from_ne_bytes(array_from_slice!(data[..*size])),
            IpcMessageData::Usize(n) => (*n) as f64,
            IpcMessageData::Isize(n) => (*n) as f64,
            IpcMessageData::I32(n) => (*n) as f64,
            IpcMessageData::F64(n) => *n,
            IpcMessageData::UsizeSlice { data, size: _ } => data.first().copied().unwrap_or_default() as f64,
            IpcMessageData::IsizeSlice { data, size: _ } => data.first().copied().unwrap_or_default() as f64,
            IpcMessageData::I32Slice { data, size: _ } => data.first().copied().unwrap_or_default() as f64,
            IpcMessageData::F64Slice { data, size: _ } => data.first().copied().unwrap_or_default(),
            _ => f64::NAN,
        }
    }


    pub fn data_as_usize_slice(&self) -> Result<&[usize], &IpcMessageData> {
        match self.data() {
            IpcMessageData::UsizeSlice { data, size } => Ok(data[..*size].as_ref()),
            IpcMessageData::Usize(n) => Ok(core::array::from_ref(n)),
            _ => Err(self.data())
        }
    }

    pub fn data_as_isize_slice(&self) -> Result<&[isize], &IpcMessageData> {
        match self.data() {
            IpcMessageData::IsizeSlice { data, size } => Ok(data[..*size].as_ref()),
            IpcMessageData::Isize(n) => Ok(core::array::from_ref(n)),
            _ => Err(self.data())
        }
    }

    pub fn data_as_i32_slice(&self) -> Result<&[i32], &IpcMessageData> {
        match self.data() {
            IpcMessageData::I32Slice { data, size } => Ok(data[..*size].as_ref()),
            IpcMessageData::I32(n) => Ok(core::array::from_ref(n)),
            _ => Err(self.data())
        }
    }

    pub fn data_as_f64_slice(&self) -> Result<&[f64], &IpcMessageData> {
        match self.data() {
            IpcMessageData::F64Slice { data, size } => Ok(data[..*size].as_ref()),
            IpcMessageData::F64(n) => Ok(core::array::from_ref(n)),
            _ => Err(self.data())
        }
    }
}


impl IpcMessageContent for &[usize] {
    fn to_bytes(&self) -> &[u8] {
        let ptr = self.as_ptr() as *const u8;
        let l = core::mem::size_of_val(*self);
        // alignment should be safe, because usize alignment should always include u8 alignment
        unsafe { core::slice::from_raw_parts(ptr, l) }
    }
    fn msg_type(&self) -> IpcMessageType {
        IpcMessageType::UsizeSlice
    }

    fn to_data(&self) -> IpcMessageData {
        IpcMessageData::UsizeSlice { data: array_from_slice!(self), size: self.len() }
    }
}

impl<const N: usize> IpcMessageContent for &[usize; N] {
    fn to_bytes(&self) -> &[u8] {
        let ptr = self.as_ptr() as *const u8;
        let l = core::mem::size_of_val(*self);
        // alignment should be safe, because usize alignment should always include u8 alignment
        unsafe { core::slice::from_raw_parts(ptr, l) }
    }
    fn msg_type(&self) -> IpcMessageType {
        IpcMessageType::UsizeSlice
    }

    fn to_data(&self) -> IpcMessageData {
        IpcMessageData::UsizeSlice { data: array_from_slice!(self), size: self.len() }
    }
}

impl IpcMessageContent for usize {
    fn to_bytes(&self) -> &[u8] {
        // cast an usize into an u8 array
        let ptr = core::ptr::from_ref(self) as *const u8;
        let l = size_of::<usize>();
        unsafe { core::slice::from_raw_parts(ptr, l) }
    }
    fn msg_type(&self) -> IpcMessageType {
        IpcMessageType::Usize
    }
    
    fn to_data(&self) -> IpcMessageData {
        IpcMessageData::Usize(*self)
    }
}


impl IpcMessageContent for &[isize] {
    fn to_bytes(&self) -> &[u8] {
        let ptr = self.as_ptr() as *const u8;
        let l = core::mem::size_of_val(*self);
        // alignment should be safe, because usize alignment should always include u8 alignment
        unsafe { core::slice::from_raw_parts(ptr, l) }
    }
    fn msg_type(&self) -> IpcMessageType {
        IpcMessageType::IsizeSlice
    }
    
    fn to_data(&self) -> IpcMessageData {
        IpcMessageData::IsizeSlice { data: core::array::from_fn(|i| self.get(i).copied().unwrap_or_default()), size: self.len() }
    }
}

impl<const N: usize> IpcMessageContent for &[isize; N] {
    fn to_bytes(&self) -> &[u8] {
        let ptr = self.as_ptr() as *const u8;
        let l = core::mem::size_of_val(*self);
        // alignment should be safe, because usize alignment should always include u8 alignment
        unsafe { core::slice::from_raw_parts(ptr, l) }
    }
    fn msg_type(&self) -> IpcMessageType {
        IpcMessageType::IsizeSlice
    }
    
    fn to_data(&self) -> IpcMessageData {
        IpcMessageData::IsizeSlice { data: array_from_slice!(self), size: self.len() }
    }
}

impl IpcMessageContent for isize {
    fn to_bytes(&self) -> &[u8] {
        // cast an isize into an u8 array
        let ptr = core::ptr::from_ref(self) as *const u8;
        let l = size_of::<isize>();
        unsafe { core::slice::from_raw_parts(ptr, l) }
    }
    fn msg_type(&self) -> IpcMessageType {
        IpcMessageType::Isize
    }
    
    fn to_data(&self) -> IpcMessageData {
        IpcMessageData::Isize(*self)
    }
}


impl IpcMessageContent for &[i32] {
    fn to_bytes(&self) -> &[u8] {
        let ptr = self.as_ptr() as *const u8;
        let l = core::mem::size_of_val(*self);
        // alignment should be safe, because usize alignment should always include u8 alignment
        unsafe { core::slice::from_raw_parts(ptr, l) }
    }
    fn msg_type(&self) -> IpcMessageType {
        IpcMessageType::I32Slice
    }
    
    fn to_data(&self) -> IpcMessageData {
        IpcMessageData::I32Slice { data: array_from_slice!(self), size: self.len() }
    }
}

impl<const N: usize> IpcMessageContent for &[i32; N] {
    fn to_bytes(&self) -> &[u8] {
        let ptr = self.as_ptr() as *const u8;
        let l = core::mem::size_of_val(*self);
        // alignment should be safe, because usize alignment should always include u8 alignment
        unsafe { core::slice::from_raw_parts(ptr, l) }
    }
    fn msg_type(&self) -> IpcMessageType {
        IpcMessageType::I32Slice
    }
    
    fn to_data(&self) -> IpcMessageData {
        IpcMessageData::I32Slice { data: array_from_slice!(self), size: self.len() }
    }
}

impl IpcMessageContent for i32 {
    fn to_bytes(&self) -> &[u8] {
        // cast an i32 into an u8 array
        let ptr = core::ptr::from_ref(self) as *const u8;
        let l = size_of::<i32>();
        unsafe { core::slice::from_raw_parts(ptr, l) }
    }
    fn msg_type(&self) -> IpcMessageType {
        IpcMessageType::I32
    }
    
    fn to_data(&self) -> IpcMessageData {
        IpcMessageData::I32(*self)
    }
}


impl IpcMessageContent for &[f64] {
    fn to_bytes(&self) -> &[u8] {
        let ptr = self.as_ptr() as *const u8;
        let l = core::mem::size_of_val(*self);
        // alignment should be safe, because usize alignment should always include u8 alignment
        unsafe { core::slice::from_raw_parts(ptr, l) }
    }
    fn msg_type(&self) -> IpcMessageType {
        IpcMessageType::F64Slice
    }
    
    fn to_data(&self) -> IpcMessageData {
        IpcMessageData::F64Slice { data: array_from_slice!(self), size: self.len() }
    }
}

impl<const N: usize> IpcMessageContent for &[f64; N] {
    fn to_bytes(&self) -> &[u8] {
        let ptr = self.as_ptr() as *const u8;
        let l = core::mem::size_of_val(*self);
        // alignment should be safe, because usize alignment should always include u8 alignment
        unsafe { core::slice::from_raw_parts(ptr, l) }
    }
    fn msg_type(&self) -> IpcMessageType {
        IpcMessageType::F64Slice
    }
    
    fn to_data(&self) -> IpcMessageData {
        IpcMessageData::F64Slice { data: array_from_slice!(self), size: self.len() }
    }
}

impl IpcMessageContent for f64 {
    fn to_bytes(&self) -> &[u8] {
        // cast an f64 into an u8 array
        let ptr = core::ptr::from_ref(self) as *const u8;
        let l = size_of::<f64>();
        unsafe { core::slice::from_raw_parts(ptr, l) }
    }
    fn msg_type(&self) -> IpcMessageType {
        IpcMessageType::F64
    }
    
    fn to_data(&self) -> IpcMessageData {
        IpcMessageData::F64(*self)
    }
}