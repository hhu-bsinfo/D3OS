use std::{env, io};
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Generates the macro `create_all_vfs_files` which will create all directories and files in the VFS directory (on the host).
/// This function is called in the naming service's `api.rs` to set up the VFS structure in the TMPFS.
fn generate_vfs_code() {
    // Get the output directory for the generated code.
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("vfs.rs");

    // Get the VFS directory path
    let pwd = env::var("PWD").unwrap();
    let vfs_dir = Path::new(&pwd).join("vfs");
    let mut file = File::create(dest_path).unwrap();

    file.write("macro_rules! create_all_vfs_files {\n".as_bytes()).unwrap();
    file.write("    ($tmpfs:expr) => {\n".as_bytes()).unwrap();

    visit_vfs_dir(vfs_dir.as_path(), vfs_dir.as_path(), &mut file).unwrap();

    file.write("    };\n".as_bytes()).unwrap();
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
            visit_vfs_dir(&path, strip_prefix, code_file)?;
        } else {
            code_file.write(format!("        create_vfs_file!($tmpfs, \"/{}\");\n", stripped_path.to_str().unwrap()).as_bytes())?;
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