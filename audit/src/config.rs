use std::{env, fs::OpenOptions, path::PathBuf, process::Command};

#[derive(Debug)]
pub struct AuditConfig {
    host: String,

    rustup_home: String,
    cargo_home: String,

    rust_version: u32,

    tmp_rsfile: PathBuf,
}

impl AuditConfig {
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

        Ok(AuditConfig {
            host,
            rustup_home,
            cargo_home,
            rust_version: 63,
            tmp_rsfile,
        })
    }

    pub fn get_rustlib_path(&self) -> String {
        format!(
            "{rustup_home}/toolchains/nightly-2023-12-12-{host}/lib/rustlib/{host}/lib",
            rustup_home = self.rustup_home,
            host = self.host
        )
    }

    pub fn get_tmp_rsfile(&self) -> &str {
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

    pub fn get_rust_version(&self) -> u32 {
        self.rust_version
    }
}

// #[test]
// fn test() {
//     let config = AuditConfig::default();
//     println!("{:?}", config.unwrap().get_rustlib_path());
// }
