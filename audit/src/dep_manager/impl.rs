use std::cell::RefCell;

use basic_usages::external::fxhash::FxHashMap as HashMap;
use basic_usages::external::semver::Version;
use basic_usages::ruf_check_info::CondRufs;

use cargo_lock::dependency::graph::{EdgeDirection, Graph, NodeIndex};
use cargo_lock::Lockfile;
use cargo_metadata::semver::VersionReq;
use cargo_metadata::MetadataCommand;
use petgraph::visit::EdgeRef;
use tame_index::external::reqwest;
use tame_index::utils::flock::LockOptions;
use tame_index::{IndexLocation, KrateName, SparseIndex};
// use tame_index::index::FileLock;

use crate::error::AuditError;
use crate::{spec_cargo, RUSTV};

use super::DepManager;

impl DepManager<'_> {
    /// Create a new DepManager from current configurations.
    pub fn new() -> Result<Self, AuditError> {
        let lockfile = Lockfile::load("Cargo.lock").map_err(|e| {
            AuditError::Unexpected(format!(
                "cannot build DepManager, load lock file fails: {e}",
            ))
        })?;

        let dep_tree = lockfile.dependency_tree().map_err(|e| {
            AuditError::Unexpected(format!("cannot build DepManager, load dep tree fails: {e}",))
        })?;

        let metadata = MetadataCommand::new()
            .env("RUSTUP_TOOLCHAIN", RUSTV)
            .exec()
            .map_err(|e| {
                AuditError::Unexpected(
                    format!("cannot build DepManager, load metadata fails: {e}",),
                )
            })?;

        let mut local_crates = HashMap::default();
        for pkg in metadata.packages {
            if pkg.source.is_none() {
                // no source means local
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

        let index = SparseIndex::new(IndexLocation::default()).map_err(|e| {
            AuditError::Unexpected(format!("cannot build DepManager, setup index fails: {e}"))
        })?;
        let client = reqwest::blocking::Client::builder().build().map_err(|e| {
            AuditError::Unexpected(format!(
                "cannot build DepManager, setup reqwest client fails: {e}",
            ))
        })?;

        let index = tame_index::index::RemoteSparseIndex::new(index, client);
        let lock = LockOptions::cargo_package_lock(None).map_err(|e| {
            AuditError::Unexpected(format!("cannot build DepManager, setup lock fails: {e}"))
        })?;

        let req_by = RefCell::new(HashMap::default());
        Ok(Self {
            index,
            lock,
            dep_tree,
            req_by,
            local_crates,
        })
    }

    /// Used in down fix, get candidates of a node that match it's parents' version req.
    pub fn get_candidates(
        &self,
        pkgnx: NodeIndex,
    ) -> Result<HashMap<Version, CondRufs>, AuditError> {
        let pkg = &self.graph()[pkgnx];

        // if local, no candidates
        let name_ver = format!("{}@{}", pkg.name, pkg.version);
        if self.local_crates.contains_key(&name_ver) {
            // println!("[Debug - get_candidates] local {name_ver}, no candidates");
            return Ok(HashMap::default());
        }

        let parents = self.get_dep_parent(pkgnx);
        assert!(parents.len() >= 1, "Fatal, root has no parents");

        let candidates = basic_usages::ruf_db_usage::get_rufs_with_crate_name(pkg.name.as_str())
            .map_err(|e| AuditError::Unexpected(format!("cannot get candidates, due to {e}")))?;

        // Early return.
        if candidates.is_empty() {
            return Ok(candidates);
        }
        // println!(
        //     "[Debug - get_candidates] get {name_ver}, candidats: {:?}",
        //     candidates.iter().map(|(v, _)| v.to_string()).collect::<Vec<String>>()
        // );

        // collect version req
        let mut version_reqs = Vec::new();
        for p in parents {
            let p_pkg = &self.graph()[p];
            let meta =
                self.get_package_reqs(p_pkg.name.as_str(), p_pkg.version.to_string().as_str())?;
            let req = meta
                .into_iter()
                .find(|(name, _)| name == pkg.name.as_str())
                .expect("Fatal, cannot find dependency in parent package")
                .1;
            // prepare for relaxing strict parents.
            let lowest = candidates
                .keys()
                .filter(|key| req.matches(key))
                .min()
                .cloned()
                .expect("Fatal, cannot find lowest allowing version");
            version_reqs.push((p, req, lowest));
        }

        // We assume parents who restricts the version most is the one not allow min_lowest,
        // and it shall be updated later, if we need up fix.
        // This assumption won't hold for all cases (cases with complex version req),
        // but most of the times it works.
        let min_lowest = version_reqs
            .iter()
            .map(|vr| &vr.2)
            .min()
            .expect("Fatal, no min version found");

        let mut req_by = None;
        for version_req in version_reqs.iter() {
            if version_req.2 > *min_lowest {
                req_by = Some(version_req.0);
                break;
            }
        }

        self.req_by
            .borrow_mut()
            .insert(pkgnx, req_by.expect("Fatal, no strict parent found"));

        // we choose candidates as:
        // 1. match its dependents' version req
        // 2. smaller than current version
        // we will record who restricts the version most, for later up fix.
        //
        // The ruf usability check will be done later, differ from design.
        let candidates = candidates
            .into_iter()
            .filter(|(ver, _)| {
                version_reqs
                    .iter()
                    .all(|(_, req, _)| req.matches(ver) && ver < &pkg.version)
            })
            .collect();

        Ok(candidates)
    }

    /// Used in up fix, same as [`get_candidates`], but get candidates for the dependent, which has older version req
    /// to the dep package.
    pub fn get_candidates_up_fix(
        &self,
        pkgnx: NodeIndex,
        dep_pkgnx: NodeIndex,
    ) -> Result<HashMap<Version, CondRufs>, AuditError> {
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
            .expect("Fatal, cannot find dependency in current parent package")
            .1;

        for cad in candidates {
            let reqs = self.get_package_reqs(pkg_name, cad.0.to_string().as_str())?;

            if let Some((_, req)) = reqs.into_iter().find(|(name, _)| name == dep_name) {
                // We take the assumption that, older verison shall have looser semver req, 
                // so if req differs, we assume it's a candidate, since semver comparision can be hard.
                if req != cur_req {
                    res.insert(cad.0, cad.1);
                }
            } else {
                // dep not found, possibily not used, thus ok.
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
    ) -> Result<(), AuditError> {
        let name_ver = format!("{name}@{cur_ver}");

        let mut cargo = spec_cargo(RUSTV);
        cargo.args(["update", &name_ver, "--precise", update_ver]);

        let output = cargo.output().expect("Fatal, execute cargo update fails");
        if !output.status.success() {
            return Err(AuditError::Unexpected(format!(
                "cannot update dep version, execute cargo update fails: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        self.update_dep_tree()?;
        self.req_by.borrow_mut().clear();

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

    pub fn req_by(&self, dep: &NodeIndex) -> Option<NodeIndex> {
        self.req_by.borrow().get(dep).cloned()
    }

    fn get_package_reqs(
        &self,
        name: &str,
        ver: &str,
    ) -> Result<Vec<(String, VersionReq)>, AuditError> {
        // check whether local crates
        let name_ver = format!("{name}@{ver}");
        if self.local_crates.contains_key(&name_ver) {
            return Ok(self.local_crates[&name_ver].clone());
        }

        // else we fetch from remote
        let krate: KrateName = name
            .try_into()
            .expect(&format!("Fatal, cannot convert {name} to KrateName"));

        // search local cache first, lock crates index is needed
        let lock = self
            .lock
            .lock(|_| None)
            .expect("Fatal, cannot get file lock");
        let res = self.index.cached_krate(krate.clone(), &lock).map_err(|e| {
            AuditError::Unexpected(format!(
                "cannot get package {name}-{ver} metadata from index: {e}"
            ))
        })?;

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
        let res = self.index.krate(krate, true, &lock).map_err(|e| {
            AuditError::Unexpected(format!(
                "cannot get package {name}-{ver} metadata from index: {e}"
            ))
        })?;

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

        return Err(AuditError::Unexpected(format!(
            "cannot get package {name}-{ver} metadata from index: cannot find in index"
        )));
    }

    fn get_dep_parent(&self, depnx: NodeIndex) -> Vec<NodeIndex> {
        self.dep_tree
            .graph()
            .edges_directed(depnx, EdgeDirection::Incoming)
            .map(|edge| edge.source())
            .collect()
    }

    fn update_dep_tree(&mut self) -> Result<(), AuditError> {
        let lockfile = Lockfile::load("Cargo.lock").map_err(|e| {
            AuditError::Unexpected(format!("cannot update dep tree, load lock file fails: {e}",))
        })?;

        let dep_tree = lockfile.dependency_tree().map_err(|e| {
            AuditError::Unexpected(format!("cannot update dep tree, load dep tree fails: {e}",))
        })?;

        self.dep_tree = dep_tree;

        Ok(())
    }
}
