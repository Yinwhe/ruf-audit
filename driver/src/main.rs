use std::env::{args, current_exe};
use std::fs::File;
use std::process::Command;

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

    info!(
        "startup command line: {:?}",
        args().collect::<Vec<String>>()
    );

    let args = args().collect::<Vec<String>>();
    if args.len() >= 3 {
        debug!("Long args: {args:?}");

        let mut cmd = if args[2] == "-" {
            debug!("Use rustc");
            let cmd = Command::new(&args[1]);
            cmd
        } else {
            debug!("Use audit");
            let mut cmd = audit();
            cmd.env("LD_LIBRARY_PATH", ld_library_path);
            cmd
        };

        cmd.args(&args[2..]);

        let output = cmd.spawn().unwrap().wait_with_output().unwrap();

        debug!("Stdout: {:?}", String::from_utf8_lossy(&output.stdout));
        debug!("Stderr: {:?}", String::from_utf8_lossy(&output.stderr));
    } else {
        warn!("Exec cargo_wrapper, this function shall be exec only once globally!");
        cargo_wrapper()
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

    let rustup_dir =
        format!("{rustup_home}/toolchains/nightly-2023-12-12-x86_64-unknown-linux-gnu");
    let lib_dir1 = format!("{rustup_dir}/lib/rustlib/x86_64-unknown-linux-gnu/lib");
    let lib_dir2 = format!("{rustup_dir}/lib");

    format!("{}:{}", lib_dir1, lib_dir2)
}
