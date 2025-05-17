use alloc::{
    format,
    string::{String, ToString},
};
use chrono::DateTime;

use super::system_info::SystemInfo;

pub fn create_banner_string(info: &SystemInfo) -> String {
    let version = format!(
        "v{} ({} - O{})",
        info.pkg_version, info.profile, info.opt_level
    );
    let git_ref = &info.git_head_ref;
    let git_commit = &info.git_commit_hash_short;
    let build_date = match DateTime::parse_from_rfc2822(&info.build_time_utc) {
        Ok(date_time) => date_time.format("%Y-%m-%d %H:%M:%S").to_string(),
        Err(_) => "Unknown".to_string(),
    };

    format!(
        include_str!("banner.txt"),
        version,
        git_ref.rsplit("/").next().unwrap_or(&git_ref),
        git_commit,
        build_date,
        info.rustic_version
            .split_once("(")
            .unwrap_or((&info.rustic_version, ""))
            .0
            .trim(),
        info.bootloader_name
    )
}
