//! This module sets the options for our audit tool and records package's building environments.

use basic_usages::external::fxhash::{FxHashMap as HashMap, FxHashSet as HashSet};

mod r#impl;

#[derive(Debug)]
pub struct BuildConfig<'c> {
    // host triple
    host: String,
    // rustup home
    rustup_home: String,
    // cargo home
    #[allow(unused)]
    cargo_home: String,
    // current configured rust version
    rust_version: u32,
    // cargo configurations during building
    cargo_args: Option<&'c [String]>,
    // dependency configurations during building
    crates_cfgs: HashMap<String, HashSet<String>>,

    // fix with rustc and minimal dep tree, which is the quickest way (default false)
    quick_fix: bool,
    // print check details
    verbose: bool,
    // test mode, not provided to user
    test: bool,
}
