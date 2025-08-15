use std::{env, io};
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// This macro takes a directory path and generates code to create this directory in the D3OS naming service.
const MK_VFS_DIR_MACRO: &str =
"macro_rules! mk_vfs_dir {
    ($path:expr) => {
        naming::api::mkdir($path).expect(concat!(\"Failed to create \", $path));
    };
}";

/// This macro takes a path to a file in the VFS directory (on the host) and generate code to create this file in the D3OS naming service.
/// The file's contents are compiled into the kernel binary using `include_bytes!`.
const CREATE_VFS_FILE_MACRO: &str =
"macro_rules! create_vfs_file {
    ($path:expr) => {
            let rom = naming::api::open($path, OpenOptions::CREATE | OpenOptions::READWRITE).expect(concat!(\"Failed to create \", $path));
            naming::api::write(rom, include_bytes!(concat!(concat!(env!(\"PWD\"), \"/vfs\"), $path))).expect(concat!(\"Failed to write to \", $path));
    };
}";

/// Generates the function `create_vfs_files` which will create all directories and files in the VFS directory (on the host).
/// This function is called in the kernel's `main.rs` to set up the VFS structure in the D3OS naming service.
fn generate_vfs_code() {
    // Get the output directory for the generated code.
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("vfs.rs");

    // Get the VFS directory path
    let pwd = env::var("PWD").unwrap();
    let vfs_dir = Path::new(&pwd).join("vfs");
    let mut file = File::create(dest_path).unwrap();

    file.write("use ::naming::shared_types::OpenOptions;\n\n".as_bytes()).unwrap();

    file.write(MK_VFS_DIR_MACRO.as_bytes()).unwrap();
    file.write("\n\n".as_bytes()).unwrap();
    file.write(CREATE_VFS_FILE_MACRO.as_bytes()).unwrap();
    file.write("\n\n".as_bytes()).unwrap();

    file.write("pub fn create_vfs_files() {\n".as_bytes()).unwrap();
    visit_vfs_dir(vfs_dir.as_path(), vfs_dir.as_path(), &mut file).unwrap();
    file.write("}\n".as_bytes()).unwrap();
}

/// Recursive function to visit all subdirectories and files in the VFS directory.
/// It generates code that calls one of the macros defined above for each directory and file found.
fn visit_vfs_dir(dir: &Path, strip_prefix: &Path, code_file: &mut File) -> io::Result<()> {
    for entry in dir.read_dir()? {
        let entry = entry?;
        let path = entry.path();
        let stripped_path = path.strip_prefix(strip_prefix.to_str().unwrap()).unwrap();

        if path.is_dir() {
            code_file.write(format!("    mk_vfs_dir!(\"/{}\");\n", stripped_path.to_str().unwrap()).as_bytes())?;
            visit_vfs_dir(&path, strip_prefix, code_file)?;
        } else {
            code_file.write(format!("    create_vfs_file!(\"/{}\");\n", stripped_path.to_str().unwrap()).as_bytes())?;
        }
    }
    Ok(())
}

fn main() {
    // Write build information to a file
    built::write_built_file().expect("Failed to acquire build-time information");

    // Generate the VFS code
    generate_vfs_code();
}