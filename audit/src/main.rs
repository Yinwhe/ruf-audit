use std::env::{args, current_exe};
use std::fs::File;
use std::process::{exit, Command};

use log::{debug, info, warn};
use simplelog::{CombinedLogger, Config, LevelFilter, WriteLogger};

mod utils;
use utils::cargo_wrapper;

mod config;
use config::AuditConfig;

fn main() {
    let config = init();

    info!(
        "startup command line: {:?}",
        args().collect::<Vec<String>>()
    );

    // TODO: Make args parse better
    let args = args().collect::<Vec<String>>();
    if args.len() >= 3 {
        debug!("Long args: {args:?}");

        // We use original rustc to do some information fetch
        let status = if args[2] == "-" {
            debug!("Use rustc, inherit std");
            Command::new(&args[1])
                .args(&args[2..])
                .spawn()
                .expect("Fatal, cannot run rustc")
                .wait_with_output()
                .expect("Fatal, cannot fetch rustc output")
                .status
        } else {
            // And here we do the scan operation
            debug!("Use audit, pass by pipe");
            scan()
                .args(&args[2..])
                .env("LD_LIBRARY_PATH", config.get_rustlib_path())
                .spawn()
                .expect("Fatal, cannot run scanner")
                .wait_with_output()
                .expect("Fatal, cannot fetch scanner output")
                .status
        };

        exit(status.code().unwrap_or(0))
    } else {
        warn!("Exec cargo_wrapper, this function shall be exec only once globally!");

        let exit_code = cargo_wrapper(config);
        exit(exit_code);
    }
}

/// Do some init things, and return needed lib path.
fn init() -> AuditConfig {
    CombinedLogger::init(vec![WriteLogger::new(
        LevelFilter::Debug,
        Config::default(),
        File::options()
            .write(true)
            .append(true)
            .create(true)
            .open("/home/ubuntu/Workspaces/ruf-audit/debug.log")
            .unwrap(),
    )])
    .unwrap();

    // TODO: fix expect
    AuditConfig::default().expect("TEMP")
}

fn scan() -> Command {
    let mut path = current_exe().expect("current executable path invalid");
    path.set_file_name("ruf_scanner");
    Command::new(path)
}

fn cargo() -> Command {
    Command::new("cargo")
}
