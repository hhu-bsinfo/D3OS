#![no_std]
extern crate alloc;

use alloc::vec;
use alloc::vec::Vec;
use naming::shared_types::{OpenOptions, Capability};
use naming::{mkfifo, open, read, write, ROOT};
use naming::{SHARED_PIPE};
use capabilities::share_naming_object;
use concurrent::thread::{current, sleep};
#[allow(unused_imports)]
use runtime::*;
use syscall::return_vals::Errno;
use terminal::{print, println};
use terminal::write::print;

type FileHandle = usize;

#[derive(Copy, Clone)]

pub struct FileClient {
    pipe_cap: Capability,
}

impl FileClient {
    pub fn new(handle: usize) -> Self {
        FileClient {
            pipe_cap: Capability::new(handle),
        }
    }

    pub fn write_file(&self, letter: u8) -> Result<FileHandle, Errno> {
        let Ok(openpipe) = open(self.pipe_cap, OpenOptions::READWRITE) else {
            println!("---fileclient2: failed to open pipe");
            return Err(Errno::ENOENT);
        };

        // Send write command (1) and the letter
        write(openpipe, &[1, letter])?;

        // Read back handle
        let mut handle_buf = [0u8; 1];

        sleep(500); // Sleep a bit to ensure the server has processed the command and is ready to send the response
        read(openpipe, &mut handle_buf)?;

        Ok(handle_buf[0].clone() as usize)
    }

    pub fn read_file(&self, handle: FileHandle) -> Result<u8, Errno> {
        let Ok(openpipe) = open(self.pipe_cap, OpenOptions::READWRITE) else {
            println!("---fileclient2: failed to open pipe");
            return Err(Errno::ENOENT);
        };

        // Send read command (2) and the handle
        write(openpipe, &[2, handle as u8])?;
        sleep(2000); // Sleep a bit to ensure the server has processed the command and is ready to send the response
        // Read back the letter
        let mut letter = [5u8];
        read(openpipe, &mut letter)?;

        Ok(letter[0].clone())
    }

}


// Example usage
#[unsafe(no_mangle)]
pub fn main(){
//     let Some(mut client) = FileClient::connect() else {
//         print!("fileclient2: failed to connect to file server\n");
//         return;
//     };
//
//     let handle = client.write_file(b'B').expect("Failed to write letter");
//     print!("Wrote 'B' with handle: {}\n", handle);

    sleep(20000); // Sleep a bit to ensure the server is ready to receive the shared capability


    let client = FileClient::new(2);

    // Read it back
    let Ok(letter) = client.read_file(1) else {
        print!("Failed to read file with handle: {}\n", 1);
        return;
    };
    print!("Read back: {}\n", letter as char);

}