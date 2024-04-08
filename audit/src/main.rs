use std::env;
use std::env::current_exe;
use std::path::PathBuf;
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
    pub static ref RE_RUSTC_VRESION: Regex = Regex::new(r"rustc\s+1\.(\d+)\.\d+-nightly").unwrap();
    pub static ref BOLD_RED: Style = Style::new().bold().fg(Color::Red);
    pub static ref BOLD_YELLOW: Style = Style::new().bold().fg(Color::Yellow);
    pub static ref BOLD_GREEN: Style = Style::new().bold().fg(Color::Green);
    pub static ref LOGGER: Logger = {
        let file_logger = FileLoggerBuilder::new("./debug.log")
            .level(sloggers::types::Severity::Debug)
            .build()
            .expect("Fatal, build logger fails");

        let async_drain = Async::new(file_logger.fuse()).build().fuse();
        Logger::root(async_drain, o!())
    };
    pub static ref SCANNER_PATH: PathBuf = {
        let mut path = current_exe().expect("current executable path invalid");
        path.set_file_name("ruf_scanner");
        path
    };
}

const RUSTV: &str = "nightly-2023-12-12";

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
            spec_rustc(RUSTV)
                .args(&args[2..])
                .spawn()
                .expect("Fatal, cannot run rustc")
                .wait()
                .expect("Fatal, cannot fetch rustc output")
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
                .wait()
                .expect("Fatal, cannot fetch scanner output")
        };

        // debug!(LOGGER, "scanner exit");
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
        info_print!(
            false,
            "Starting",
            "extract rufs used in current configurations"
        );
        // TODO: extract functionality
        match extract(&mut config, false) {
            Ok(used_rufs) => {
                println!("extract success: {used_rufs:?}");
                for (name, rufs) in used_rufs {
                    if config.rufs_usable(&rufs) {
                        println!("crate {name} ruf usage ok");
                    } else {
                        println!("crate {name} ruf usage not ok");
                    }
                }
            }
            Err(_e) => {}
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
    let mut cmd = Command::new(SCANNER_PATH.as_os_str());
    cmd.env("RUSTUP_TOOLCHAIN", RUSTV);

    cmd
}

fn spec_cargo(ver: &str) -> Command {
    let mut cmd = Command::new("cargo");
    cmd.env("RUSTUP_TOOLCHAIN", ver);

    cmd
}

fn spec_rustc(ver: &str) -> Command {
    let mut cmd = Command::new("rustc");
    cmd.env("RUSTUP_TOOLCHAIN", ver);

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

/*
ruf_scanner --checkinfo --rustc /home/ubuntu/.rustup/toolchains/nightly-2023-12-12-x86_64-unknown-linux-gnu/bin/rustc -- --crate-name libsecp256k1 --edition=2018
 /home/ubuntu/.cargo/registry/src/mirrors.ustc.edu.cn-61ef6e0cd06fb9b8/libsecp256k1-0.6.0/src/lib.rs --error-format=json --json=diagnostic-rendered-ansi,artifacts,future-incompat --crate-type lib
 --emit=dep-info,metadata,link -C embed-bitcode=no -C debuginfo=2 --cfg feature="default" --cfg feature="hmac" --cfg feature="hmac-drbg" --cfg feature="sha2" --cfg feature="static-context" --cfg
 feature="std" --cfg feature="typenum" -C metadata=154f7cd92950c9e2 -C extra-filename=-154f7cd92950c9e2 --out-dir
 /home/ubuntu/Workspaces/Cargo-Ecosystem-Monitor/Code/crate_downloader/on_process/solana-local-cluster/solana-local-cluster-1.9.13/target/debug/deps -L
 dependency=/home/ubuntu/Workspaces/Cargo-Ecosystem-Monitor/Code/crate_downloader/on_process/solana-local-cluster/solana-local-cluster-1.9.13/target/debug/deps --extern
 arrayref=/home/ubuntu/Workspaces/Cargo-Ecosystem-Monitor/Code/crate_downloader/on_process/solana-local-cluster/solana-local-cluster-1.9.13/target/debug/deps/libarrayref-51769418d0067876.rmeta --extern
 base64=/home/ubuntu/Workspaces/Cargo-Ecosystem-Monitor/Code/crate_downloader/on_process/solana-local-cluster/solana-local-cluster-1.9.13/target/debug/deps/libbase64-adbd1c8fc04e1957.rmeta --extern
 digest=/home/ubuntu/Workspaces/Cargo-Ecosystem-Monitor/Code/crate_downloader/on_process/solana-local-cluster/solana-local-cluster-1.9.13/target/debug/deps/libdigest-553aeee4092a1a80.rmeta --extern
 hmac_drbg=/home/ubuntu/Workspaces/Cargo-Ecosystem-Monitor/Code/crate_downloader/on_process/solana-local-cluster/solana-local-cluster-1.9.13/target/debug/deps/libhmac_drbg-895ebab67ac95949.rmeta --extern
 libsecp256k1_core=/home/ubuntu/Workspaces/Cargo-Ecosystem-Monitor/Code/crate_downloader/on_process/solana-local-cluster/solana-local-cluster-1.9.13/target/debug/deps/liblibsecp256k1_core-2a42537dec6fe15
9.rmeta --extern rand=/home/ubuntu/Workspaces/Cargo-Ecosystem-Monitor/Code/crate_downloader/on_process/solana-local-cluster/solana-local-cluster-1.9.13/target/debug/deps/librand-bf1019cb3e354268.rmeta
 --extern serde=/home/ubuntu/Workspaces/Cargo-Ecosystem-Monitor/Code/crate_downloader/on_process/solana-local-cluster/solana-local-cluster-1.9.13/target/debug/deps/libserde-9841ebf5c16aadb6.rmeta
 --extern sha2=/home/ubuntu/Workspaces/Cargo-Ecosystem-Monitor/Code/crate_downloader/on_process/solana-local-cluster/solana-local-cluster-1.9.13/target/debug/deps/libsha2-1725700b33598f2a.rmeta --extern
 typenum=/home/ubuntu/Workspaces/Cargo-Ecosystem-Monitor/Code/crate_downloader/on_process/solana-local-cluster/solana-local-cluster-1.9.13/target/debug/deps/libtypenum-37e8da50dfc9f34b.rmeta --cap-lints
 allow
*/
