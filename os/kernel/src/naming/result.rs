/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: result                                                          ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Errors potentially returned by naming service functions.        ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, 23.7.2024, HHU                              ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

use core::fmt;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Errno {
    ENOENT = 2, 	    /* No such file or directory */
    EACCES = 13,	    /* Permission denied */
    EEXIST = 17,	    /* File/directory exists */
    ENOTDIR = 20,	    /* Not a directory */
    EINVAL = 22,	    /* Invalid argument */
    ENOTEMPTY = 90,	    /* Directory not empty */
}


pub type Result<T> = ::core::result::Result<T, Error>;

#[derive(PartialEq)]
enum ErrorMessage {
    StaticStr(&'static str),
}

#[derive(PartialEq)]
pub struct Error {
    errno: Errno,
    message: Option<ErrorMessage>,
}

impl Error {
    pub fn new(errno: Errno) -> Error {
        Error {
            errno,
            message: None,
        }
    }

    pub fn with_message(errno: Errno, message: &'static str) -> Error {
        Error {
            errno,
            message: Some(ErrorMessage::StaticStr(message)),
        }
    }

    pub fn errno(&self) -> Errno {
        self.errno
    }
}

impl fmt::Debug for Error {


    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(message) = self.message.as_ref() {
            match message {
                ErrorMessage::StaticStr(message) => {
                   write!(f, "[{:?}] {}", self.errno, message)
                }
            }
        } else {
            write!(f, "{:?}", self.errno)
        }
    }
}
