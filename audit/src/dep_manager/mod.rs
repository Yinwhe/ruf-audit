use std::cell::RefCell;

use basic_usages::external::fxhash::FxHashMap as HashMap;

use cargo_lock::dependency::Tree;
use cargo_lock::dependency::graph::NodeIndex;
use cargo_metadata::semver::VersionReq;
use tame_index::index::RemoteSparseIndex;
use tame_index::utils::flock::LockOptions;

mod r#impl;

pub struct DepManager<'long> {
    // lockfile: Lockfile,
    index: RemoteSparseIndex,
    lock: LockOptions<'long>,

    // updates each fresh
    dep_tree: Tree,
    req_by: RefCell<HashMap<NodeIndex, Vec<NodeIndex>>>,

    local_crates: HashMap<String, Vec<(String, VersionReq)>>,
}
