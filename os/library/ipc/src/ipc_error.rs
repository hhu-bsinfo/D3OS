use syscall::return_vals;
use syscall::return_vals::Errno;

#[derive(Debug, PartialEq)]
pub enum IpcError {
    Generic = -1,
    Timeout = -2,
    Full = -3,
    Empty = -4,
    MsgTooLarge = -5,
    NoIpcQueue = -6,
    Invalid = -7,
}

impl IpcError {
    pub fn convert_syscall_result(syscall_result: Result<usize, Errno>) -> Result<usize, Self> {
        match syscall_result {
            Ok(ok) => Ok(ok),
            Err(err) => Err(err.into()),
        }
    }
}

impl From<IpcError> for isize {
    fn from(value: IpcError) -> Self {
        value as isize
    }
}

impl From<isize> for IpcError {
    fn from(value: isize) -> Self {
        match value {
            -1 => IpcError::Generic,
            -2 => IpcError::Timeout,
            -3 => IpcError::Full,
            -4 => IpcError::Empty,
            -5 => IpcError::MsgTooLarge,
            -6 => IpcError::NoIpcQueue,
            -7 => IpcError::Invalid,
            _ => IpcError::Generic,
        }
    }
}

impl From<Errno> for IpcError {
    fn from(value: Errno) -> Self {
        let ret_code = return_vals::convert_syscall_result_to_ret_code(Err(value));
        ret_code.into()
    }
}