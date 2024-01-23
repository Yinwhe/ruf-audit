use basic_usages::external::fxhash::FxHashMap as HashMap;
use basic_usages::external::semver::Version;
use basic_usages::ruf_check_info::CondRufs;

use cargo_lock::dependency::graph::{EdgeDirection, Graph, NodeIndex};
use cargo_lock::Lockfile;
use cargo_metadata::semver::VersionReq;
use cargo_metadata::MetadataCommand;
use petgraph::visit::EdgeRef;
use tame_index::external::reqwest;
// use tame_index::index::FileLock;
use tame_index::utils::flock::LockOptions;
use tame_index::{IndexLocation, KrateName, SparseIndex};

use crate::cargo;

use super::DepManager;

impl DepManager<'_> {
    /// Create a new DepManager from current configurations.
    pub fn new() -> Result<Self, String> {
        let lockfile = Lockfile::load("Cargo.lock")
            .map_err(|e| format!("cannot build DepManager, load lock file fails: {}", e))?;

        let dep_tree = lockfile
            .dependency_tree()
            .map_err(|e| format!("cannot build DepManager, load lock file fails: {}", e))?;

        let metadata = MetadataCommand::new()
            .exec()
            .map_err(|e| format!("cannot build DepManager, load metadata fails: {}", e))?;

        let mut local_crates = HashMap::default();
        for pkg in metadata.packages {
            // Local crates
            if pkg.source.is_none() {
                let name_ver = format!("{}@{}", pkg.name, pkg.version);
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
            .map_err(|e| format!("cannot build DepManager, setup index fails: {}", e))?;
        let client = reqwest::blocking::Client::builder()
            .build()
            .map_err(|e| format!("cannot build DepManager, setup reqwest client fails: {}", e))?;

        let index = tame_index::index::RemoteSparseIndex::new(index, client);
        let lock = LockOptions::cargo_package_lock(None)
            .map_err(|e| format!("cannot build DepManager, setup lock fails: {}", e))?;

        Ok(Self {
            // lockfile,
            index,
            lock,

            dep_tree,
            local_crates,
        })
    }

    /// Use in down fix, get candidates that match it's dependents' version req.
    pub fn get_candidates(&self, pkgnx: NodeIndex) -> Result<HashMap<Version, CondRufs>, String> {
        let pkg = &self.graph()[pkgnx];

        // if local, no candidates
        let name_ver = format!("{}@{}", pkg.name, pkg.version);
        if self.local_crates.contains_key(&name_ver) {
            return Ok(HashMap::default());
        }

        let parents = self.get_dep_parent(pkgnx);
        assert!(parents.len() >= 1, "Fatal, root has no parents");

        // collect version req
        let mut version_reqs = Vec::new();
        for p in parents {
            let p = &self.graph()[p];
            let meta = self.get_package_reqs(p.name.as_str(), p.version.to_string().as_str())?;
            let req = meta
                .into_iter()
                .find(|(name, _)| name == pkg.name.as_str())
                .expect("Fatal, cannot find dependency in parent package")
                .1;
            version_reqs.push(req);
        }

        let candidates = basic_usages::ruf_db_usage::get_rufs_with_crate_name(pkg.name.as_str())?;
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

    /// Used in up fix, get candidates for the dependent, which has older version req
    /// to the dep package.
    pub fn get_candidates_up_fix(
        &self,
        pkgnx: NodeIndex,
        dep_pkgnx: NodeIndex,
    ) -> Result<HashMap<Version, CondRufs>, String> {
        let pkg = &self.graph()[pkgnx];
        let dep_pkg = &self.graph()[dep_pkgnx];

        let pkg_name = pkg.name.as_str();
        let pkg_ver = pkg.version.to_string();

        let dep_name = dep_pkg.name.as_str();
        let candidates = self.get_candidates(pkgnx)?;

        // We find out version with looser req to dep_pkgnx
        let mut res = HashMap::default();
        let cur_req = self
            .get_package_reqs(pkg_name, pkg_ver.as_str())?
            .into_iter()
            .find(|(name, _)| name == dep_name)
            .expect("Fatal, cannot find dependency in parent package")
            .1;

        for cad in candidates {
            let reqs = self.get_package_reqs(pkg_name, cad.0.to_string().as_str())?;

            if let Some((_, req)) = reqs.into_iter().find(|(name, _)| name == dep_name) {
                if req != cur_req {
                    res.insert(cad.0, cad.1);
                }
            } else {
                // dep is moved, of course ok
                res.insert(cad.0, cad.1);
            }
        }

        Ok(res)
    }

    /// Update package version in Cargo.lock, using cargo update subcommand.
    pub fn update_pkg(
        &mut self,
        name: &str,
        cur_ver: &str,
        update_ver: &str,
    ) -> Result<(), String> {
        let name_ver = format!("{name}@{cur_ver}");

        let mut cargo = cargo();
        cargo.args(["update", &name_ver, "--precise", update_ver]);

        let output = cargo.output().expect("Fatal, execute cargo update fails");
        if !output.status.success() {
            return Err(format!(
                "execute cargo update fails: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        self.update_dep_tree()?;

        Ok(())
    }

    pub fn root(&self) -> NodeIndex {
        let roots = self.dep_tree.roots();
        assert!(roots.len() == 1); // When will this not be 1 ?
        roots[0]
    }

    pub fn graph(&self) -> &Graph {
        self.dep_tree.graph()
    }

    fn get_package_reqs(&self, name: &str, ver: &str) -> Result<Vec<(String, VersionReq)>, String> {
        // check whether local crates
        let name_ver = format!("{name}@{ver}");
        if self.local_crates.contains_key(&name_ver) {
            return Ok(self.local_crates[&name_ver].clone());
        }

        // else we fetch from remote
        let krate: KrateName = name
            .try_into()
            .expect(&format!("Fatal, cannot convert {name} to KrateName"));

        // search local first, lock crates index is needed
        let lock = self
            .lock
            .lock(|_| None)
            .expect("Fatal, cannot get file lock");
        let res = self
            .index
            .cached_krate(krate.clone(), &lock)
            .map_err(|e| format!("cannot get package metadata from index: {}", e))?;

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
            .krate(krate, true, &lock)
            .map_err(|e| format!("cannot get package metadata from index: {}", e))?;

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
            "cannot get package {name}-{ver} metadata from index",
        ))
    }

    fn get_dep_parent(&self, depnx: NodeIndex) -> Vec<NodeIndex> {
        self.dep_tree
            .graph()
            .edges_directed(depnx, EdgeDirection::Incoming)
            .map(|edge| edge.source())
            .collect()
    }

    fn update_dep_tree(&mut self) -> Result<(), String> {
        let lockfile = Lockfile::load("Cargo.lock")
            .map_err(|e| format!("cannot build DepManager, load lock file fails: {}", e))?;

        let dep_tree = lockfile
            .dependency_tree()
            .map_err(|e| format!("cannot build DepManager, load lock file fails: {}", e))?;

        self.dep_tree = dep_tree;

        Ok(())
    }
}