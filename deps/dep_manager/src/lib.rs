use basic_usages::external::fxhash::FxHashMap as HashMap;

use cargo_lock::dependency::Tree;
// use cargo_lock::Lockfile;
use cargo_metadata::semver::VersionReq;
use tame_index::index::{FileLock, RemoteSparseIndex};

mod r#impl;

// re-export
pub use petgraph;

pub struct DepManager {
    // lockfile: Lockfile,
    index: RemoteSparseIndex,
    lock: FileLock,

    dep_tree: Tree,
    local_crates: HashMap<String, Vec<(String, VersionReq)>>,
}
