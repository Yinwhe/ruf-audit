use std::env;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

use basic_usages::external::fxhash::{FxHashMap as HashMap, FxHashSet as HashSet};
use basic_usages::ruf_check_info::{CondRufs, UsedRufs};

use super::BuildConfig;
use crate::scanner;
use crate::utils::RE_USEDFEATS;

impl BuildConfig {
    pub fn default() -> Result<Self, String> {
        let mut rustup = Command::new("rustup");
        rustup.arg("show");

        let output = rustup
            .output()
            .map_err(|e| format!("Fatal, cannot fetch rustup profiles: {}", e))?;

        let profiles = if output.status.success() {
            String::from_utf8_lossy(&output.stdout)
        } else {
            return Err(format!("Fatal, cannot fetch rustup profiles"));
        };

        let host = {
            let host_line = profiles
                .lines()
                .find(|line| line.starts_with("Default host:"))
                .ok_or_else(|| format!("Fatal, cannot fetch rustup default host"))?;

            host_line[13..].trim().to_string()
        };

        let rustup_home = {
            let rustup_home_line = profiles
                .lines()
                .find(|line| line.starts_with("rustup home:"))
                .ok_or_else(|| format!("Fatal, cannot fetch rustup home"))?;

            rustup_home_line[13..].trim().to_string()
        };

        let cargo_home = if let Ok(cargo_home) = env::var("CARGO_HOME") {
            cargo_home
        } else {
            env::var("HOME").map_err(|_| format!("Fatal, cannot fetch cargo home"))? + "/.cargo"
        };

        let tmp_rsfile = PathBuf::from(&rustup_home).join("tmp/audit_tmp.rs");

        let crates_cfgs = HashMap::default();

        Ok(BuildConfig {
            host,
            rustup_home,
            cargo_home,
            rust_version: 63,
            tmp_rsfile,
            crates_cfgs,
        })
    }

    pub fn update_buildinfo(&mut self, crate_name: String, cfgs: HashSet<String>) {
        let entry = self
            .crates_cfgs
            .entry(crate_name)
            .or_insert_with(HashSet::default);
        entry.extend(cfgs);
    }

    /// Filter used rufs in current configurations
    pub fn filter_rufs(&self, rufs: CondRufs) -> Result<UsedRufs, String> {
        let tmp_rsfile_path = self.get_tmp_rsfile();
        let mut content = String::new();
        let mut used_rufs = UsedRufs::empty();

        for ruf in rufs.into_iter() {
            if let Some(cond) = &ruf.cond {
                content.push_str(&format!(
                    "#[cfg_attr({}, feature({}))]\n",
                    cond, ruf.feature
                ))
            } else {
                used_rufs.push(ruf.feature);
            }
        }

        // use scanner to check cfg rufs
        if !content.is_empty() {
            let mut f = File::open(tmp_rsfile_path).expect("Fatal, cannot open tmp file");
            f.write_all(content.as_bytes())
                .expect("Fatal, cannot write tmp file");

            let output = scanner()
                .args(["--", tmp_rsfile_path])
                .env("LD_LIBRARY_PATH", self.get_rustlib_path())
                .output()
                .expect("Fatal, cannot fetch scanner output");

            if !output.status.success() {
                return Err(format!(
                    "Fatal, cannot check candidate rufs: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }

            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(caps) = RE_USEDFEATS.captures(&stdout) {
                used_rufs.extend(UsedRufs::from(
                    caps.get(1).expect("Fatal, resolve ruf fails").as_str(),
                ));
            }
        }

        Ok(used_rufs)
    }

    /// Check whether rufs is usable in current configurations
    pub fn rufs_usable(&self, rufs: &UsedRufs) -> bool {
        assert!(self.rust_version < basic_usages::ruf_lifetime::RUSTC_VER_NUM as u32);
        if rufs
            .iter()
            .filter(|ruf| {
                !basic_usages::ruf_lifetime::get_ruf_status(ruf, self.rust_version).is_usable()
            })
            .count()
            > 0
        {
            return false;
        }

        return true;
    }

    pub fn get_rustlib_path(&self) -> String {
        format!(
            "{rustup_home}/toolchains/nightly-2023-12-12-{host}/lib/rustlib/{host}/lib",
            rustup_home = self.rustup_home,
            host = self.host
        )
    }

    fn get_tmp_rsfile(&self) -> &str {
        if !self.tmp_rsfile.exists() {
            let _file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&self.tmp_rsfile)
                .expect("Fatal, cannot create tmp file");
        }

        self.tmp_rsfile
            .to_str()
            .expect("Fatal, cannot convert tmp file path to str")
    }
}

// #[test]
// fn test() {
//     let config = BuildConfig::default();
//     println!("{:?}", config.unwrap().get_rustlib_path());
// }
