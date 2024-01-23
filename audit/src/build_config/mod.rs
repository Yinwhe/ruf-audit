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
}
