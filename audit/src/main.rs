use std::env::args;
use std::fs::File;
use std::process::{exit, Command};

use log::{debug, info, warn};
use simplelog::{CombinedLogger, Config, LevelFilter, WriteLogger};

mod utils;
use utils::cargo_wrapper;

mod build_config;
use build_config::BuildConfig;

mod dep_manager;

fn main() {
    let config = init();

    info!(
        "startup command line: {:?}",
        args().collect::<Vec<String>>()
    );

    // TODO: Make args parse better
    let mut args = args().collect::<Vec<String>>();
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
            // We need to collect build configs.
            args[0] = "--checkinfo".to_string();
            args[1] = "--".to_string();
            scanner()
                .args(&args)
                .env("LD_LIBRARY_PATH", config.get_rustlib_path())
                .spawn()
                .expect("Fatal, cannot run scanner")
                .wait_with_output()
                .expect("Fatal, cannot fetch scanner output")
                .status
        };

        exit(status.code().unwrap_or(-1))
    } else {
        warn!("Exec cargo_wrapper, this function shall be exec only once globally!");

        let exit_code = cargo_wrapper(config);
        exit(exit_code);
    }
}

/// Do some init things, and return needed lib path.
fn init() -> BuildConfig {
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
    BuildConfig::default().expect("TEMP")
}

fn scanner() -> Command {
    let mut cmd = Command::new("ruf_scanner");
    cmd.env("RUSTUP_TOOLCHAIN", "nightly-2023-12-12");

    cmd
}

fn cargo() -> Command {
    let mut cmd = Command::new("cargo");
    cmd.env("RUSTUP_TOOLCHAIN", "nightly-2023-12-12");

    cmd
}
