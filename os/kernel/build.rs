fn main() {
    // Write build information to a file
    built::write_built_file().expect("Failed to acquire build-time information");
}