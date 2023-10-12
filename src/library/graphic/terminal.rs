use crate::library::io::stream::{InputStream, OutputStream};

pub trait Terminal: OutputStream + InputStream {}