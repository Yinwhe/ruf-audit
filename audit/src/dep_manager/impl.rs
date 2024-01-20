use basic_usages::external::fxhash::FxHashMap as HashMap;
use basic_usages::external::semver::Version;
use basic_usages::ruf_check_info::CondRufs;

use cargo_lock::dependency::graph::{EdgeDirection, Graph, NodeIndex};
use cargo_lock::Lockfile;
use cargo_metadata::semver::VersionReq;
use cargo_metadata::MetadataCommand;
use petgraph::visit::EdgeRef;
use tame_index::external::reqwest;
use tame_index::index::FileLock;
use tame_index::{IndexLocation, KrateName, SparseIndex};

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

        let metadata = MetadataCommand::new()
            .exec()
            .map_err(|e| format!("Fatal, cannot build DepManager, load metadata fails: {}", e))?;

        let mut local_crates = HashMap::default();

        for pkg in metadata.packages {
            // Local crates
            if pkg.source.is_none() {
                let name_ver = format!("{}-{}", pkg.name, pkg.version);
                let deps = pkg
                    .dependencies
                    .into_iter()
                    .map(|dep| {
                        let req = dep.req;
                        (dep.name, req)
                    })
                    .collect();
                local_crates.insert(name_ver, deps);
            }
        }

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
            // lockfile,
            index,
            lock,

            dep_tree,
            local_crates,
        })
    }

    pub fn get_candidates(&self, pkgnx: NodeIndex) -> Result<HashMap<Version, CondRufs>, String> {
        let pkg = &self.graph()[pkgnx];
        let candidates = basic_usages::ruf_db_usage::get_rufs_with_crate_name(pkg.name.as_str())?;

        let parents = self.get_dep_parent(pkgnx);
        assert!(parents.len() >= 1, "Fatal, root has no parents");

        // collect version req
        let mut version_reqs = Vec::new();
        for p in parents {
            let meta = self.get_package_reqs(p)?;
            let req = meta
                .into_iter()
                .find(|(name, _)| name == pkg.name.as_str())
                .expect("Fatal, cannot find dependency in parent package")
                .1;
            version_reqs.push(req);
        }

        // We filter out version that:
        // 1. match its dependents' version req
        // 2. smaller than current version
        let candidates = candidates
            .into_iter()
            .filter(|(key, _)| {
                version_reqs
                    .iter()
                    .all(|req| req.matches(key) && key < &pkg.version)
            })
            .collect();

        Ok(candidates)
    }

    pub fn root(&self) -> NodeIndex {
        let roots = self.dep_tree.roots();
        assert!(roots.len() == 1); // When will this not be 1 ?
        roots[0]
    }

    pub fn graph(&self) -> &Graph {
        self.dep_tree.graph()
    }

    pub fn update_pkg() {}

    fn get_package_reqs(&self, pkgnx: NodeIndex) -> Result<Vec<(String, VersionReq)>, String> {
        let pkg = &self.graph()[pkgnx];
        let name = pkg.name.as_str();
        let ver = pkg.version.to_string();

        // check whether local crates
        let key = format!("{name}-{ver}");
        if self.local_crates.contains_key(&key) {
            return Ok(self.local_crates[&key].clone());
        }

        // else we fetch from remote
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
            return Ok(iv
                .dependencies()
                .into_iter()
                .map(|dep| {
                    let req = dep.version_requirement();
                    (dep.crate_name().to_string(), req)
                })
                .collect());
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
            return Ok(iv
                .dependencies()
                .into_iter()
                .map(|dep| {
                    let req = dep.version_requirement();
                    (dep.crate_name().to_string(), req)
                })
                .collect());
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
