use alloc::string::{String, ToString};
use log::error;
use syscall::return_vals::Errno;
use system_info::build_info::BuildInfo;

use crate::{boot_info, built_info};

/// SystemCall implementation for SystemCall::MapSystemInfo.
/// Exposes build infos to User-Space.
///
/// Author: Sebastian Keller
pub extern "sysv64" fn sys_map_build_info(address: *mut u8, length: usize, info_type: usize) -> isize {
    if address.is_null() {
        error!("Unable to map build info, buffer is null");
        return Errno::EINVAL as isize;
    }

    let info_type = BuildInfo::from(info_type);
    let value = map_build_info(info_type);
    let value_bytes = value.as_bytes();
    let value_len = value_bytes.len();
    if value_len > length {
        error!(
            "Unable to map build info, buffer was to small (required {}, received {})",
            value_len, length
        );
        return Errno::EINVAL as isize;
    }

    let buffer = unsafe { core::slice::from_raw_parts_mut(address, length) };
    buffer[..value_len].copy_from_slice(value_bytes);
    value_len as isize
}

/// Helper function.
/// Maps BuildInfo type to its value.
///
/// Author: Sebastian Keller
fn map_build_info(info_type: BuildInfo) -> String {
    let info = match info_type {
        BuildInfo::CiPlatform => built_info::CI_PLATFORM.unwrap_or("Unknown"),
        BuildInfo::PkgVersion => built_info::PKG_VERSION,
        BuildInfo::PkgVersionMajor => built_info::PKG_VERSION_MAJOR,
        BuildInfo::PkgVersionMinor => built_info::PKG_VERSION_MINOR,
        BuildInfo::PkgVersionPatch => built_info::PKG_VERSION_PATCH,
        BuildInfo::PkgVersionPre => built_info::PKG_VERSION_PRE,
        BuildInfo::PkgAuthors => built_info::PKG_AUTHORS,
        BuildInfo::PkgName => built_info::PKG_NAME,
        BuildInfo::PkgDescription => built_info::PKG_DESCRIPTION,
        BuildInfo::PkgHomepage => built_info::PKG_HOMEPAGE,
        BuildInfo::PkgLicense => built_info::PKG_LICENSE,
        BuildInfo::PkgRepository => built_info::PKG_REPOSITORY,
        BuildInfo::Target => built_info::TARGET,
        BuildInfo::Host => built_info::HOST,
        BuildInfo::Profile => built_info::PROFILE,
        BuildInfo::Rustc => built_info::RUSTC,
        BuildInfo::Rustdoc => built_info::RUSTDOC,
        BuildInfo::OptLevel => built_info::OPT_LEVEL,
        BuildInfo::NumJobs => &built_info::NUM_JOBS.to_string(),
        BuildInfo::Debug => &built_info::DEBUG.to_string(),
        // BuildInfo::Features => todo!(), NOT IMPLEMENTED
        BuildInfo::FeaturesStr => built_info::FEATURES_STR,
        // BuildInfo::FeaturesLowercase => todo!(), NOT IMPLEMENTED
        BuildInfo::FeaturesLowercaseStr => built_info::FEATURES_LOWERCASE_STR,
        BuildInfo::RustcVersion => built_info::RUSTC_VERSION,
        BuildInfo::RustdocVersion => built_info::RUSTDOC_VERSION,
        BuildInfo::CfgTargetArch => built_info::CFG_TARGET_ARCH,
        BuildInfo::CfgEndian => built_info::CFG_ENDIAN,
        BuildInfo::CfgEnv => built_info::CFG_ENV,
        BuildInfo::CfgFamily => built_info::CFG_FAMILY,
        BuildInfo::CfgOs => built_info::CFG_OS,
        BuildInfo::CfgPointerWidth => built_info::CFG_POINTER_WIDTH,
        BuildInfo::GitVersion => built_info::GIT_VERSION.unwrap_or("Unknown"),
        BuildInfo::GitDirty => &built_info::GIT_DIRTY.map(|b| b.to_string()).unwrap_or("".to_string()),
        BuildInfo::GitHeadRef => built_info::GIT_HEAD_REF.unwrap_or("Unknown"),
        BuildInfo::GitCommitHash => built_info::GIT_COMMIT_HASH.unwrap_or("Unknown"),
        BuildInfo::GitCommitHashShort => built_info::GIT_COMMIT_HASH_SHORT.unwrap_or("Unknown"),
        BuildInfo::BuiltTimeUtc => built_info::BUILT_TIME_UTC,
        BuildInfo::BootloaderName => &boot_info().bootloader_name,
    };
    info.to_string()
}
