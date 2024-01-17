use std::env::{args, current_exe};
use std::process::{exit, Command};

use log::{debug, info, warn};

mod utils;
use utils::{cargo_wrapper, init};

fn main() {
    let ld_library_path = init();
    // TODO: use config types.
    info!(
        "startup command line: {:?}",
        args().collect::<Vec<String>>()
    );

    debug!("Library Path: {:?}", ld_library_path);

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
                .env("LD_LIBRARY_PATH", ld_library_path)
                .spawn()
                .expect("Fatal, cannot run scanner")
                .wait_with_output()
                .expect("Fatal, cannot fetch scanner output")
                .status
        };

        exit(status.code().unwrap_or(0))
    } else {
        warn!("Exec cargo_wrapper, this function shall be exec only once globally!");

        let exit_code = cargo_wrapper();
        exit(exit_code);
    }
}

fn scan() -> Command {
    let mut path = current_exe().expect("current executable path invalid");
    path.set_file_name("ruf_scanner");
    Command::new(path)
}

fn cargo() -> Command {
    Command::new("cargo")
}
