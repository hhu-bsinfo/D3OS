use alloc::string::String;
use system_info::build_info::{BuildInfo, build_info};

pub struct SystemInfo {
    pub pkg_version: String,
    pub profile: String,
    pub opt_level: String,
    pub git_head_ref: String,
    pub git_commit_hash_short: String,
    pub build_time_utc: String,
    pub rustic_version: String,
    pub bootloader_name: String,
}

impl SystemInfo {
    pub fn new() -> Self {
        Self {
            pkg_version: build_info(BuildInfo::PkgVersion),
            profile: build_info(BuildInfo::Profile),
            opt_level: build_info(BuildInfo::OptLevel),
            git_head_ref: build_info(BuildInfo::GitHeadRef),
            git_commit_hash_short: build_info(BuildInfo::GitCommitHashShort),
            build_time_utc: build_info(BuildInfo::BuiltTimeUtc),
            rustic_version: build_info(BuildInfo::RustcVersion),
            bootloader_name: build_info(BuildInfo::BootloaderName),
        }
    }
}
