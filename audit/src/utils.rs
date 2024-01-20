use std::env::current_exe;
use std::io::{BufRead, BufReader};
use std::process::Stdio;
use std::thread;

use basic_usages::external::fxhash::{FxHashMap as HashMap, FxHashSet as HashSet};
use basic_usages::ruf_check_info::{CheckInfo, UsedRufs};

use ansi_term::{Color, Style};
use lazy_static::lazy_static;
use log::debug;
use petgraph::visit;
use regex::Regex;

use crate::build_config::BuildConfig;
use crate::cargo;
use crate::dep_manager::DepManager;

// Some regex definitions.
lazy_static! {
    pub static ref RE_USEDFEATS: Regex = Regex::new(r"FDelimiter::\{(.*?)\}::FDelimiter").unwrap();
    pub static ref RE_CHECKINFO: Regex = Regex::new(r"CDelimiter::\{(.*?)\}::CDelimiter").unwrap();
    pub static ref BOLD_RED: Style = Style::new().bold().fg(Color::Red);
    pub static ref BOLD_GREEN: Style = Style::new().bold().fg(Color::Green);
}

// lazy_static! {
//     static ref DEPMANAGER: Mutex<MaybeUninit<DepManager>> = Mutex::new(MaybeUninit::uninit());
// }

/// Create a wrapper around cargo and rustc,
/// this function shall be called only once, at first layer.
pub fn cargo_wrapper(mut config: BuildConfig) -> i32 {
    // We check ruf usage first
    let used_rufs = match rustc_wrapper(&mut config) {
        Ok(used_rufs) => used_rufs,
        Err(err) => {
            error_print(&err);
            return -1;
        }
    };

    // We fetch the used features, and then we shall check it
    info_print("Finishing", "fetching used rufs");

    println!("[Debug] rufs: {:#?}", used_rufs);

    info_print("Starting", "analyzing used rufs");
    // Check rufs
    if let Err(err) = check_rufs(&config, used_rufs) {
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
fn rustc_wrapper(config: &mut BuildConfig) -> Result<HashMap<String, UsedRufs>, String> {
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
        let mut checkinfos: HashMap<String, (HashSet<String>, HashSet<String>)> =
            HashMap::default();

        for line in stdout.lines() {
            debug!("Stdout: {line:?}");
            let line = line.expect("Fatal, get stdout line fails");

            if let Some(caps) = RE_CHECKINFO.captures(&line) {
                let info = CheckInfo::from(
                    caps.get(1)
                        .expect("Fatal, resolve buildinfo fails")
                        .as_str(),
                );
                let entry = checkinfos
                    .entry(info.crate_name)
                    .or_insert_with(|| (HashSet::default(), HashSet::default()));

                entry.0.extend(info.used_rufs.into_iter());
                entry.1.extend(info.cfg.into_iter());
            }
        }

        return checkinfos;
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
    let checkinfos = stdout_handle
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

    let mut used_rufs = HashMap::default();
    for (crate_name, (rufs, cfgs)) in checkinfos {
        config.update_buildinfo(crate_name.clone(), cfgs);
        used_rufs.insert(crate_name, UsedRufs::new(rufs.into_iter().collect()));
    }

    return Ok(used_rufs);
}

fn check_rufs(config: &BuildConfig, used_rufs: HashMap<String, UsedRufs>) -> Result<(), String> {
    let dm = DepManager::new()?;

    // Check ruf usage in BFS mode.
    let graph = dm.graph();
    let root = dm.root();
    let mut issued_depnx = None;

    let mut bfs = visit::Bfs::new(&graph, root);
    while let Some(nx) = bfs.next(&graph) {
        let node = &graph[nx];
        let rufs = used_rufs.get(node.name.as_str()).expect(&format!(
            "Fatal, cannot fetch used rufs for crate {}",
            node.name
        ));

        if !config.rufs_usable(&rufs) {
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

    // Canditate versions, restricted by semver, no rufs checks
    let candidate_vers = dm.get_candidates(issued_depnx)?;
    println!("[Debug] candidates: {:?}", candidate_vers);

    info_print(
        "\tFixing",
        &format!(
            "filter usable version from {} candidate versions",
            candidate_vers.len()
        ),
    );

    // here we check rufs
    let mut usable_vers = vec![];

    for cad in candidate_vers {
        let used_rufs = config.filter_rufs(cad.1)?;
        if config.rufs_usable(&used_rufs) {
            usable_vers.push(cad.0);
        }
    }

    info_print(
        "\tFixing",
        &format!(
            "found {} usable version: {:?}",
            usable_vers.len(),
            usable_vers
        ),
    );

    // if usable_vers.is_empty() {
    //     return Err(format!(
    //         "Fatal, cannot find usable version for crate '{}'",
    //         issued_dep.name
    //     ));
    // }

    unimplemented!()
}
