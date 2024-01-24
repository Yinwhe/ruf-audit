// Use nightly-2023-12-12
use std::env;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Arc;

use basic_usages::ruf_check_info::{CheckInfo, UsedRufs};
use rustc_driver::{
    args, catch_with_exit_code, diagnostics_registry, handle_options, Callbacks, Compilation,
    TimePassesCallbacks, DEFAULT_LOCALE_RESOURCES,
};

use rustc_ast::{self as ast, Attribute};
use rustc_errors::ErrorGuaranteed;
use rustc_feature::Features;
use rustc_interface::interface;
use rustc_session::config::{self, ErrorOutputType, Input, OutFileName};
use rustc_session::getopts::Matches;
use rustc_session::EarlyErrorHandler;
use rustc_span::symbol::sym;
use rustc_span::FileName;

pub fn run_rustc(args: &Vec<String>, output_buildinfo: bool) -> i32 {
    let mut callbacks = TimePassesCallbacks::default();

    let exit_code = catch_with_exit_code(|| {
        if output_buildinfo {
            run_compiler(true, args, &mut callbacks)
        } else {
            run_compiler(false, args, &mut callbacks)
        }
    });

    exit_code
}

// Is it correct?
fn run_compiler(
    output_buildinfo: bool,
    at_args: &[String],
    callbacks: &mut (dyn Callbacks + Send),
) -> interface::Result<()> {
    // println!("AUDIT DEBUG: args: {at_args:?}");
    let mut default_handler = EarlyErrorHandler::new(ErrorOutputType::default());

    // let at_args = at_args.get(1..).unwrap_or_default();

    let args = args::arg_expand_all(&default_handler, at_args);

    let Some(matches) = handle_options(&default_handler, &args) else {
        return Ok(());
    };

    let sopts = config::build_session_options(&mut default_handler, &matches);

    let crate_name: Vec<String> = matches.opt_strs("crate-name");

    let (odir, ofile) = make_output(&matches);
    let mut config = interface::Config {
        opts: sopts,
        crate_cfg: matches.opt_strs("cfg"),
        crate_check_cfg: matches.opt_strs("check-cfg"),
        input: Input::File(PathBuf::new()),
        output_file: ofile,
        output_dir: odir,
        ice_file: None,
        file_loader: None,
        locale_resources: DEFAULT_LOCALE_RESOURCES,
        lint_caps: Default::default(),
        parse_sess_created: None,
        hash_untracked_state: None,
        register_lints: None,
        override_queries: None,
        make_codegen_backend: None,
        registry: diagnostics_registry(),
        using_internal_features: Arc::default(),
        expanded_args: args,
    };

    let has_input = match make_input(&default_handler, &matches.free) {
        Err(reported) => return Err(reported),
        Ok(Some(input)) => {
            config.input = input;
            true // has input: normal compilation
        }
        Ok(None) => match matches.free.len() {
            0 => false, // no input: we will exit early
            1 => panic!("make_input should have provided valid inputs"),
            _ => default_handler.early_error(format!(
                "multiple input filenames provided (first two filenames are `{}` and `{}`)",
                matches.free[0], matches.free[1],
            )),
        },
    };

    callbacks.config(&mut config);

    default_handler.abort_if_errors();
    drop(default_handler);

    let res = interface::run_compiler(config, |compiler| {
        let sess = &compiler.sess;

        let handler = EarlyErrorHandler::new(sess.opts.error_format);

        if !has_input {
            handler.early_error("no input filename given"); // this is fatal
        }

        let features = compiler.enter(|queries| {
            let early_exit = || sess.compile_status().map(|_| None);

            queries.parse()?;

            if callbacks.after_crate_root_parsing(compiler, queries) == Compilation::Stop {
                return early_exit();
            }

            let sess = &compiler.sess;

            let krate = queries.parse()?.steal();

            let pre_configured_attrs =
                rustc_expand::config::pre_configure_attrs(sess, &krate.attrs);

            // parse `#[crate_name]` even if `--crate-name` was passed, to make sure it matches.
            // let crate_name = find_crate_name(sess, &pre_configured_attrs);

            let f = features(&pre_configured_attrs).declared_features;

            // queries.global_ctxt()?;

            // if callbacks.after_expansion(compiler, queries) == Compilation::Stop {
            //     return early_exit();
            // }

            // let f = queries
            //     .global_ctxt()?
            //     .enter(|tcx| tcx.features().declared_features.clone());

            Ok(Some(f))
        })?;

        let used_rufs = features
            .map(|mut feats| feats.drain().map(|sym| sym.to_string()).collect())
            .unwrap_or(Vec::new());

        if output_buildinfo {
            assert!(crate_name.len() == 1, "Fatal, fetch crate name errors");
            let build_info = CheckInfo {
                crate_name: crate_name.first().unwrap().clone(),
                used_rufs: UsedRufs::new(used_rufs),
                cfg: matches.opt_strs("cfg"),
            };

            println!("{}", build_info);
        } else {
            println!("{}", UsedRufs::new(used_rufs));
        }

        Ok(())
    });

    return res;
}

/// Extract input (string or file and optional path) from matches.
/// Copy from rustc_driver_impl crates.
fn make_input(
    handler: &EarlyErrorHandler,
    free_matches: &[String],
) -> Result<Option<Input>, ErrorGuaranteed> {
    if free_matches.len() == 1 {
        let ifile = &free_matches[0];
        if ifile == "-" {
            let mut src = String::new();
            if io::stdin().read_to_string(&mut src).is_err() {
                // Immediately stop compilation if there was an issue reading
                // the input (for example if the input stream is not UTF-8).
                let reported = handler.early_error_no_abort(
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

/// Extract output directory and file from matches.
/// Copy from rustc_driver_impl crates.
fn make_output(matches: &Matches) -> (Option<PathBuf>, Option<OutFileName>) {
    let odir = matches.opt_str("out-dir").map(|o| PathBuf::from(&o));
    let ofile = matches.opt_str("o").map(|o| match o.as_str() {
        "-" => OutFileName::Stdout,
        path => OutFileName::Real(PathBuf::from(path)),
    });
    (odir, ofile)
}

/// Procecss features used.
/// Copy and modify from rustc_expand crates.
fn features(krate_attrs: &[Attribute]) -> Features {
    fn feature_list(attr: &Attribute) -> Vec<ast::NestedMetaItem> {
        if attr.has_name(sym::feature)
            && let Some(list) = attr.meta_item_list()
        {
            list.to_vec()
        } else {
            Vec::new()
        }
    }

    let mut features = Features::default();

    // Process all features declared in the code.
    for attr in krate_attrs {
        for mi in feature_list(attr) {
            let name = match mi.ident() {
                Some(ident) if mi.is_word() => ident.name,
                _ => continue,
            };

            // We simply record all features.
            features.set_declared_lib_feature(name, mi.span());
        }
    }

    features
}

#[allow(unused)]
fn run_actual_rustc(
    rustc_path: &str,
    rustc_args: &[String],
    file_contents: &str,
) -> Result<i32, String> {
    let mut rustc = Command::new(rustc_path);

    let empty_rs = format!("#[allow(unused)]\nfn main(){{let hash = \"{file_contents}\";}}");
    let mut rustc = rustc
        .args(rustc_args)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot spawn: {e}"))?;

    if let Some(mut stdin) = rustc.stdin.take() {
        stdin
            .write_all(empty_rs.as_bytes())
            .map_err(|e| format!("cannot write empty fn into rustc stdin: {e}"))?;
    } else {
        return Err(format!("cannot open rustc stdin"));
    }

    let output = rustc
        .wait_with_output()
        .map_err(|e| format!("cannot wait rustc output: {e}"))?;

    Ok(output.status.code().unwrap_or(0))
}
