use alloc::string::String;
use syscall::{SystemCall, syscall};

#[derive(Debug)]
pub enum BuildInfo {
    PkgVersion = 0,
    Profile = 1,
}

static BUFFER_LEN: usize = 32;

pub fn build_info(info_type: BuildInfo) -> String {
    let mut buffer = [0; BUFFER_LEN];

    let written_len = syscall(
        SystemCall::MapSystemInfo,
        &[
            info_type as usize,
            &mut buffer as *mut _ as usize,
            buffer.len(),
        ],
    )
    .expect("Unable to map build info");

    String::from_utf8(buffer[..written_len].to_vec())
        .expect("Buffer contains invalid UTF8 characters")
}

// TODO#8 cache info after it has been mapped once (build info doesn't change in runtime)
