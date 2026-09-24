use crate::ipc_message::{IpcMessage, IpcMessageContent, IpcMessageType};

macro_rules! deref_num_to_num {
    ($msg:expr, $in_type:ty) => {
        Ok(unsafe { IpcMessage::deref_data::<$in_type>($msg) })
    };
    ($msg:expr, $in_type:ty, $out_type:ty) => {
        Ok(unsafe { IpcMessage::deref_data::<$in_type>($msg) } as $out_type)
    };
}

macro_rules! deref_num_slice_to_num {
    ($msg:expr, $in_type:ty) => {
       Ok(unsafe { IpcMessage::ref_data_as_slice::<$in_type>($msg) }.first().copied().unwrap_or_default())
    };
    ($msg:expr, $in_type:ty, $out_type:ty) => {
        Ok(unsafe { IpcMessage::ref_data_as_slice::<$in_type>($msg) }.first().copied().map(|n| n as $out_type).unwrap_or_default())
    };
}

macro_rules! data_as_number {
    ($msg:expr, $out_type:ty) => {
        match IpcMessage::msg_type($msg) {
            IpcMessageType::Usize => deref_num_to_num!($msg, usize, $out_type),
            IpcMessageType::Isize => deref_num_to_num!($msg, isize, $out_type),
            IpcMessageType::I32 => deref_num_to_num!($msg, i32, $out_type),
            IpcMessageType::F64 => deref_num_to_num!($msg, f64, $out_type),
            IpcMessageType::UsizeSlice => deref_num_slice_to_num!($msg, usize, $out_type),
            IpcMessageType::IsizeSlice => deref_num_slice_to_num!($msg, isize, $out_type),
            IpcMessageType::I32Slice => deref_num_slice_to_num!($msg, i32, $out_type),
            IpcMessageType::F64Slice => deref_num_slice_to_num!($msg, f64, $out_type),
            _ => Err(IpcMessage::msg_type($msg)),
        }
    };
}

impl IpcMessage {
    
    pub fn data_as_usize(&self) -> Result<usize, IpcMessageType> {
        data_as_number!(self, usize)
    }
    
    pub fn data_as_isize(&self) -> Result<isize, IpcMessageType> {
        data_as_number!(self, isize)
    }
    
    pub fn data_as_i32(&self) -> Result<i32, IpcMessageType> {
        data_as_number!(self, i32)
    }
    
    pub fn data_as_f64(&self) -> Result<f64, IpcMessageType> {
        data_as_number!(self, f64)
    }


    pub fn data_as_usize_slice(&self) -> Result<&[usize], IpcMessageType> {
        match self.msg_type {
            IpcMessageType::Usize | IpcMessageType::UsizeSlice => Ok(unsafe { self.ref_data_as_slice() }),
            _ => Err(self.msg_type),
        }
    }

    pub fn data_as_isize_slice(&self) -> Result<&[isize], IpcMessageType> {
        match self.msg_type {
            IpcMessageType::Isize | IpcMessageType::IsizeSlice => Ok(unsafe { self.ref_data_as_slice() }),
            _ => Err(self.msg_type),
        }
    }

    pub fn data_as_i32_slice(&self) -> Result<&[i32], IpcMessageType> {
        match self.msg_type {
            IpcMessageType::I32 | IpcMessageType::I32Slice => Ok(unsafe { self.ref_data_as_slice() }),
            _ => Err(self.msg_type),
        }
    }

    pub fn data_as_f64_slice(&self) -> Result<&[f64], IpcMessageType> {
        match self.msg_type {
            IpcMessageType::F64 | IpcMessageType::F64Slice => Ok(unsafe { self.ref_data_as_slice() }),
            _ => Err(self.msg_type),
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
}