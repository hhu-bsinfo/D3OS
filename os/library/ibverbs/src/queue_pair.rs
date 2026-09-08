use bincode::{Decode, Encode};

#[derive(PartialEq, Debug, Copy, Clone, Encode, Decode)]
pub enum WorkRequestOpcode {
    RdmaWrite,
    Send,
    RdmaRead,
}

