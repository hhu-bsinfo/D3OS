## Adding a new app

1. Copy `hello` world app directory and insert it in the `application` directory.

2. Rename the directory to new name

3. Update following fields in the `Cargo.toml` file in your app directory
 - `name` of your app
 - `path` to the file with the `main`function. Should fit `src/name.rs`

4. Add your app in `D3OS/Cargo.toml`

5. Add your app in `D3OS/Makefile.toml` in list `[tasks.initrd]`
