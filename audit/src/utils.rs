use std::env::current_exe;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::mem::MaybeUninit;
use std::process::Stdio;
use std::sync::Mutex;
use std::thread;

use basic_usages::external::fxhash::{FxHashMap as HashMap, FxHashSet as HashSet};
use dep_manager::petgraph::visit;
use dep_manager::DepManager;

use ansi_term::{Color, Style};
use lazy_static::lazy_static;
use log::debug;
use regex::Regex;

use basic_usages::ruf_build_info::{BuildInfo, UsedRufs};

use crate::config::AuditConfig;

use super::{cargo, scan};

// Some regex definitions.
lazy_static! {
    static ref RE_USEDFEATS: Regex = Regex::new(r"FDelimiter::\{(.*?)\}::FDelimiter").unwrap();
    static ref RE_BUILDINFO: Regex = Regex::new(r"BDelimiter::\{(.*?)\}::BDelimiter").unwrap();
    // static ref RE_INFOS: Regex =
    //     Regex::new(r"\s+Compiling\s+(\w+)\s+v([\d.]+)\s+\((.*?)\)").unwrap();
    static ref BOLD_RED: Style = Style::new().bold().fg(Color::Red);
    static ref BOLD_GREEN: Style = Style::new().bold().fg(Color::Green);
}

// lazy_static! {
//     static ref DEPMANAGER: Mutex<MaybeUninit<DepManager>> = Mutex::new(MaybeUninit::uninit());
// }

/// Create a wrapper around cargo and rustc,
/// this function shall be called only once, at first layer.
pub fn cargo_wrapper(config: AuditConfig) -> i32 {
    // We check ruf usage first
    let buildinfos = match rustc_wrapper() {
        Ok(used_rufs) => used_rufs,
        Err(err) => {
            error_print(&err);
            return -1;
        }
    };

    // We fetch the used features, and then we shall check it
    info_print("Finishing", "fetching used rufs");

    println!("[Debug] rufs: {:#?}", buildinfos);

    info_print("Starting", "analyzing used rufs");
    // Check rufs
    if let Err(err) = check_rufs(&config, buildinfos) {
        error_print(&err);
        return -1;
    }

    // info_print("Finishing", "currently no rufs issue found");
    // return 0;
    unimplemented!()
}

fn info_print(title: &str, msg: &str) {
    println!("{} {}", BOLD_GREEN.paint(title), msg);
}

fn error_print(msg: &str) {
    println!("{} {}", BOLD_RED.paint("error"), msg);
}

/// This function wrap rustc with our audit tool, and fetch
/// build configurations in the crate.
fn rustc_wrapper() -> Result<HashMap<String, BuildInfo>, String> {
    let mut cmd = cargo();
    cmd.arg("rustc");

    let path = current_exe().map_err(|_| format!("Fatal, cannot get current exe path"))?;
    cmd.env("RUSTC_WRAPPER", &path);

    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| format!("Fatal, cannot spawn cargo process"))?;

    // Listen to the cargo process
    let stdout = BufReader::new(child.stdout.take().unwrap());
    let stderr = BufReader::new(child.stderr.take().unwrap());

    info_print("Starting", "scan rufs");
    // Resolving stdout and stderr infos
    let stdout_handle = thread::spawn(move || {
        let mut buildinfos: HashMap<String, (HashSet<String>, HashSet<String>)> =
            HashMap::default();

        for line in stdout.lines() {
            debug!("Stdout: {line:?}");
            let line = line.expect("Fatal, get stdout line fails");

            if let Some(caps) = RE_BUILDINFO.captures(&line) {
                let info = BuildInfo::from(
                    caps.get(1)
                        .expect("Fatal, resolve buildinfo fails")
                        .as_str(),
                );
                let entry = buildinfos
                    .entry(info.crate_name)
                    .or_insert_with(|| (HashSet::default(), HashSet::default()));

                entry.0.extend(info.used_rufs.0.into_iter());
                entry.1.extend(info.cfg.into_iter());
            }
        }

        let buildinfos = buildinfos
            .into_iter()
            .map(|(crate_name, (used_rufs, cfgs))| {
                (
                    crate_name.clone(),
                    BuildInfo {
                        used_rufs: UsedRufs::new(used_rufs.into_iter().collect()),
                        crate_name,
                        cfg: cfgs.into_iter().collect(),
                    },
                )
            })
            .collect();

        return buildinfos;
    });

    let stderr_handle = thread::spawn(move || {
        for line in stderr.lines() {
            debug!("Stderr: {line:?}");
            let line = line.expect("Fatal, get stdout line fails");

            // compiling info, we can print it
            if let Some(index) = line.find("Compiling") {
                info_print("\tScanning", &line[index + 9..]);
            } else if line.trim().starts_with("error") {
                error_print(&line);
            }
        }
    });

    // Waiting for process to ends
    let buildinfos = stdout_handle
        .join()
        .expect("Fatal, cannot join stdout thread");
    let _ = stderr_handle
        .join()
        .expect("Fatal, cannot join stderr thread");

    let output = child
        .wait_with_output()
        .map_err(|_| format!("Fatal, fails to wait cargo process"))?;

    if !output.status.success() {
        return Err("Fatal, cargo process run fails".to_string());
    }

    return Ok(buildinfos);
}

fn check_rufs(config: &AuditConfig, buildinfos: HashMap<String, BuildInfo>) -> Result<(), String> {
    let dm = DepManager::new()?;

    // Check ruf usage in BFS mode.
    let graph = dm.graph();
    let root = dm.root();
    let mut issued_depnx = None;

    let mut bfs = visit::Bfs::new(&graph, root);
    while let Some(nx) = bfs.next(&graph) {
        let node = &graph[nx];
        let info = buildinfos.get(node.name.as_str()).expect(&format!(
            "Fatal, cannot find buildinfo for crate {}",
            node.name
        ));

        if !DepManager::rufs_usable(&info.used_rufs, config.get_rust_version()) {
            issued_depnx = Some(nx);
            break;
        }
    }

    if issued_depnx.is_none() {
        // no rufs issue found (but other problem may exists)
        return Ok(());
    }


    // We found a ruf issue
    let issued_depnx = issued_depnx.unwrap();
    let issued_dep = &graph[issued_depnx];

    info_print(
        "\tFixing",
        &format!("dep '{}' cause ruf issues", issued_dep.name),
    );

    let candidate_vers = dm.get_candidates(issued_depnx)?;

    println!("candidate_vers: {:?}", candidate_vers);
    unimplemented!()
    // // println!("[Debug] candidates: {:?}", candidate_vers);
    // let current_ver = issued_dep.version;
    // let mut usable_vers = vec![];

    // info_print(
    //     "\tFixing",
    //     &format!(
    //         "filter usable version from {} versions",
    //         candidate_vers.len()
    //     ),
    // );

    // for cad in &candidate_vers {
    //     if cad.0 == &current_ver {
    //         continue;
    //     }

    //     let is_usable = match check_candidate(config, &cad.1) {
    //         Ok(is_usable) => is_usable,
    //         Err(err) => {
    //             error_print(&err);
    //             continue;
    //         }
    //     };

    //     if is_usable {
    //         usable_vers.push(cad.0);
    //     }
    // }

    // info_print(
    //     "\tFixing",
    //     &format!(
    //         "found {} usable version: {:?}",
    //         usable_vers.len(),
    //         usable_vers
    //     ),
    // );

    // if usable_vers.is_empty() {
    //     return Err(format!(
    //         "Fatal, cannot find usable version for crate '{}'",
    //         issued_dep.name
    //     ));
    // }

    // unimplemented!()
}

// /// Check whether the candidate version is usable.
// fn check_candidate(config: &AuditConfig, rufs: &Vec<Ruf>) -> Result<bool, String> {
//     let tmp_rsfile_path = config.get_tmp_rsfile();
//     let mut content = String::new();
//     let mut used_rufs = vec![];
//     let livetime_tmp;

//     for ruf in rufs {
//         if let Some(cond) = &ruf.cond {
//             content.push_str(&format!(
//                 "#[cfg_attr({}, feature({}))]\n",
//                 cond, ruf.feature
//             ))
//         } else {
//             used_rufs.push(ruf.feature.as_str());
//         }
//     }

//     // We determine the rufs used first
//     if !content.is_empty() {
//         let mut f = File::open(tmp_rsfile_path).expect("Fatal, cannot open tmp file");
//         f.write_all(content.as_bytes())
//             .expect("Fatal, cannot write tmp file");

//         let output = scan()
//             .args(["--crate-name", "TODO", tmp_rsfile_path])
//             .env("LD_LIBRARY_PATH", config.get_rustlib_path())
//             .output()
//             .expect("Fatal, cannot fetch scanner output");

//         if !output.status.success() {
//             return Err(format!(
//                 "Fatal, cannot check candidate rufs: {}",
//                 String::from_utf8_lossy(&output.stderr)
//             ));
//         }

//         let stdout = String::from_utf8_lossy(&output.stdout);
//         if let Some(caps) = RE_FEATURES.captures(&stdout) {
//             livetime_tmp =
//                 CrateRufs::from(caps.get(1).expect("Fatal, resolve ruf fails").as_str()).rufs;
//             for ruf in &livetime_tmp {
//                 assert!(ruf.cond.is_none());
//                 used_rufs.push(ruf.feature.as_str());
//             }
//         }
//     }

//     if used_rufs
//         .iter()
//         .filter(|ruf| !lifetime::get_ruf_status(ruf, config.get_rust_version()).is_usable())
//         .count()
//         > 0
//     {
//         return Ok(false);
//     } else {
//         return Ok(true);
//     }
// }
