use basic_usages::external::fxhash::FxHashMap as HashMap;

use cargo_lock::dependency::Tree;
// use cargo_lock::Lockfile;
use cargo_metadata::semver::VersionReq;
use tame_index::index::RemoteSparseIndex;
use tame_index::utils::flock::LockOptions;

mod r#impl;

pub struct DepManager<'long> {
    // lockfile: Lockfile,
    index: RemoteSparseIndex,
    lock: LockOptions<'long>,

    dep_tree: Tree,
    local_crates: HashMap<String, Vec<(String, VersionReq)>>,
}
