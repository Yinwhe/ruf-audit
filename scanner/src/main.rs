#![feature(rustc_private)]
#![feature(let_chains)]

extern crate rustc_ast;
extern crate rustc_codegen_ssa;
extern crate rustc_data_structures;
extern crate rustc_driver;
extern crate rustc_error_codes;
extern crate rustc_errors;
extern crate rustc_expand;
extern crate rustc_feature;
extern crate rustc_hash;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_lint;
extern crate rustc_metadata;
extern crate rustc_parse;
extern crate rustc_session;
extern crate rustc_span;
extern crate rustc_target;

use getopts::Options;
use std::{
    env,
    process::{exit, Command},
};

mod rustc76;

fn main() {
    // parse args
    let args = env::args().collect::<Vec<_>>();

    let mut opts = Options::new();
    opts.optflag("h", "help", "Print help information");
    opts.optflag(
        "c",
        "checkinfo",
        "Print full check information, or only print used rufs",
    );
    opts.optopt("r", "rustc", "Run rustc after scan", "VALUE");

    let split_index = args.iter().position(|arg| arg == "--");

    if split_index.is_none() {
        println!("Args error: no `--` found");
        show_help();
        exit(-1);
    }

    let split_index = split_index.unwrap();
    let my_args = &args[..split_index];
    let rustc_args = args[split_index + 1..].to_vec();

    let matches = match opts.parse(my_args) {
        Ok(m) => m,
        Err(e) => {
            println!("Args error: {e}");
            show_help();
            exit(-1);
        }
    };
    if matches.opt_present("h") || rustc_args.is_empty() {
        show_help();
    }
    let output_buildinfo = matches.opt_present("c");

    // run our scanner
    let exit_code = rustc76::run_rustc(&rustc_args, output_buildinfo);
    if exit_code != 0 {
        exit(exit_code);
    }

    if let Some(rustc_path) = matches.opt_str("r") {
        let mut rustc = Command::new(rustc_path);
        rustc.args(&rustc_args);
        let output = rustc.output().expect("failed to execute rustc");
        // println!("{output:?}");
        if output.status.success() {
            // if success, we print its output
            println!("{}", String::from_utf8_lossy(&output.stdout));
        }
        // but we block its failure, since we should continue to next crates.
    }

    exit(0)
}

fn show_help() {}
