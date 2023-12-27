use std::env::{args, current_exe};
use std::fs::File;
use std::process::{exit, Command};

use log::{debug, info, warn};
use simplelog::{CombinedLogger, Config, LevelFilter, WriteLogger};

fn audit() -> Command {
    let mut path = current_exe().expect("current executable path invalid");
    path.set_file_name("audit");
    Command::new(path)
}

fn cargo() -> Command {
    Command::new("cargo")
}

fn main() {
    let ld_library_path = init();

    debug!(
        "startup command line: {:?}",
        args().collect::<Vec<String>>()
    );

    debug!("Library Path: {:?}", ld_library_path);

    let args = args().collect::<Vec<String>>();
    if args.len() >= 3 {
        debug!("Long args: {args:?}");

        let output = if args[2] == "-" {
            debug!("Use rustc, inherit std");
            Command::new(&args[1])
                .args(&args[2..])
                .spawn()
                .expect("Fatal, cannot run rustc")
                .wait_with_output()
                .expect("Fatal, cannot fetch output")
        } else {
            debug!("Use audit, use pipe");
            audit()
                .args(&args[2..])
                .env("LD_LIBRARY_PATH", ld_library_path)
                .output()
                .expect("Fatal, cannot fetch output")
        };

        info!("Stdout: {:?}", String::from_utf8_lossy(&output.stdout));
        info!("Stderr: {:?}", String::from_utf8_lossy(&output.stderr));

        exit(output.status.code().unwrap_or(0))
    } else {
        warn!("Exec cargo_wrapper, this function shall be exec only once globally!");
        cargo_wrapper();

        exit(0)
    }
}

fn cargo_wrapper() {
    let mut cmd = cargo();
    cmd.arg("rustc");

    let path = current_exe().expect("current executable path invalid");
    cmd.env("RUSTC_WRAPPER", &path);

    let status = cmd
        .spawn()
        .expect("could not run cargo")
        .wait()
        .expect("failed to wait cargo");

    // TODO: status
}

fn init() -> String {
    CombinedLogger::init(vec![
        // TermLogger::new(
        //     LevelFilter::Info,
        //     Config::default(),
        //     TerminalMode::Mixed,
        //     ColorChoice::Auto,
        // ),
        WriteLogger::new(
            LevelFilter::Info,
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
