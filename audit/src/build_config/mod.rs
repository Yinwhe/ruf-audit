use std::path::PathBuf;

use basic_usages::external::fxhash::{FxHashMap as HashMap, FxHashSet as HashSet};

mod r#impl;

#[derive(Debug)]
pub struct BuildConfig {
    host: String,
    rustup_home: String,
    cargo_home: String,
    rust_version: u32,

    tmp_rsfile: PathBuf,

    crates_cfgs: HashMap<String, HashSet<String>>,
}
