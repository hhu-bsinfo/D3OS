use alloc::string::{String, ToString};
use num_enum::{FromPrimitive, IntoPrimitive};
use syscall::{SystemCall, syscall};

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

static BUFFER_LEN: usize = 64;

pub fn build_info(info_type: BuildInfo) -> String {
    let mut buffer: [u8; BUFFER_LEN] = [0; BUFFER_LEN];

    let written_len = syscall(
        SystemCall::MapSystemInfo,
        &[
            buffer.as_mut_ptr() as usize,
            buffer.len(),
            info_type as usize,
        ],
    )
    .expect("Unable to map build info");

    String::from_utf8_lossy(&buffer[..written_len]).to_string()
}
