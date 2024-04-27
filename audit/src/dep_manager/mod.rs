//! This modules manipulate package's dependency trees.

use std::cell::RefCell;

use basic_usages::external::fxhash::FxHashMap as HashMap;

use cargo_lock::dependency::graph::NodeIndex;
use cargo_lock::dependency::Tree;
use cargo_metadata::semver::VersionReq;
use tame_index::index::RemoteSparseIndex;
use tame_index::utils::flock::LockOptions;

mod r#impl;

pub struct DepManager<'long> {
    // lockfile: Lockfile,
    index: RemoteSparseIndex,
    lock: LockOptions<'long>,

    /// dependency tree of the package; updated after each change.
    dep_tree: Tree,
    /// record parent with strictest semver reqs.
    req_by: RefCell<HashMap<NodeIndex, NodeIndex>>,

    /// local crates and thire semver reqs on the dependencies.
    local_crates: HashMap<String, Vec<(String, VersionReq)>>,
}
