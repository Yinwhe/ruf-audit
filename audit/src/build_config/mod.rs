use basic_usages::external::fxhash::{FxHashMap as HashMap, FxHashSet as HashSet};

mod r#impl;

#[derive(Debug)]
pub struct BuildConfig<'c> {
    // user home
    host: String,
    // rustup home
    rustup_home: String,
    // cargo home
    cargo_home: String,
    // current enabled rustc version
    rust_version: u32,
    
    // cargo args when checking
    cargo_args: Option<&'c [String]>,
    // dep crate build cfgs
    crates_cfgs: HashMap<String, HashSet<String>>,

    // user configs
    // each fix we choose newer one or older one (default)
    newer_fix: bool,
    // fix with rustc and minimal dep tree, which is the quickest way (default false)
    quick_fix: bool,
}
