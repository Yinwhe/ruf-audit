use std::env::current_exe;

// use slog::debug;

use basic_usages::external::fxhash::{FxHashMap as HashMap, FxHashSet as HashSet};
use basic_usages::ruf_check_info::{CheckInfo, UsedRufs};

use crate::build_config::BuildConfig;
use crate::error::AuditError;
// use crate::{LOGGER, error_print, info_print};
use crate::{cargo, RE_CHECKINFO};

pub fn extract(
    config: &mut BuildConfig,
    // recheck: bool,
) -> Result<HashMap<String, UsedRufs>, AuditError> {
    let mut cargo = cargo();
    cargo.arg("check");
    if let Some(cargo_args) = config.get_cargo_args() {
        cargo.args(cargo_args);
    }

    let path = current_exe()
        .map_err(|_| AuditError::Unexpected(format!("cannot get current exe path")))?;
    cargo.env("RUSTC_WRAPPER", &path);

    let cargo = cargo
        .output()
        .map_err(|e| AuditError::Unexpected(format!("cannot spawn cargo process: {e}")))?;

    if !cargo.status.success() {
        let stderr = String::from_utf8_lossy(&cargo.stderr);
        let err = stderr
            .lines()
            .into_iter()
            .find(|line| line.trim().starts_with("error"));

        return Err(AuditError::Unexpected(format!(
            "cargo process run fails: {err:?}",
            err = err.unwrap_or(&"unknown error")
        )));
    }

    let stdout = String::from_utf8_lossy(&cargo.stdout);
    let mut checkinfos = HashMap::default();

    // resolves used rufs from stdout
    for cap in RE_CHECKINFO.captures_iter(&stdout) {
        let info = CheckInfo::from(cap.get(1).expect("Fatal, resolve buildinfo fails").as_str());
        let entry = checkinfos
            .entry(info.crate_name)
            .or_insert_with(|| (HashSet::default(), HashSet::default()));

        entry.0.extend(info.used_rufs.into_iter());
        entry.1.extend(
            info.cfg
                .into_iter()
                .map(|cfg| cfg.escape_default().to_string()),
        );
    }

    let mut used_rufs = HashMap::default();
    for (crate_name, (rufs, cfgs)) in checkinfos {
        config.update_build_cfgs(crate_name.clone(), cfgs);
        used_rufs.insert(crate_name, UsedRufs::new(rufs.into_iter().collect()));
    }

    Ok(used_rufs)
}

// /// rufs usage extract, based on `cargo check`
// pub fn extract(
//     config: &mut BuildConfig,
//     recheck: bool,
// ) -> Result<HashMap<String, UsedRufs>, String> {
//     let mut cmd = cargo();
//     cmd.arg("rustc");
//     if let Some(cargo_args) = config.get_cargo_args() {
//         cmd.args(cargo_args);
//     }

//     let path = current_exe().map_err(|_| format!("cannot get current exe path"))?;
//     cmd.env("RUSTC_WRAPPER", &path);

//     let mut child = cmd
//         .stdout(Stdio::piped())
//         .stderr(Stdio::piped())
//         .spawn()
//         .map_err(|_| format!("cannot spawn cargo process"))?;

//     // Listen to the cargo process
//     let stdout = BufReader::new(child.stdout.take().unwrap());
//     let stderr = BufReader::new(child.stderr.take().unwrap());

//     // Resolving stdout and stderr infos
//     let stdout_handle = thread::spawn(move || {
//         let mut checkinfos: HashMap<String, (HashSet<String>, HashSet<String>)> =
//             HashMap::default();

//         for line in stdout.lines() {
//             // debug!(LOGGER, "Stdout: {line:?}");
//             let line = line.expect("Fatal, get stdout line fails");

//             if let Some(caps) = RE_CHECKINFO.captures(&line) {
//                 let info = CheckInfo::from(
//                     caps.get(1)
//                         .expect("Fatal, resolve buildinfo fails")
//                         .as_str(),
//                 );
//                 let entry = checkinfos
//                     .entry(info.crate_name)
//                     .or_insert_with(|| (HashSet::default(), HashSet::default()));

//                 entry.0.extend(info.used_rufs.into_iter());
//                 entry.1.extend(info.cfg.into_iter());
//             }
//         }

//         return checkinfos;
//     });

//     // let stderr_handle = thread::spawn(move || {
//     //     for line in stderr.lines() {
//     //         // debug!(LOGGER, "Stderr: {line:?}");
//     //         let line = line.expect("Fatal, get stdout line fails");

//     //         // compiling info, we can print it
//     //         if let Some(index) = line.find("Checking") {
//     //             if recheck {
//     //                 info_print("\tRescanning", &line[index + 9..]);
//     //             } else {
//     //                 info_print("\tScanning", &line[index + 9..]);
//     //             }
//     //         } else if line.trim().starts_with("error") {
//     //             error_print(&line);
//     //         }
//     //     }
//     // });

//     // Waiting for process to ends
//     let checkinfos = stdout_handle
//         .join()
//         .expect("Fatal, cannot join stdout thread");
//     // let _ = stderr_handle
//     //     .join()
//     //     .expect("Fatal, cannot join stderr thread");

//     let output = child
//         .wait_with_output()
//         .expect("Fatal, fails to wait cargo process");

//     if !output.status.success() {
//         return Err("cargo process run fails".to_string());
//     }

//     let mut used_rufs = HashMap::default();
//     for (crate_name, (rufs, cfgs)) in checkinfos {
//         config.update_build_cfgs(crate_name.clone(), cfgs);
//         used_rufs.insert(crate_name, UsedRufs::new(rufs.into_iter().collect()));
//     }

//     return Ok(used_rufs);
// }
