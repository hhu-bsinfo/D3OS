use core3::io;

pub type Result<T> = io::Result<T>;

/// Builds an `Other`-kind `io::Error` from a static message, for the ad hoc failures that don't
/// have a more specific `ErrorKind`.
pub fn other(msg: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::Other, msg)
}
