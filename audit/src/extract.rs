use std::env::current_exe;
use std::io::{BufRead, BufReader};
use std::process::Stdio;

use basic_usages::external::fxhash::{FxHashMap as HashMap, FxHashSet as HashSet};
use basic_usages::ruf_check_info::{CheckInfo, UsedRufs};

use crate::build_config::BuildConfig;
use crate::error::AuditError;
use crate::{cargo, warn_print, RE_CHECKINFO};

// pub fn extract(
//     config: &mut BuildConfig,
//     // recheck: bool,
// ) -> Result<HashMap<String, UsedRufs>, AuditError> {
//     let mut cargo = cargo();
//     cargo.arg("rustc");
//     if let Some(cargo_args) = config.get_cargo_args() {
//         cargo.args(cargo_args);
//     }

//     let path = current_exe()
//         .map_err(|_| AuditError::Unexpected(format!("cannot get current exe path")))?;
//     cargo.env("RUSTC_WRAPPER", &path);

//     let cargo = cargo
//         .output()
//         .map_err(|e| AuditError::Unexpected(format!("cannot spawn cargo process: {e}")))?;

//     if !cargo.status.success() {
//         let stderr = String::from_utf8_lossy(&cargo.stderr);
//         let err = stderr
//             .lines()
//             .into_iter()
//             .skip_while(|line| !line.trim_start().starts_with("error"))
//             .collect::<Vec<&str>>()
//             .join("\n");

//         return Err(AuditError::Unexpected(format!(
//             "cargo process run fails: {err:?}",
//         )));
//     }

//     let stdout = String::from_utf8_lossy(&cargo.stdout);
//     let mut checkinfos = HashMap::default();

//     // resolves used rufs from stdout
//     for cap in RE_CHECKINFO.captures_iter(&stdout) {
//         let info = CheckInfo::from(cap.get(1).expect("Fatal, resolve buildinfo fails").as_str());
//         let entry = checkinfos
//             .entry(info.crate_name)
//             .or_insert_with(|| (HashSet::default(), HashSet::default()));

//         entry.0.extend(info.used_rufs.into_iter());
//         entry.1.extend(
//             info.cfg
//                 .into_iter()
//                 .map(|cfg| cfg.escape_default().to_string()),
//         );
//     }

//     let mut used_rufs = HashMap::default();
//     for (crate_name, (rufs, cfgs)) in checkinfos {
//         config.update_build_cfgs(crate_name.clone(), cfgs);
//         used_rufs.insert(crate_name, UsedRufs::new(rufs.into_iter().collect()));
//     }

//     Ok(used_rufs)
// }

/// rufs usage extract, based on `cargo check`
pub fn extract(
    config: &mut BuildConfig,
    // recheck: bool,
) -> Result<HashMap<String, UsedRufs>, AuditError> {
    let mut cmd = cargo();
    cmd.args(["rustc", "-Z", "unstable-options", "--keep-going"]);
    if let Some(cargo_args) = config.get_cargo_args() {
        cmd.args(cargo_args);
    }

    let path = current_exe()
        .map_err(|_| AuditError::Unexpected(format!("cannot get current exe path")))?;
    cmd.env("RUSTC_WRAPPER", &path);

    let mut child = cmd
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| AuditError::Unexpected(format!("cannot spawn cargo process")))?;

    // verbose stderr infos
    if config.is_verbose() {
        let stderr = BufReader::new(child.stderr.take().unwrap());

        for line in stderr.lines() {
            let line = line.expect("Fatal, get stdout line fails");
            print!("{line}\r")
        }
    }

    let output = child
        .wait_with_output()
        .expect("Fatal, fails to wait cargo process");

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // let err = stderr
        //     .lines()
        //     .into_iter()
        //     .skip_while(|line| !line.trim_start().starts_with("error"))
        //     .collect::<Vec<&str>>()
        //     .join("\n");

        // return Err(AuditError::Unexpected(format!(
        //     "cargo process run fails: {err:?}",
        // )));

        let err = stderr
            .lines()
            .into_iter()
            .find(|line| line.trim_start().starts_with("error"))
            .unwrap_or("unknown error");

        // We may not stop here, keeps on going and just print errors, 
        // since we only cares ruf usage, rather than syntax error or things like that.
        warn_print("Buiding Issues", &format!("extraction incomplete, mostly due to syntax fatal errors (you can check details with cargo), but we will keep on going: {err}"));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
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

    return Ok(used_rufs);
}
