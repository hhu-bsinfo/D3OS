use log::error;
use syscall::return_vals::Errno;
use system_info::build_info::BuildInfo;

use crate::built_info;

pub fn sys_map_build_info(info_type: BuildInfo, buffer: *mut u8, buffer_len: usize) -> isize {
    if buffer.is_null() {
        error!("Unable to map build info, buffer is null");
        return Errno::EINVAL as isize;
    }

    let value: &str = match info_type {
        BuildInfo::PkgVersion => built_info::PKG_VERSION,
        BuildInfo::Profile => built_info::PROFILE,
        _ => {
            error!("Unable to map build info, received invalid build info type");
            return Errno::EINVAL as isize;
        }
    };

    let value_bytes = value.as_bytes();
    let value_len = value_bytes.len();

    if value_len > buffer_len {
        error!(
            "Unable to map build info, buffer was to small (required {}, received {})",
            value_len, buffer_len
        );
        return Errno::EINVAL as isize;
    }

    let des_buffer = unsafe { core::slice::from_raw_parts_mut(buffer, buffer_len) };
    des_buffer[..value_len].copy_from_slice(value_bytes);

    value_len as isize
}
