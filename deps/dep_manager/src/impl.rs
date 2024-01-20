use basic_usages::external::fxhash::FxHashMap as HashMap;
use basic_usages::external::semver::Version;
use basic_usages::ruf_build_info::{CondRuf, UsedRufs};

use cargo_lock::dependency::graph::{EdgeDirection, Graph, NodeIndex};
use cargo_lock::Lockfile;
use petgraph::visit::EdgeRef;
use tame_index::external::reqwest;
use tame_index::index::FileLock;
use tame_index::{IndexLocation, IndexVersion, KrateName, SparseIndex};

use super::DepManager;

impl DepManager {
    pub fn new() -> Result<Self, String> {
        let lockfile = Lockfile::load("Cargo.lock").map_err(|e| {
            format!(
                "Fatal, cannot build DepManager, load lock file fails: {}",
                e
            )
        })?;

        let dep_tree = lockfile.dependency_tree().map_err(|e| {
            format!(
                "Fatal, cannot build DepManager, load lock file fails: {}",
                e
            )
        })?;

        let index = SparseIndex::new(IndexLocation::default())
            .map_err(|e| format!("Fatal, cannot build DepManager, setup index fails: {}", e))?;
        let client = reqwest::blocking::Client::builder().build().map_err(|e| {
            format!(
                "Fatal, cannot build DepManager, setup reqwest client fails: {}",
                e
            )
        })?;

        let index = tame_index::index::RemoteSparseIndex::new(index, client);
        let lock = FileLock::unlocked();

        Ok(Self {
            lockfile,
            dep_tree,
            index,
            lock,
        })
    }

    pub fn update_lockfile(&mut self) -> Result<(), String> {
        let lockfile = Lockfile::load("Cargo.lock")
            .map_err(|e| format!("Fatal, cannot update lockfile, load lock file fails: {}", e))?;
        self.lockfile = lockfile;

        Ok(())
    }

    pub fn get_candidates(
        &self,
        pkgnx: NodeIndex,
    ) -> Result<HashMap<Version, Vec<CondRuf>>, String> {
        let pkg = &self.graph()[pkgnx];
        let candidates = basic_usages::ruf_db_usage::get_rufs_with_crate_name(pkg.name.as_str())?;

        let parents = self.get_dep_parent(pkgnx);
        assert!(parents.len() >= 1, "Fatal, root has no parents");

        // collect version req
        let mut version_reqs = Vec::new();
        for p in parents {
            let meta = self.get_package_metadata(p)?;
            let req = meta
                .dependencies()
                .iter()
                .find(|dep| dep.crate_name() == pkg.name.as_str())
                .expect("Fatal, cannot find dependency in parent package")
                .version_requirement();
            version_reqs.push(req);
        }

        let candidates = candidates
            .into_iter()
            .filter(|(key, _)| version_reqs.iter().all(|req| req.matches(key)))
            .collect();

        Ok(candidates)
    }

    pub fn rufs_usable(rufs: &UsedRufs, rustc_version: u32) -> bool {
        assert!(rustc_version < basic_usages::ruf_lifetime::RUSTC_VER_NUM as u32);
        if rufs
            .0
            .iter()
            .filter(|ruf| {
                !basic_usages::ruf_lifetime::get_ruf_status(ruf, rustc_version).is_usable()
            })
            .count()
            > 0
        {
            return false;
        }

        return true;
    }

    pub fn root(&self) -> NodeIndex {
        let roots = self.dep_tree.roots();
        assert!(roots.len() == 1); // When will this not be 1 ?
        roots[0]
    }

    pub fn graph(&self) -> &Graph {
        self.dep_tree.graph()
    }

    fn get_package_metadata(&self, pkgnx: NodeIndex) -> Result<IndexVersion, String> {
        let pkg = &self.graph()[pkgnx];
        let name = pkg.name.as_str();
        let ver = pkg.version.to_string();

        let krate: KrateName = name
            .try_into()
            .expect(&format!("Fatal, cannot convert {name} to KrateName"));

        // search local first
        let res = self
            .index
            .cached_krate(krate.clone(), &self.lock)
            .map_err(|e| format!("Fatal, cannot get package metadata from index: {}", e))?;

        if let Some(iv) = res
            .map(|krate| krate.versions.into_iter().find(|iv| iv.version == ver))
            .flatten()
        {
            return Ok(iv);
        }

        // Or from remote
        let res = self
            .index
            .krate(krate, true, &self.lock)
            .map_err(|e| format!("Fatal, cannot get package metadata from index: {}", e))?;

        if let Some(iv) = res
            .map(|krate| krate.versions.into_iter().find(|iv| iv.version == ver))
            .flatten()
        {
            return Ok(iv);
        }

        Err(format!(
            "Fatal, cannot get package {name}-{ver} metadata from index",
        ))
    }

    fn get_dep_parent(&self, depnx: NodeIndex) -> Vec<NodeIndex> {
        self.dep_tree
            .graph()
            .edges_directed(depnx, EdgeDirection::Incoming)
            .map(|edge| edge.source())
            .collect()
    }
}
