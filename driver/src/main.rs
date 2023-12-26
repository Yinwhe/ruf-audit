use std::env::{args, current_exe};
use std::fs::File;
use std::io::Write;
use std::process::{exit, Command};

use log::{debug, info};
use simplelog::{
    ColorChoice, CombinedLogger, Config, LevelFilter, TermLogger, TerminalMode, WriteLogger,
};

fn audit() -> Command {
    let mut path = current_exe().expect("current executable path invalid");
    path.set_file_name("audit");
    Command::new(path)
}

fn cargo() -> Command {
    Command::new("cargo")
}

fn main() {
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

    info!(
        "startup command line: {:?}",
        args().collect::<Vec<String>>()
    );

    let args = args().collect::<Vec<String>>();
    if args.len() >= 3 {
        info!("Long args: {args:?}");

        let mut cmd = if args[2] == "-" {
            info!("Use rustc");
            let cmd = Command::new(&args[1]);
            cmd
        } else {
            info!("Use audit");
            let mut cmd = audit();
            cmd.env("LD_LIBRARY_PATH", "/home/ubuntu/.rustup/toolcains/nightly-2023-12-12-x86_64-unknown-linux-gnu/lib/rustlib/x86_64-unknown-linux-gnu/lib:/home/ubuntu/.rustup/toolchains/nightly-2023-12-12-x86_64-unknown-linux-gnu/lib");
            cmd
        };

        cmd.args(&args[2..]);

        let output = cmd.spawn().unwrap().wait_with_output().unwrap();

        info!("Stdout: {:?}", String::from_utf8_lossy(&output.stdout));
        info!("Stderr: {:?}", String::from_utf8_lossy(&output.stderr));
    } else {
        info!("No arg found");
        cargo_wrapper();
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
}
