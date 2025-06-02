use std::env;
use std::path::Path;

fn main() {
    let manifest = Path::new("../../kernel/Cargo.toml");
    let dst = Path::new(&env::var("OUT_DIR").expect("OUT_DIR not set")).join("built.rs");
    built::write_built_file_with_opts(Some(manifest), dst.as_path()).expect("Failed to acquire build-time information");
}
