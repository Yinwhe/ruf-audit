use std::env::current_exe;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::thread;

use ansi_term::{Color, Style};
use features::Rufs;
use fxhash::{FxHashMap as HashMap, FxHashSet as HashSet};
use lazy_static::lazy_static;
use lockfile::parse_from_path;
use log::debug;
use regex::Regex;
use simplelog::{CombinedLogger, Config, LevelFilter, WriteLogger};

use super::cargo;

// Test usage, we shall remove it later.
const RUSTC_VERSION: u32 = 65;

lazy_static! {
    static ref RE_FEATURES: Regex = Regex::new(r"FDelimiter::\{(.*?)\}::FDelimiter").unwrap();
    static ref RE_INFOS: Regex =
        Regex::new(r"\s+Compiling\s+(\w+)\s+v([\d.]+)\s+\((.*?)\)").unwrap();
    static ref BOLD_RED: Style = Style::new().bold().fg(Color::Red);
    static ref BOLD_GREEN: Style = Style::new().bold().fg(Color::Green);
}

/// Create a wrapper around cargo and rustc,
/// this function shall be called only once, at first layer.
pub fn cargo_wrapper() -> i32 {
    // We check ruf usage first
    let used_rufs = match rustc_wrapper() {
        Ok(used_rufs) => used_rufs,
        Err(err) => {
            error(&err);
            return -1;
        }
    };

    // We fetch the used features, and then we shall check it
    println!("{} ruf scan done", BOLD_GREEN.paint("Finishing"));

    // Check rufs
    check_rufs(used_rufs).unwrap();

    return 0;
}

/// Do some init things, and return needed lib path.
pub fn init() -> String {
    CombinedLogger::init(vec![
        // TermLogger::new(
        //     LevelFilter::Info,
        //     Config::default(),
        //     TerminalMode::Mixed,
        //     ColorChoice::Auto,
        // ),
        WriteLogger::new(
            LevelFilter::Debug,
            Config::default(),
            File::options()
                .write(true)
                .append(true)
                .create(true)
                .open("/home/ubuntu/Workspaces/ruf-audit/debug.log")
                .unwrap(),
        ),
    ])
    .unwrap();

    // Get some library path
    let mut rustup_home = Command::new("rustup");
    rustup_home.args(["show", "home"]);
    let output = rustup_home
        .output()
        .expect("Fatal, cannot fetch rustup home");

    let rustup_home = if output.status.success() {
        String::from_utf8_lossy(&output.stdout)
    } else {
        panic!("Fatal, cannot fetch rustup home")
    };

    let rustup_dir = format!(
        "{}/toolchains/nightly-2023-12-12-x86_64-unknown-linux-gnu",
        rustup_home.trim()
    );
    let lib_dir1 = format!("{rustup_dir}/lib/rustlib/x86_64-unknown-linux-gnu/lib");
    let lib_dir2 = format!("{rustup_dir}/lib");

    format!("{}:{}", lib_dir1, lib_dir2)
}

fn error(msg: &str) {
    println!("{} {}", BOLD_RED.paint("error"), msg);
}

/// This function wrap rustc with our audit tool, and fetch
/// all rufs used in the crate.
fn rustc_wrapper() -> Result<HashMap<String, HashSet<String>>, String> {
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

    // Resolving stdout and stderr infos
    let stdout_handle = thread::spawn(move || {
        let mut used_features: HashMap<String, HashSet<String>> = HashMap::default();

        for line in stdout.lines() {
            debug!("Stdout: {line:?}");
            let line = line.expect("Fatal, get stdout line fails");

            if let Some(caps) = RE_FEATURES.captures(&line) {
                let rufs = Rufs::from(caps.get(1).expect("Fatal, resolve ruf fails").as_str());
                used_features
                    .entry(rufs.crate_name)
                    .or_insert_with(HashSet::default)
                    .extend(rufs.rufs.into_iter().map(|ruf| {
                        assert!(ruf.cond.is_none());
                        ruf.feature
                    }));
            }
        }

        return used_features;
    });

    let stderr_handle = thread::spawn(move || {
        for line in stderr.lines() {
            debug!("Stderr: {line:?}");
            let line = line.expect("Fatal, get stdout line fails");

            // compiling info, we can print it
            if let Some(index) = line.find("Compiling") {
                println!("\t{} {}", BOLD_GREEN.paint("Scanning"), &line[index..]);
            }
        }
    });

    // Waiting for process to ends
    let used_rufs = stdout_handle.join().map_err(|err| {
        format!("Fatal, cannot extract rufs from stdout thread, maybe cargo process fails? Error info: {:?}", err)
    })?;
    stderr_handle.join().unwrap();

    let output = child
        .wait_with_output()
        .map_err(|_| format!("Fatal, fails to wait cargo process"))?;

    if !output.status.success() {
        return Err(format!(
            "Fatal, cargo process run fails, stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    return Ok(used_rufs);
}

fn check_rufs(used_rufs: HashMap<String, HashSet<String>>) -> Result<(), String> {
    let lockfile = parse_from_path("Cargo.lock")
        .map_err(|err| format!("Fatal, cannot parse Cargo.lock: {}", err))?;

    let dep_tree = lockfile
        .dependency_tree()
        .map_err(|err| format!("Fatal, cannot get dependency tree from Cargo.lock: {}", err))?;


    for pack in lockfile.packages {
        let name = pack.name.as_str();
        if let Some(ruf) = used_rufs.get(name) {
            //TODO: check ruf
        }
    }

    unimplemented!()
}
