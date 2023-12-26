// Use nightly-2023-03-12
use std::{
    env,
    io::{self, Read},
    path::PathBuf,
};

use rustc_driver::{args, diagnostics_registry, handle_options, Callbacks, Compilation};
use rustc_driver::{catch_with_exit_code, TimePassesCallbacks};
use rustc_errors::ErrorGuaranteed;
use rustc_interface::interface;
use rustc_session::{
    config, config::ErrorOutputType, config::Input, early_error, early_error_no_abort,
};
use rustc_span::FileName;
// use rustc_ast::CRATE_NODE_ID;

pub fn run_rustc() -> i32 {
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
        run_compiler(&args, &mut callbacks)
    });

    exit_code
}

fn run_compiler(
    at_args: &[String],
    callbacks: &mut (dyn Callbacks + Send),
) -> interface::Result<()> {
    let args = args::arg_expand_all(at_args);

    let Some(matches) = handle_options(&args) else { return Ok(()) };

    let sopts = config::build_session_options(&matches);

    let cfg = interface::parse_cfgspecs(matches.opt_strs("cfg"));
    let check_cfg = interface::parse_check_cfg(matches.opt_strs("check-cfg"));

    // println!("cfg: {:?}", cfg);
    // println!("check-cfg: {:?}", check_cfg);
    let mut config = interface::Config {
        opts: sopts,
        crate_cfg: cfg,
        crate_check_cfg: check_cfg,
        input: Input::File(PathBuf::new()),
        output_file: None,
        output_dir: None,
        file_loader: None,
        lint_caps: Default::default(),
        parse_sess_created: None,
        register_lints: None,
        override_queries: None,
        make_codegen_backend: None,
        registry: diagnostics_registry(),
        locale_resources: rustc_driver::DEFAULT_LOCALE_RESOURCES
    };

    match make_input(config.opts.error_format, &matches.free) {
        Err(reported) => return Err(reported),
        Ok(Some(input)) => {
            config.input = input;

            callbacks.config(&mut config);
        }
        Ok(None) => match matches.free.len() {
            0 => {
                callbacks.config(&mut config);
                interface::run_compiler(config, |compiler| {
                    let sopts = &compiler.session().opts;
                    early_error(sopts.error_format, "no input filename given")
                });
                return Ok(());
            }
            1 => panic!("make_input should have provided valid inputs"),
            _ => early_error(
                config.opts.error_format,
                &format!(
                    "multiple input filenames provided (first two filenames are `{}` and `{}`)",
                    matches.free[0], matches.free[1],
                ),
            ),
        },
    };

    interface::run_compiler(config, |compiler| {
        let sess = compiler.session();

        compiler.enter(|queries| {
            let early_exit = || sess.compile_status();

            queries.parse()?;

            if callbacks.after_parsing(compiler, queries) == Compilation::Stop {
                return early_exit();
            }

            println!("Debug 1");

            queries.register_plugins()?;

            queries.global_ctxt()?;

            println!("Debug 2");

            if callbacks.after_expansion(compiler, queries) == Compilation::Stop {
                return early_exit();
            }

            queries.global_ctxt()?.enter(|tcx| {
                let features = tcx.features();
                println!("{:?}", features);
            });


            Ok(())
        })?;

        Ok(())
    })
}

/// Extract input (string or file and optional path) from matches.
/// Copy from rustc_driver_impl crates.
fn make_input(
    error_format: ErrorOutputType,
    free_matches: &[String],
) -> Result<Option<Input>, ErrorGuaranteed> {
    if free_matches.len() == 1 {
        let ifile = &free_matches[0];
        if ifile == "-" {
            let mut src = String::new();
            if io::stdin().read_to_string(&mut src).is_err() {
                // Immediately stop compilation if there was an issue reading
                // the input (for example if the input stream is not UTF-8).
                let reported = early_error_no_abort(
                    error_format,
                    "couldn't read from stdin, as it did not contain valid UTF-8",
                );
                return Err(reported);
            }
            if let Ok(path) = env::var("UNSTABLE_RUSTDOC_TEST_PATH") {
                let line = env::var("UNSTABLE_RUSTDOC_TEST_LINE").expect(
                    "when UNSTABLE_RUSTDOC_TEST_PATH is set \
                                    UNSTABLE_RUSTDOC_TEST_LINE also needs to be set",
                );
                let line = isize::from_str_radix(&line, 10)
                    .expect("UNSTABLE_RUSTDOC_TEST_LINE needs to be an number");
                let file_name = FileName::doc_test_source_code(PathBuf::from(path), line);
                Ok(Some(Input::Str {
                    name: file_name,
                    input: src,
                }))
            } else {
                Ok(Some(Input::Str {
                    name: FileName::anon_source_code(&src),
                    input: src,
                }))
            }
        } else {
            Ok(Some(Input::File(PathBuf::from(ifile))))
        }
    } else {
        Ok(None)
    }
}

// /// Process command line options.
// /// Copy and modify from rustc_driver_impl crates.
// fn handle_options(args: &[String]) -> Option<getopts::Matches> {
//     // Throw away the first argument, the name of the binary
//     let args = &args[1..];

//     if args.is_empty() {
//         // No args input.
//         return None;
//     }

//     const NEEDED_OPTION: [&str; 10] = [
//         "cfg",
//         "check-cfg",
//         "L",
//         "crate-type",
//         "edition",
//         "target",
//         "version",
//         "extern",
//         "sysroot",
//         "help",
//     ];

//     let mut options = getopts::Options::new();
//     for option in config::rustc_optgroups() {
//         // We only parse some options.
//         if NEEDED_OPTION.contains(&option.name) {
//             // println!("name: {:?}, status: {:?}", option.name, option.stability);
//             (option.apply)(&mut options);
//         }
//     }

//     let matches = options.parse(args).unwrap_or_else(|e| {
//         early_error(ErrorOutputType::default(), &e.to_string());
//     });

//     // TODO: write help docs
//     if matches.opt_present("h") || matches.opt_present("help") {
//         println!("help tbc...");
//         return None;
//     }

//     if matches.opt_present("version") {
//         println!("ruf-audit 0.1.0");
//         return None;
//     }

//     Some(matches)
// }
