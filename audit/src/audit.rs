use basic_usages::external::fxhash::{FxHashMap as HashMap, FxHashSet as HashSet};
use basic_usages::ruf_check_info::UsedRufs;

use cargo_lock::dependency::graph::NodeIndex;
use petgraph::visit::{self};

use crate::build_config::BuildConfig;
use crate::dep_manager::DepManager;
use crate::error::AuditError;
use crate::extract::extract;
use crate::{cargo, error_print, info_print, warn_print};

/// The main audit functions,
/// this function shall be called only once, at first layer.
pub fn audit(mut config: BuildConfig, queit: bool) -> i32 {
    // We check ruf usage first
    info_print!(
        queit,
        "Starting",
        "extract used rufs in current configurations"
    );
    let used_rufs = match extract(&mut config, queit) {
        Ok(used_rufs) => used_rufs,
        Err(err) => {
            error_print!(queit, &format!("extract used rufs fail: {err}"));
            return -1;
        }
    };
    // println!("[Debug] rufs: {:?}", used_rufs);

    // We fetch the used features, and then we shall check it
    if let Err(err) = check_rufs(config, used_rufs, queit) {
        error_print!(queit, &format!("we cannot fix rufs issue: {err}"));
        return -1;
    }

    info_print!(queit, "Finished", "currently no rufs issue found");
    return 0;
}

fn check_rufs(
    mut config: BuildConfig,
    used_rufs: HashMap<String, UsedRufs>,
    queit: bool,
) -> Result<(), AuditError> {
    info_print!(queit, "Starting", "analyzing used rufs");

    let mut dm = DepManager::new()?;

    // check all used rufs
    if used_rufs.iter().all(|(_, rufs)| config.rufs_usable(rufs)) {
        // no rufs issue found (but other problem may exists)
        return Ok(());
    }

    // or we have to things to fix.
    if !config.is_quick_fix() {
        info_print!(queit, "\tIssue", "ruf issues exist, try dep tree fix first");
        // if not quick fix, we will do this, since dep tree fix can be hard and slow
        let err = match fix_with_dep(&mut config, used_rufs, &mut dm, queit) {
            Ok(()) => {
                info_print!(
                    queit,
                    "\tFixed",
                    "all ruf issues are fixed, usable depenency tree are written in `Cargo.lock`"
                );
                return Ok(());
            }
            Err(e) => e,
        };

        warn_print!(
            queit,
            "\tFailed",
            &format!("we cannot fix ruf issues through changing dep tree: {err}")
        );
    }

    info_print!(
        queit,
        "\tIssue",
        "try fix by changing rustc with minimal dep tree"
    );
    let err = match fix_with_rustc(&mut config, &mut dm, queit) {
        Ok(rustc_version) => {
            info_print!(
                queit,
                "\tFixed",
                &format!("rustc 1.{rustc_version}.* can be used in current configurations")
            );
            return Ok(());
        }
        Err(e) => e,
    };
    warn_print!(
        queit,
        "\tFailed",
        &format!("we cannot fix ruf issues through chaning rustc version: {err}")
    );

    return Err(err);
}

fn fix_with_dep(
    config: &mut BuildConfig,
    mut used_rufs: HashMap<String, UsedRufs>,
    dm: &mut DepManager,
    queit: bool,
) -> Result<(), AuditError> {
    // this algo will ends, because we have a finite number of crates
    // and each time we slim down the candidates.
    loop {
        let graph = dm.graph();
        let root = dm.root();
        let mut issued_depnx = None;

        let mut bfs = visit::Bfs::new(&graph, root);
        while let Some(nx) = bfs.next(&graph) {
            let node = &graph[nx];
            // though we record all used crates, `Cargo.lock` seems to record optional deps too.
            if let Some(rufs) = used_rufs.get(node.name.as_str()) {
                if !config.rufs_usable(&rufs) {
                    issued_depnx = Some(nx);
                    break;
                }
            }
        }

        if issued_depnx.is_none() {
            // no rufs issue found (but other problem may exists)
            return Ok(());
        }

        // We found a ruf issue
        let issued_depnx = issued_depnx.unwrap();
        let issued_dep = &graph[issued_depnx];

        warn_print!(
            queit,
            "\tDetect",
            &format!("dep '{}' cause ruf issues, try fixing", issued_dep.name)
        );

        // Canditate versions, restricted by semver, no rufs checks
        let candidate_vers = dm.get_candidates(issued_depnx)?;
        // println!("[Debug] candidates: {:?}", candidate_vers);

        // here we check rufs
        let mut usable_vers = vec![];
        for cad in candidate_vers {
            let used_rufs = config.filter_rufs(issued_dep.name.as_str(), cad.1)?;
            if config.rufs_usable(&used_rufs) {
                usable_vers.push(cad.0);
            }
        }

        // donw fix first
        let choose = if config.is_newer_fix() {
            usable_vers.into_iter().max()
        } else {
            usable_vers.into_iter().min()
        };
        if let Some(fix_ver) = choose {
            let name = issued_dep.name.to_string();
            let ver = issued_dep.version.to_string();
            let fix_ver = fix_ver.to_string();

            info_print!(
                queit,
                "\tFixing",
                &format!("changing {name}@{ver} to {name}@{fix_ver}")
            );
            // Here previous graph and issue_dep are droped, we have to copy rather than borrow.
            dm.update_pkg(&name, &ver, &fix_ver)?;

            info_print!(queit, "\tFixing", "rechecking ruf issues");
            used_rufs = extract(config, queit)?;
        } else {
            // No usable version, maybe parents' version not compatible, we do up fix.
            warn_print!(queit, "\tFixing", &format!("no candidates found, do upfix"));
            match up_fix(config, issued_depnx, dm, queit) {
                Ok(_) => {
                    info_print!(queit, "\tUpfixing", "rechecking ruf issues");
                    used_rufs = extract(config, queit)?;
                }
                Err(e) => {
                    if !e.is_unexpected() {
                        // TODO: Print fail paths
                    }
                    return Err(e);
                }
            }
        }
    }
}

fn up_fix(
    config: &mut BuildConfig,
    issued_depnx: NodeIndex,
    dm: &mut DepManager,
    queit: bool,
) -> Result<(), AuditError> {
    // check which parent shall be updated.
    let p_reqs = match dm.req_by(&issued_depnx) {
        Some(reqs) => reqs,
        None => {
            // already root crates, up fix fails
            return Err(AuditError::Functionality(format!(
                "up fix failed, reaching root"
            )));
        }
    };

    let mut fix_one = false;

    assert!(p_reqs.len() > 0, "no depdents found");
    for p in &p_reqs {
        let p_pkg = &dm.graph()[p.to_owned()];
        let p_candidates_vers = dm.get_candidates_up_fix(p.clone(), issued_depnx.clone())?;

        let mut usable_vers = vec![];
        for cad in p_candidates_vers {
            let used_rufs = config.filter_rufs(p_pkg.name.as_str(), cad.1)?;
            if config.rufs_usable(&used_rufs) {
                usable_vers.push(cad.0);
            }
        }

        let choose = if config.is_newer_fix() {
            usable_vers.into_iter().max()
        } else {
            usable_vers.into_iter().min()
        };
        if let Some(fix_ver) = choose {
            let name = p_pkg.name.to_string();
            let ver = p_pkg.version.to_string();
            let fix_ver = fix_ver.to_string();

            info_print!(
                queit,
                "\tUpfixing",
                &format!("changing {name}@{ver} to {name}@{fix_ver}")
            );
            // Here previous graph and issue_dep are droped, we have to copy rather than borrow.
            dm.update_pkg(&name, &ver, &fix_ver)?;

            fix_one = true;
            break;
        }
        // dependent cannot be fixed too, check next p first, if exists
    }

    if fix_one {
        // make sure each time we are making progress
        return Ok(());
    }

    // or, maybe we have to nested upfix
    for p in p_reqs {
        match up_fix(config, p, dm, queit) {
            Ok(_) => {
                fix_one = true;
                break;
            }
            Err(e) => {
                if e.is_unexpected() {
                    return Err(e);
                }
            }
        }
    }

    if !fix_one {
        // up fix fails
        return Err(AuditError::Functionality(format!(
            "up fix fails at current layer"
        )));
    }

    Ok(())
}

fn fix_with_rustc(
    config: &mut BuildConfig,
    _dm: &mut DepManager,
    queit: bool,
) -> Result<u32, AuditError> {
    // we restore the dep tree to its release configurations, which is, all oldest.
    let mut cargo = cargo();
    cargo.args(["generate-lockfile", "-Z", "minimal-versions"]);
    let output = cargo
        .output()
        .map_err(|e| AuditError::Unexpected(format!("cannot run cargo generate-lockfile: {e}")))?;

    if !output.status.success() {
        return Err(AuditError::Unexpected(format!(
            "failed to minimal generate-lock"
        )));
    }
    // loop {
    //     let graph = dm.graph();
    //     let root = dm.root();
    //     let mut update = None;

    //     let mut bfs = visit::Bfs::new(&graph, root);
    //     while let Some(nx) = bfs.next(&graph) {
    //         let candidates = dm.get_candidates(nx)?;
    //         if !candidates.is_empty() {
    //             let oldest = candidates.into_iter().map(|cad| cad.0).min().unwrap();
    //             update = Some((nx, oldest));
    //         }
    //     }

    //     if let Some(update) = update {
    //         let pkg_name = &graph[update.0].name.to_string();
    //         let pkg_ver = &graph[update.0].version.to_string();
    //         // graph drops here
    //         dm.update_pkg(
    //             pkg_name.as_str(),
    //             pkg_ver.to_string().as_str(),
    //             update.1.to_string().as_str(),
    //         )?;
    //     } else {
    //         break;
    //     }
    // }

    // recheck all used rufs
    let used_rufs = extract(config, queit)?;

    let mut usable_rustc = HashSet::from_iter(0..=63);
    for rufs in used_rufs.into_values() {
        let rustc_versions = config.usable_rustc_for_rufs(&rufs);
        if rustc_versions.is_empty() {
            return Err(AuditError::Functionality(
                "cannot find usable rustc version for current configurations".to_string(),
            ));
        }
        usable_rustc = usable_rustc
            .intersection(&rustc_versions)
            .cloned()
            .collect();
    }

    usable_rustc.iter().max().cloned().ok_or_else(|| {
        AuditError::Functionality(
            "cannot find usable rustc version for current configurations".to_string(),
        )
    })
}

pub fn test(mut config: BuildConfig) -> i32 {
    // we test no fix first
    fn show_result(result: (bool, bool, bool, bool)) {
        println!(
            "\n===({},{},{},{})===\n",
            result.0, result.1, result.2, result.3
        );
    }

    let mut result = (true, true, true, true);

    info_print!(false, "Test 1", "no fix ruf usage");
    let used_rufs = match extract(&mut config, true) {
        Ok(used_rufs) => used_rufs,
        Err(err) => {
            error_print!(false, &format!("extract used rufs fail: {err}"));
            return -1;
        }
    };

    if used_rufs.iter().all(|(_, rufs)| config.rufs_usable(rufs)) {
        info_print!(false, "Test 1", "ruf usage ok");
        show_result(result);
        return 0;
    }
    result.0 = false;

    info_print!(false, "Test 2", "no dep tree fix, only rustc fix");
    let mut usable_rustc = HashSet::from_iter(0..=63);
    for rufs in used_rufs.into_values() {
        let ur = config.usable_rustc_for_rufs(&rufs);
        usable_rustc = usable_rustc.intersection(&ur).cloned().collect();
    }

    if !usable_rustc.is_empty() {
        info_print!(
            false,
            "Test 2",
            &format!("rustc fix: {:?}", usable_rustc.into_iter().max())
        );
        show_result(result);
        return 0;
    }
    result.1 = false;

    info_print!(false, "Test 3", "no rustc fix, only min dep tree");
    let mut cargo = cargo();
    cargo.args(["generate-lockfile", "-Z", "minimal-versions"]);
    if matches!(
        cargo.output().map(|output| output.status.success()),
        Err(_) | Ok(false)
    ) {
        error_print!(false, &format!("cannot generate minimal dep tree"));
        return -1;
    }

    let used_rufs = match extract(&mut config, true) {
        Ok(used_rufs) => used_rufs,
        Err(err) => {
            error_print!(false, &format!("extract used rufs fail: {err}"));
            return -1;
        }
    };

    if used_rufs.iter().all(|(_, rufs)| config.rufs_usable(rufs)) {
        info_print!(false, "Test 3", "ruf usage ok");
        show_result(result);
        return 0;
    }
    result.2 = false;

    info_print!(false, "Test 4", "min dep tree, and rustc fix");
    let mut usable_rustc = HashSet::from_iter(0..=63);
    for rufs in used_rufs.into_values() {
        let ur = config.usable_rustc_for_rufs(&rufs);
        usable_rustc = usable_rustc.intersection(&ur).cloned().collect();
    }

    if !usable_rustc.is_empty() {
        info_print!(
            false,
            "Test 4",
            &format!("rustc fix: {:?}", usable_rustc.into_iter().max())
        );
        show_result(result);
        return 0;
    }
    result.3 = false;

    show_result(result);
    return 0;
}
