/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: name_service_tests                                              ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Test all API functions of the naming service.                   ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, 1.8.2024, HHU                               ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/
use alloc::vec;
use ::log::info;

use crate::naming::name_service::{cont, del, dir, mkdir, mkentry, rename, stat};
use crate::naming::result::{Errno, Error};

///
/// Description:
///    Init naming service (called only once during booting)
///
pub fn run_tests() {
    info!("name_service: running tests");

    test_mkdir();
    test_mkentry();
    test_stat();
    test_cont();
    test_dir();
    test_del();
    test_rename();

    info!("name_service: all tests passed.");
}

///
/// Description:
///    Should create following directories: \
///    `/home/schoettner` \
///    `/home/ruhland`
///
fn test_mkdir() {
    // Create directory & subdirectory -> should work
    let path = "/home/schoettner";
    let r = mkdir(path);
    assert!(r == Ok(()), "mkdir(\"{}\") -> {:?}", path, r);

    // Create directory & subdirectory -> should work
    let path = "/home/ruhland";
    let r = mkdir("/home/ruhland");
    assert!(r == Ok(()), "mkdir(\"{}\") -> {:?}", path, r);

    // Create same directory & subdirectory -> should fail
    let r = mkdir("/home/schoettner");
    assert!(
        r == Err(Error::new(Errno::EEXIST)),
        "mkdir(\"{}\") -> {:?}",
        path,
        r
    );

    // Create same parent directory -> should fail
    let r = mkdir("/home");
    assert!(
        r == Err(Error::new(Errno::EEXIST)),
        "mkdir(\"{}\") -> {:?}",
        path,
        r
    );

    info!("   test 'mkdir':   passed");
}

///
/// Description:
///    Should create following containers: \
///    `/home/schoettner/brief.txt` \
///    `/home/ruhland/klausur.txt`
///
fn test_mkentry() {
    // Create container entry in existing directory -> should work
    let path = "/home/schoettner";
    let name = "brief.txt";
    let r = mkentry(path, name, vec![1, 1, 1, 1, 1]);
    assert!(r == Ok(()), "mkdir(\"{}\", \"{}\") -> {:?}", path, name, r);

    // Create container entry in existing directory -> should work
    let path = "/home/ruhland";
    let name = "klausur.txt";
    let r = mkentry(path, name, vec![1, 1, 1, 1, 1]);
    assert!(r == Ok(()), "mkdir(\"{}\", \"{}\") -> {:?}", path, name, r);

    // Create container in non-existing directory -> should fail
    let path = "/home/krakowski";
    let name = "brief.txt";
    let r = mkentry(path, name, vec![1, 1, 1, 1, 1]);
    assert!(
        r == Err(Error::new(Errno::ENOENT)),
        "mkdir(\"{}\", \"{}\") -> {:?}",
        path,
        name,
        r
    );

    // Create container entry which already exists -> should fail
    let path = "/home/schoettner";
    let name = "brief.txt";
    let r = mkentry(path, name, vec![1, 1, 1, 1, 1]);
    assert!(
        r == Err(Error::new(Errno::EEXIST)),
        "mkdir(\"{}\", \"{}\") -> {:?}",
        path,
        name,
        r
    );

    info!("   test 'mkentry': passed");
}

///
/// Description:
///    Testing `stat`
///
fn test_stat() {
    // Get stat from existing container  -> should work
    let pathname = "/home/schoettner/brief.txt";
    let r = stat(pathname);
    assert!(r.is_ok(), "stat(\"{}\") failed -> {:?}", pathname, r);

    // Get stat from existing directory  -> should work
    let pathname = "/home/schoettner";
    let r = stat(pathname);
    assert!(r.is_ok(), "stat(\"{}\") failed -> {:?}", pathname, r);

    // Get stat from non-existing container  -> should fail
    let pathname = "/home/ruhland/brief.txt";
    let r = stat(pathname);
    assert!(r.is_err(), "stat(\"{}\") did not fail -> {:?}", pathname, r);

    // Get stat from non-existing directory  -> should fail
    let pathname = "/home/krakowski";
    let r = stat(pathname);
    assert!(r.is_err(), "stat(\"{}\") did not fail -> {:?}", pathname, r);

    info!("   test 'stat':    passed");
}

///
/// Description:
///    Testing `cont`
///
fn test_cont() {
    // Get existing container  -> should work
    let pathname = "/home/schoettner/brief.txt";
    let r = cont(pathname);
    assert!(r.is_ok(), "cont(\"{}\") failed -> {:?}", pathname, r);

    // Get cont on existing directory -> should work
    let pathname = "/home/schoettner";
    let r = cont(pathname);
    assert!(r.is_err(), "cont(\"{}\") did not fail -> {:?}", pathname, r);

    // Get cont from non-existing container  -> should fail
    let pathname = "/home/ruhland/brief.txt";
    let r = cont(pathname);
    assert!(r.is_err(), "stat(\"{}\") did not fail -> {:?}", pathname, r);

    // Get cont from non-existing directory  -> should fail
    let pathname = "/home/krakowski";
    let r = cont(pathname);
    assert!(r.is_err(), "stat(\"{}\") did not fail -> {:?}", pathname, r);

    info!("   test 'cont':    passed");
}

///
/// Description:
///    Testing `dir`
///
fn test_dir() {
    // Get existing directory  -> should work
    let pathname = "/home/schoettner";
    let r = dir(pathname);
    assert!(r.is_ok(), "dir(\"{}\") failed -> {:?}", pathname, r);

    // Try to get non-existing directory -> should fail
    let pathname = "/home/krakowski";
    let r = cont(pathname);
    assert!(r.is_err(), "dir(\"{}\") did not fail -> {:?}", pathname, r);

    // Get existing container  -> should fail
    let pathname = "/home/schoettner/brief.txt";
    let r = dir(pathname);
    assert!(r.is_err(), "dir(\"{}\") did not fail -> {:?}", pathname, r);

    info!("   test 'dir':     passed");
}

///
/// Description:
///    Testing `del`
///
fn test_del() {
    // Delete non-existing entry -> should fail
    let pathname = "/home/schoettner2";
    let r = del(pathname);
    assert!(r.is_err(), "del(\"{}\") did not fail -> {:?}", pathname, r);

    // Delete existing but not empty directory -> should fail
    let pathname = "/home/schoettner";
    let r = del(pathname);
    assert!(r.is_err(), "del(\"{}\") did not fail -> {:?}", pathname, r);

    // Delete empty existing subdirectory -> should work
    let pathname = "/home/krakowski";
    let r = mkdir(pathname);
    assert!(r == Ok(()), "mkdir(\"{}\") -> {:?}", pathname, r);
    let r = del(pathname);
    assert!(r == Ok(()), "del(\"{}\") -> {:?}", pathname, r);

    // Delete existing container -> should work
    let pathname = "/home/schoettner/brief.txt";
    let r = del(pathname);
    assert!(r == Ok(()), "del(\"{}\") -> {:?}", pathname, r);

    info!("   test 'del':     passed");
}

///
/// Description:
///    Testing `rename`
///
fn test_rename() {
    // Create container entry in existing directory -> should work
    let path = "/home/schoettner";
    let name = "brief.txt";
    let r = mkentry(path, name, vec![1, 1, 1, 1, 1]);
    assert!(r == Ok(()), "mkdir(\"{}\", \"{}\") -> {:?}", path, name, r);

    // Rename existing container -> should work
    let pathname = "/home/schoettner/brief.txt";
    let new_name = "email.txt";
    let r = rename(pathname, new_name);
    assert!(
        r == Ok(()),
        "rename(\"{}\", \"{}\") -> {:?}",
        pathname,
        new_name,
        r
    );

    // Rename existing directory -> should work
    let pathname = "/home/schoettner";
    let new_name = "krakowski";
    let r = rename(pathname, new_name);
    assert!(
        r == Ok(()),
        "rename(\"{}\", \"{}\") -> {:?}",
        pathname,
        new_name,
        r
    );

    // Try to rename non-existing directory -> should fail
    // Rename existing directory -> should work
    let pathname = "/home/schoettner";
    let new_name = "krakowski";
    let r = rename(pathname, new_name);
    assert!(r.is_err(), "rename(\"{}\") did not fail -> {:?}", pathname, r);

    // Try to rename non-existing container -> should fail
    let pathname = "/home/krakowski/brief.txt";
    let new_name = "email.txt";
    let r = rename(pathname, new_name);
    assert!(r.is_err(), "rename(\"{}\") did not fail -> {:?}", pathname, r);

    info!("   test 'rename':  passed");
}
