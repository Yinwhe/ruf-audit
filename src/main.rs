#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_error_codes;
extern crate rustc_errors;
extern crate rustc_hash;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_lint;
extern crate rustc_save_analysis;
extern crate rustc_session;
extern crate rustc_span;

use core::sync::atomic::Ordering;
use std::{
    env,
    io::{self, Read},
    path::PathBuf,
    process,
};

use rustc_driver::{
    args, catch_with_exit_code, describe_lints, diagnostics_registry, pretty,
    Callbacks, Compilation, TimePassesCallbacks,
};
use rustc_errors::ErrorGuaranteed;
use rustc_interface::interface;
use rustc_save_analysis::DumpHandler;
use rustc_session::{
    config, config::ErrorOutputType, config::Input, config::OutputType, early_error,
    early_error_no_abort, getopts
};
use rustc_span::{def_id::LOCAL_CRATE, FileName};

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
        run_compiler(&args, &mut callbacks)
    });

    process::exit(exit_code)
}

fn run_compiler(
    at_args: &[String],
    callbacks: &mut (dyn Callbacks + Send),
) -> interface::Result<()> {
    let args = args::arg_expand_all(at_args);

    let Some(matches) = handle_options(&args) else { return Ok(()) };

    let sopts = config::build_session_options(&matches);

    // if let Some(ref code) = matches.opt_str("explain") {
    //     handle_explain(diagnostics_registry(), code, sopts.error_format);
    //     return Ok(());
    // }

    let cfg = interface::parse_cfgspecs(matches.opt_strs("cfg"));
    let check_cfg = interface::parse_check_cfg(matches.opt_strs("check-cfg"));
    // let (odir, ofile) = make_output(&matches);

    println!("cfg: {:?}", cfg);
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
    };

    match make_input(config.opts.error_format, &matches.free) {
        Err(reported) => return Err(reported),
        Ok(Some(input)) => {
            config.input = input;

            callbacks.config(&mut config);
        }
        Ok(None) => match matches.free.len() {
            0 => early_error(config.opts.error_format, "no input filename given"),
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

        let linker = compiler.enter(|queries| {
            let early_exit = || sess.compile_status().map(|_| None);
            queries.parse()?;

            if let Some(ppm) = &sess.opts.pretty {
                if ppm.needs_ast_map() {
                    queries.global_ctxt()?.enter(|tcx| {
                        pretty::print_after_hir_lowering(tcx, *ppm);
                        Ok(())
                    })?;
                } else {
                    let krate = queries.parse()?.steal();
                    pretty::print_after_parsing(sess, &krate, *ppm);
                }
                return early_exit();
            }

            if callbacks.after_parsing(compiler, queries) == Compilation::Stop {
                return early_exit();
            }

            if sess.opts.unstable_opts.parse_only || sess.opts.unstable_opts.show_span.is_some() {
                return early_exit();
            }

            // // NOTE HERE
            // if sess.opts.ruf_analysis {
            //     my_plugin(queries);
            //     return early_exit();
            // }

            {
                let plugins = queries.register_plugins()?;
                let (_, lint_store) = &*plugins.borrow();

                // Lint plugins are registered; now we can process command line flags.
                if sess.opts.describe_lints {
                    describe_lints(sess, lint_store, true);
                    return early_exit();
                }
            }

            // Make sure name resolution and macro expansion is run.
            queries.global_ctxt()?;

            if callbacks.after_expansion(compiler, queries) == Compilation::Stop {
                return early_exit();
            }

            // Make sure the `output_filenames` query is run for its side
            // effects of writing the dep-info and reporting errors.
            queries.global_ctxt()?.enter(|tcx| tcx.output_filenames(()));

            if sess.opts.output_types.contains_key(&OutputType::DepInfo)
                && sess.opts.output_types.len() == 1
            {
                return early_exit();
            }

            if sess.opts.unstable_opts.no_analysis {
                return early_exit();
            }

            queries.global_ctxt()?.enter(|tcx| {
                let result = tcx.analysis(());
                if sess.opts.unstable_opts.save_analysis {
                    let crate_name = tcx.crate_name(LOCAL_CRATE);
                    sess.time("save_analysis", || {
                        rustc_save_analysis::process_crate(
                            tcx,
                            crate_name,
                            &sess.io.input,
                            None,
                            DumpHandler::new(sess.io.output_dir.as_deref(), crate_name),
                        )
                    });
                }
                result
            })?;

            if callbacks.after_analysis(compiler, queries) == Compilation::Stop {
                return early_exit();
            }

            queries.ongoing_codegen()?;

            if sess.opts.unstable_opts.print_type_sizes {
                sess.code_stats.print_type_sizes();
            }

            let linker = queries.linker()?;
            Ok(Some(linker))
        })?;

        if let Some(linker) = linker {
            let _timer = sess.timer("link");
            linker.link()?
        }

        if sess.opts.unstable_opts.perf_stats {
            sess.print_perf_stats();
        }

        if sess.opts.unstable_opts.print_fuel.is_some() {
            eprintln!(
                "Fuel used by {}: {}",
                sess.opts.unstable_opts.print_fuel.as_ref().unwrap(),
                sess.print_fuel.load(Ordering::SeqCst)
            );
        }

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

fn handle_options(args: &[String]) -> Option<getopts::Matches> {
    // Throw away the first argument, the name of the binary
    let args = &args[1..];

    if args.is_empty() {
        // user did not write `-v` nor `-Z unstable-options`, so do not
        // include that extra information.
        // let nightly_build =
        //     rustc_feature::UnstableFeatures::from_environment(None).is_nightly_build();
        // usage(false, false, nightly_build);
        return None;
    }

    // Parse with *all* options defined in the compiler, we don't worry about
    // option stability here we just want to parse as much as possible.
    let mut options = getopts::Options::new();
    for option in config::rustc_optgroups() {
        (option.apply)(&mut options);
    }
    let matches = options.parse(args).unwrap_or_else(|e| {
        let msg = match e {
            getopts::Fail::UnrecognizedOption(ref opt) => CG_OPTIONS
                .iter()
                .map(|&(name, ..)| ('C', name))
                .chain(Z_OPTIONS.iter().map(|&(name, ..)| ('Z', name)))
                .find(|&(_, name)| *opt == name.replace('_', "-"))
                .map(|(flag, _)| format!("{e}. Did you mean `-{flag} {opt}`?")),
            _ => None,
        };
        early_error(ErrorOutputType::default(), &msg.unwrap_or_else(|| e.to_string()));
    });

    // // For all options we just parsed, we check a few aspects:
    // //
    // // * If the option is stable, we're all good
    // // * If the option wasn't passed, we're all good
    // // * If `-Z unstable-options` wasn't passed (and we're not a -Z option
    // //   ourselves), then we require the `-Z unstable-options` flag to unlock
    // //   this option that was passed.
    // // * If we're a nightly compiler, then unstable options are now unlocked, so
    // //   we're good to go.
    // // * Otherwise, if we're an unstable option then we generate an error
    // //   (unstable option being used on stable)
    // nightly_options::check_nightly_options(&matches, &config::rustc_optgroups());

    // if matches.opt_present("h") || matches.opt_present("help") {
    //     // Only show unstable options in --help if we accept unstable options.
    //     let unstable_enabled = nightly_options::is_unstable_enabled(&matches);
    //     let nightly_build = nightly_options::match_is_nightly_build(&matches);
    //     usage(matches.opt_present("verbose"), unstable_enabled, nightly_build);
    //     return None;
    // }

    // // Handle the special case of -Wall.
    // let wall = matches.opt_strs("W");
    // if wall.iter().any(|x| *x == "all") {
    //     print_wall_help();
    //     rustc_errors::FatalError.raise();
    // }

    // // Don't handle -W help here, because we might first load plugins.
    // let debug_flags = matches.opt_strs("Z");
    // if debug_flags.iter().any(|x| *x == "help") {
    //     describe_debug_flags();
    //     return None;
    // }

    // let cg_flags = matches.opt_strs("C");

    // if cg_flags.iter().any(|x| *x == "help") {
    //     describe_codegen_flags();
    //     return None;
    // }

    // if cg_flags.iter().any(|x| *x == "no-stack-check") {
    //     early_warn(
    //         ErrorOutputType::default(),
    //         "the --no-stack-check flag is deprecated and does nothing",
    //     );
    // }

    // if cg_flags.iter().any(|x| *x == "passes=list") {
    //     let backend_name = debug_flags.iter().find_map(|x| x.strip_prefix("codegen-backend="));
    //     get_codegen_backend(&None, backend_name).print_passes();
    //     return None;
    // }

    // if matches.opt_present("version") {
    //     version!("rustc", &matches);
    //     return None;
    // }

    Some(matches)
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
