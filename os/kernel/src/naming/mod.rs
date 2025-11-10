pub mod api;
pub mod stat;

mod open_objects;
mod tmpfs;
mod lookup;
mod traits;

pub mod virtual_objects;

pub use traits::{PseudoFileObject, PseudoType};

use open_objects::{create_open_table_entry, free_open_table_entry, get_open_table_entry};
use traits::{NamedObject, PseudoFile};