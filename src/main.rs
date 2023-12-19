#![feature(rustc_private)]

extern crate rustc_ast;
extern crate rustc_driver;
extern crate rustc_error_codes;
extern crate rustc_errors;
extern crate rustc_expand;
extern crate rustc_hash;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_lint;
extern crate rustc_save_analysis;
extern crate rustc_session;
extern crate rustc_span;

use std::{env, process};

use rustc_driver::{catch_with_exit_code, TimePassesCallbacks};
use rustc_session::{config::ErrorOutputType, early_error};

mod utils;

fn main() {
    let mut callbacks = TimePassesCallbacks::default();
    let exit_code = catch_with_exit_code(|| {
        // map ArgsOs to String
        let args = env::args_os()
            .enumerate()
            .map(|(i, arg)| {
                arg.into_string().unwrap_or_else(|arg| {
                    early_error(
                        ErrorOutputType::default(),
                        &format!("argument {i} is not valid Unicode: {arg:?}"),
                    )
                })
            })
            .collect::<Vec<_>>();
        utils::run_compiler(&args, &mut callbacks)
    });

    process::exit(exit_code)
}

// use std::{path, process, str};

// use rustc_errors::registry;
// use rustc_hash::{FxHashMap, FxHashSet};
// use rustc_session::config::{self, CheckCfg};
// use rustc_span::source_map;

// fn main() {
//     let out = process::Command::new("rustc")
//         .arg("--print=sysroot")
//         .current_dir(".")
//         .output()
//         .unwrap();
//     let sysroot = str::from_utf8(&out.stdout).unwrap().trim();

//     let config = rustc_interface::Config {
//         // Command line options
//         opts: config::Options {
//             maybe_sysroot: Some(path::PathBuf::from(sysroot)),
//             ..config::Options::default()
//         },
//         // cfg! configuration in addition to the default ones
//         crate_cfg: FxHashSet::default(), // FxHashSet<(String, Option<String>)>
//         crate_check_cfg: CheckCfg::default(), // CheckCfg
//         input: config::Input::Str {
//             name: source_map::FileName::Custom("main.rs".into()),
//             input: r#"
// static HELLO: &str = "Hello, world!";
// fn main() {
//     println!("{HELLO}");
// }
// "#
//             .into(),
//         },
//         output_dir: None,  // Option<PathBuf>
//         output_file: None, // Option<PathBuf>
//         file_loader: None, // Option<Box<dyn FileLoader + Send + Sync>>

//         lint_caps: FxHashMap::default(), // FxHashMap<lint::LintId, lint::Level>
//         // This is a callback from the driver that is called when [`ParseSess`] is created.
//         parse_sess_created: None, //Option<Box<dyn FnOnce(&mut ParseSess) + Send>>
//         // This is a callback from the driver that is called when we're registering lints;
//         // it is called during plugin registration when we have the LintStore in a non-shared state.
//         //
//         // Note that if you find a Some here you probably want to call that function in the new
//         // function being registered.
//         register_lints: None, // Option<Box<dyn Fn(&Session, &mut LintStore) + Send + Sync>>
//         // This is a callback from the driver that is called just after we have populated
//         // the list of queries.
//         //
//         // The second parameter is local providers and the third parameter is external providers.
//         override_queries: None, // Option<fn(&Session, &mut ty::query::Providers<'_>, &mut ty::query::Providers<'_>)>
//         // This is a callback from the driver that is called to create a codegen backend.
//         make_codegen_backend: None,
//         // Registry of diagnostics codes.
//         registry: registry::Registry::new(&rustc_error_codes::DIAGNOSTICS),
//     };
// }
