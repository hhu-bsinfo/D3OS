fn main() {
    println!("cargo:rerun-if-changed=src,../../library/libc/src/");

    cc::Build::new()
        .file("src/hello.c")
        .flag("-nostdlib")
        .flag("-ffreestanding")
        .flag("-fno-stack-protector")
        .flag("-fpic")
        .compile("hello");
}