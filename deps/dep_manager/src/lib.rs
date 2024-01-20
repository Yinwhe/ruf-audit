use cargo_lock::{Lockfile, dependency::Tree};
use tame_index::index::{RemoteSparseIndex, FileLock};

mod r#impl;

// re-export
pub use petgraph;

pub struct DepManager {
    lockfile: Lockfile,
    index: RemoteSparseIndex,
    lock: FileLock,
    dep_tree: Tree,
}