use std::env;
use std::io::Write;
use std::process::{Command, Stdio};

use basic_usages::external::fxhash::{FxHashMap as HashMap, FxHashSet as HashSet};
use basic_usages::ruf_check_info::{CondRufs, UsedRufs};
use basic_usages::ruf_lifetime::{get_ruf_all_status, get_ruf_status, RUSTC_VER_NUM};

use super::BuildConfig;
use crate::error::AuditError;
use crate::scanner;
use crate::RE_USEDFEATS;

impl<'short, 'long: 'short> BuildConfig<'long> {
    pub fn default() -> Result<Self, AuditError> {
        let mut rustup = Command::new("rustup");
        rustup.arg("show");

        let output = rustup
            .output()
            .map_err(|e| AuditError::Unexpected(format!("cannot run rustup: {e}")))?;

        let profiles = if output.status.success() {
            String::from_utf8_lossy(&output.stdout)
        } else {
            return Err(AuditError::Unexpected(
                "cannot fetch rustup profiles".to_string(),
            ));
        };

        let host = {
            let host_line = profiles
                .lines()
                .find(|line| line.starts_with("Default host:"))
                .ok_or_else(|| {
                    AuditError::Unexpected(format!("cannot fetch rustup default host"))
                })?;

            host_line[13..].trim().to_string()
        };

        let rustup_home = {
            let rustup_home_line = profiles
                .lines()
                .find(|line| line.starts_with("rustup home:"))
                .ok_or_else(|| AuditError::Unexpected(format!("cannot fetch rustup home")))?;

            rustup_home_line[13..].trim().to_string()
        };

        let cargo_home = if let Ok(cargo_home) = env::var("CARGO_HOME") {
            cargo_home
        } else {
            env::var("HOME")
                .map_err(|_| AuditError::Unexpected(format!("cannot fetch cargo home")))?
                + "/.cargo"
        };

        let crates_cfgs = HashMap::default();

        Ok(BuildConfig {
            host,
            rustup_home,
            cargo_home,
            rust_version: 63,
            cargo_args: None,
            crates_cfgs,

            newer_fix: false,
            quick_fix: false,
            verbose: false,
            test: false,
        })
    }

    pub fn update_build_cfgs(&mut self, crate_name: String, cfgs: HashSet<String>) {
        // println!("[Debug - update_build_cfgs] {crate_name}");
        // for cfg in &cfgs {
        //     println!("[Debug - update_build_cfgs] {cfg}");
        // }
        self.crates_cfgs.insert(crate_name, cfgs);
    }

    pub fn update_cargo_args(&mut self, cargo_args: &'long [String]) {
        self.cargo_args = Some(cargo_args)
    }

    /// Filter used rufs in current configurations
    pub fn filter_rufs(&self, crate_name: &str, rufs: CondRufs) -> Result<UsedRufs, AuditError> {
        // let tmp_rsfile_path = self.get_tmp_rsfile();
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
            let cfgs = self
                .crates_cfgs
                .get(crate_name)
                .expect(&format!("Fatal, no cfgs found with {crate_name}"));

            // println!("[Debug - filter_rufs] {crate_name} {cfgs:?}");
            let mut cfg_args = String::new();
            for cfg in cfgs {
                cfg_args.push_str(&format!("--cfg {}", cfg));
            }

            let scanner = scanner()
                .args(["--", "-"])
                .arg(cfg_args)
                .stdin(Stdio::piped())
                .env("LD_LIBRARY_PATH", self.get_rustlib_path())
                .spawn()
                .expect("Fatal, cannot spawn scanner output");

            {
                let mut stdin = scanner
                    .stdin
                    .as_ref()
                    .expect("Fatal, cannot fetch scanner stdin");
                stdin
                    .write_all(content.as_bytes())
                    .expect("Fatal, cannot write to scanner stdin");
            }

            let output = scanner
                .wait_with_output()
                .expect("Fatal, cannot fetch scanner output");

            if !output.status.success() {
                return Err(AuditError::Unexpected(format!(
                    "cannot check candidate rufs: {}",
                    String::from_utf8_lossy(&output.stderr)
                )));
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
            .filter(|ruf| !get_ruf_status(ruf, self.rust_version).is_usable())
            .count()
            > 0
        {
            return false;
        }

        return true;
    }

    /// Get usable rustc versions for rufs
    pub fn usable_rustc_for_rufs(&self, rufs: &UsedRufs) -> HashSet<u32> {
        let mut usable_rustc = HashSet::from_iter(0..RUSTC_VER_NUM as u32);
        for ruf in rufs.iter() {
            let ur = get_ruf_all_status(ruf)
                .into_iter()
                .enumerate()
                .filter(|(_, status)| status.is_usable())
                .map(|(ver, _)| ver as u32)
                .collect();
            usable_rustc = usable_rustc.intersection(&ur).cloned().collect();
        }

        usable_rustc
    }

    pub fn get_rustlib_path(&self) -> String {
        format!(
            "{rustup_home}/toolchains/nightly-2023-12-12-{host}/lib/rustlib/{host}/lib",
            rustup_home = self.rustup_home,
            host = self.host
        )
    }

    pub fn get_audit_rustc_path(&self) -> String {
        format!(
            "{rustup_home}/toolchains/nightly-2023-12-12-{host}/bin/rustc",
            rustup_home = self.rustup_home,
            host = self.host
        )
    }

    pub fn get_cargo_args(&'long self) -> Option<&'short [String]> {
        self.cargo_args
    }

    #[inline]
    pub fn set_newer_fix(&mut self, newer_fix: bool) {
        self.newer_fix = newer_fix
    }

    #[inline]
    pub fn is_newer_fix(&self) -> bool {
        self.newer_fix
    }

    #[inline]
    pub fn set_quick_fix(&mut self, quick_fix: bool) {
        self.quick_fix = quick_fix
    }

    #[inline]
    pub fn is_quick_fix(&self) -> bool {
        self.quick_fix
    }

    #[inline]
    pub fn set_verbose(&mut self, verbose: bool) {
        self.verbose = verbose
    }

    #[inline]
    pub fn is_verbose(&self) -> bool {
        self.verbose
    }

    #[inline]
    pub fn set_test(&mut self, test: bool) {
        self.test = test
    }

    #[inline]
    pub fn is_test(&self) -> bool {
        self.test
    }
}

// #[test]
// fn test() {
//     let config = BuildConfig::default();
//     println!("{:?}", config.unwrap().get_rustlib_path());
// }
