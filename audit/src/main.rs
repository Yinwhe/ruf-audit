use std::env;
use std::process::{exit, Command};

use ansi_term::{Color, Style};
use getopts::Options;
use lazy_static::lazy_static;
use regex::Regex;

#[allow(unused)]
use slog::{debug, info, o, warn, Drain, Logger};
use slog_async::Async;
use sloggers::{file::FileLoggerBuilder, Build};

mod extract;
use extract::extract;

mod audit;
use audit::{audit, test};

mod error;

mod build_config;
use build_config::BuildConfig;

mod dep_manager;

// Some regex definitions.
lazy_static! {
    pub static ref RE_USEDFEATS: Regex = Regex::new(r"FDelimiter::\{(.*?)\}::FDelimiter").unwrap();
    pub static ref RE_CHECKINFO: Regex = Regex::new(r"CDelimiter::\{(.*?)\}::CDelimiter").unwrap();
    pub static ref BOLD_RED: Style = Style::new().bold().fg(Color::Red);
    pub static ref BOLD_YELLOW: Style = Style::new().bold().fg(Color::Yellow);
    pub static ref BOLD_GREEN: Style = Style::new().bold().fg(Color::Green);
    pub static ref LOGGER: Logger = {
        let file_logger = FileLoggerBuilder::new("./debug.log")
            .level(sloggers::types::Severity::Warning)
            .build()
            .expect("Fatal, build logger fails");

        let async_drain = Async::new(file_logger.fuse()).build().fuse();
        Logger::root(async_drain, o!())
    };
}

fn main() {
    // Get current config first
    let mut config = match BuildConfig::default() {
        Ok(config) => config,
        Err(e) => {
            error_print!(false, &format!("{e}"));
            exit(-1);
        }
    };

    let args: Vec<String> = env::args().collect();
    // info!(LOGGER, "startup command line: {:?}", &args);

    // cargo wrapper usage, act as scanner.
    if args.len() >= 2 && args[1] == config.get_audit_rustc_path() {
        // debug!(LOGGER, "scanner args: {args:?}");

        // We use original rustc to do some information fetch
        let status = if args[2] == "-" {
            // debug!(LOGGER, "Use rustc, inherit std");
            rustc()
                .args(&args[2..])
                .spawn()
                .expect("Fatal, cannot run rustc")
                .wait_with_output()
                .expect("Fatal, cannot fetch rustc output")
                .status
        } else {
            // debug!(LOGGER, "Use audit, pass by pipe");

            // And here we do the scan operation, after scan we launch real rustc,
            // this is essential, since some crates has build scripts or things like that.
            // Besides, we gain incremental check from cargo for launching real rustc, which is good
            // for later repeated extract process.
            scanner()
                .args(["--checkinfo", "--rustc", &args[1], "--"])
                .args(&args[2..])
                .env("LD_LIBRARY_PATH", config.get_rustlib_path())
                .spawn()
                .expect("Fatal, cannot run scanner")
                .wait_with_output()
                .expect("Fatal, cannot fetch scanner output")
                .status
        };

        exit(status.code().unwrap_or(-1))
    }

    let my_args = if let Some(split_index) = args.iter().position(|arg| arg == "--") {
        config.update_cargo_args(&args[split_index + 1..]);
        &args[..split_index]
    } else {
        &args
    };

    let mut opts = Options::new();
    opts.optflag("h", "help", "Print help information");
    opts.optflag("", "extract", "Extract rufs used in current configurations");
    opts.optflag(
        "",
        "newer-fix",
        "Attempt to choose newer version when fixing dep trees",
    );
    opts.optflag(
        "",
        "quick-fix",
        "Fix by changing rustc and using oldest dep tree",
    );
    opts.optflag("", "verbose", "Print audit detail info");
    opts.optflag("", "test", "Only used for test purpose");

    let matches = match opts.parse(my_args) {
        Ok(m) => m,
        Err(e) => {
            error_print!(false, &format!("parse cli args fails: {e}"));
            exit(-1);
        }
    };

    // TODO: write help doc.
    if matches.opt_present("h") {
        unimplemented!()
    }

    if matches.opt_present("extract") {
        match extract(&mut config, false) {
            Ok(used_rufs) => {
                // TODO: Pretty print
                println!("{used_rufs:?}");
            }
            Err(_e) => {
                unimplemented!()
            }
        }

        exit(0);
    }

    if matches.opt_present("newer-fix") {
        config.set_newer_fix(true);
    }

    if matches.opt_present("quick-fix") {
        config.set_quick_fix(true);
    }

    if matches.opt_present("verbose") {
        config.set_verbose(true);
    }

    if matches.opt_present("test") {
        config.set_test(true);
        let exit_code = test(config);
        exit(exit_code);
    }

    // warn!(LOGGER, "Exec audit, this function shall be exec only once globally!");

    // default we do ruf audit
    let exit_code = audit(config, false);
    exit(exit_code);
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

fn rustc() -> Command {
    let mut cmd = Command::new("rustc");
    cmd.env("RUSTUP_TOOLCHAIN", "nightly-2023-12-12");

    cmd
}

#[macro_export]
macro_rules! info_print {
    ($quiet:expr, $title:expr, $msg:expr) => {
        if !$quiet {
            println!("{} {}", $crate::BOLD_GREEN.paint($title), $msg);
        }
    };
}

#[macro_export]
macro_rules! warn_print {
    ($quiet:expr, $title:expr, $msg:expr) => {
        if !$quiet {
            println!("{} {}", $crate::BOLD_YELLOW.paint($title), $msg);
        }
    };
}

#[macro_export]
macro_rules! error_print {
    ($quiet:expr, $msg:expr) => {
        if !$quiet {
            println!("{} {}", $crate::BOLD_RED.paint("error"), $msg);
        }
    };
}
