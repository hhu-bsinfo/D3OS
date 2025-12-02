use num_enum::{FromPrimitive, IntoPrimitive};
#[cfg(feature = "userspace")]
use syscall::{SystemCall, syscall};
#[cfg(feature = "userspace")]
use alloc::string::{String, ToString};

#[derive(Debug, PartialEq, IntoPrimitive, FromPrimitive)]
#[repr(usize)]
pub enum BuildInfo {
    #[num_enum(default)]
    CiPlatform = 0,
    PkgVersion = 1,
    PkgVersionMajor = 2,
    PkgVersionMinor = 3,
    PkgVersionPatch = 4,
    PkgVersionPre = 5,
    PkgAuthors = 6,
    PkgName = 7,
    PkgDescription = 8,
    PkgHomepage = 9,
    PkgLicense = 10,
    PkgRepository = 11,
    Target = 12,
    Host = 13,
    Profile = 14,
    Rustc = 15,
    Rustdoc = 16,
    OptLevel = 17,
    NumJobs = 18,
    Debug = 19,
    // Features = 20, NOT IMPLEMENTED
    FeaturesStr = 21,
    // FeaturesLowercase = 22, NOT IMPLEMENTED
    FeaturesLowercaseStr = 23,
    RustcVersion = 24,
    RustdocVersion = 25,
    CfgTargetArch = 26,
    CfgEndian = 27,
    CfgEnv = 28,
    CfgFamily = 29,
    CfgOs = 30,
    CfgPointerWidth = 31,
    GitVersion = 32,
    GitDirty = 33,
    GitHeadRef = 34,
    GitCommitHash = 35,
    GitCommitHashShort = 36,
    BuiltTimeUtc = 37,
    BootloaderName = 38,
}

#[cfg(feature = "userspace")]
static BUFFER_LEN: usize = 64;

/// Get build information from boot.
///
/// Author: Sebastian Keller
#[cfg(feature = "userspace")]
pub fn build_info(info_type: BuildInfo) -> String {
    let mut buffer: [u8; BUFFER_LEN] = [0; BUFFER_LEN];

    let written_len = syscall(
        SystemCall::MapSystemInfo,
        &[buffer.as_mut_ptr() as usize, buffer.len(), info_type as usize],
    )
    .expect("Unable to map build info");

    String::from_utf8_lossy(&buffer[..written_len]).to_string()
}

/*impl From<usize> for BuildInfo {
    fn from(b: usize) -> Self {
        match b {
            0  => BuildInfo::CiPlatform,
            1  => BuildInfo::PkgVersion,
            2  => BuildInfo::PkgVersionMajor,
            3  => BuildInfo::PkgVersionMinor,
            4  => BuildInfo::PkgVersionPatch,
            5  => BuildInfo::PkgVersionPre,
            6  => BuildInfo::PkgAuthors,
            7  => BuildInfo::PkgName,
            8  => BuildInfo::PkgDescription,
            9  => BuildInfo::PkgHomepage,
            10 => BuildInfo::PkgLicense,
            11 => BuildInfo::PkgRepository,
            12 => BuildInfo::Target,
            13 => BuildInfo::Host,
            14 => BuildInfo::Profile,
            15 => BuildInfo::Rustc,
            16 => BuildInfo::Rustdoc,
            17 => BuildInfo::OptLevel,
            18 => BuildInfo::NumJobs,
            19 => BuildInfo::Debug,
            21 => BuildInfo::FeaturesStr,
            23 => BuildInfo::FeaturesLowercaseStr,
            24 => BuildInfo::RustcVersion,
            25 => BuildInfo::RustdocVersion,
            26 => BuildInfo::CfgTargetArch,
            27 => BuildInfo::CfgEndian,
            28 => BuildInfo::CfgEnv,
            29 => BuildInfo::CfgFamily,
            30 => BuildInfo::CfgOs,
            31 => BuildInfo::CfgPointerWidth,
            32 => BuildInfo::GitVersion,
            33 => BuildInfo::GitDirty,
            34 => BuildInfo::GitHeadRef,
            35 => BuildInfo::GitCommitHash,
            36 => BuildInfo::GitCommitHashShort,
            37 => BuildInfo::BuiltTimeUtc,
            38 => BuildInfo::BootloaderName,
            _ => panic!("Invalid value {} for BuildInfo", b),
        }
    }
}*/