use std::collections::{HashMap, HashSet};
use std::env::current_exe;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::thread;

use ansi_term::{Color, Style};
use features::Features;
use lazy_static::lazy_static;
use log::debug;
use regex::Regex;
use simplelog::{CombinedLogger, Config, LevelFilter, WriteLogger};

use super::cargo;

lazy_static! {
    static ref RE_FEATURES: Regex = Regex::new(r"FDelimiter::\{(.*?)\}::FDelimiter").unwrap();
    static ref RE_INFOS: Regex = Regex::new(r"\s+Compiling\s+(\w+)\s+v([\d.]+)\s+\((.*?)\)").unwrap();
    static ref BOLD_RED: Style = Style::new().bold().fg(Color::Red);
    static ref BOLD_GREEN: Style = Style::new().bold().fg(Color::Green);
}

/// Create a wrapper around cargo and rustc,
/// this function shall be called only once, at first layer.
pub fn cargo_wrapper() -> i32 {
    let mut cmd = cargo();

    cmd.arg("rustc");

    let path = current_exe().expect("current executable path invalid");
    cmd.env("RUSTC_WRAPPER", &path);

    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("could not run cargo");

    // Listen to the cargo process
    let stdout = BufReader::new(child.stdout.take().expect("Fatal, failed to get stdout"));
    let stderr = BufReader::new(child.stderr.take().expect("Fatal, failed to get stderr"));

    // Resolving stdout and stderr infos
    let stdout_handle = thread::spawn(move || {
        let mut used_features: HashMap<String, HashSet<String>> = HashMap::new();

        for line in stdout.lines() {
            debug!("Stdout: {line:?}");
            let line = line.expect("Fatal, get stdout line fails");

            if let Some(caps) = RE_FEATURES.captures(&line) {
                let features = Features::from(caps.get(1).map_or("Compiling...", |m| m.as_str()));
                used_features
                    .entry(features.crate_name)
                    .or_insert_with(HashSet::new)
                    .extend(features.features);
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
    let rufs = stdout_handle.join().unwrap();
    stderr_handle.join().unwrap();

    let output = child
        .wait_with_output()
        .expect("Fatal, fails to get status of cargo");

    if !output.status.success() {
        error(&String::from_utf8_lossy(&output.stderr));
    }

    println!("{} ruf scan done", BOLD_GREEN.paint("Finishing"));
    println!("{:?}", rufs);

    return output.status.code().unwrap_or(0);
}

/// Do some init things.
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
                .open("/home/ubuntu/Workspace/ruf-audit/debug.log")
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
