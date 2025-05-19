use alloc::{string::String, sync::Arc, vec::Vec};
use concurrent::thread;
use spin::Mutex;

/**
 * TODO proper docs
 * Cache args for build in threads (Currently threads don't support passing args directly)
 */

static CACHE: Mutex<Vec<(usize, Arc<Vec<String>>)>> = Mutex::new(Vec::new());

pub fn cache_args(id: usize, args: Vec<&str>) {
    let args = args.into_iter().map(String::from).collect();
    CACHE.lock().push((id, Arc::new(args)));
}

pub fn flush_args(id: usize) -> Arc<Vec<String>> {
    for _retry in [1..10] {
        if let Some(args_arc) = try_flush_args(id) {
            return args_arc;
        }
        thread::sleep(100);
    }
    panic!("No args registered for thread {}", id)
}

fn try_flush_args(id: usize) -> Option<Arc<Vec<String>>> {
    let mut registry = CACHE.lock();
    let (_, args_arc) = match registry.iter().position(|(stored_id, _)| *stored_id == id) {
        Some(index) => registry.swap_remove(index),
        None => return None,
    };
    Some(args_arc.clone())
}
