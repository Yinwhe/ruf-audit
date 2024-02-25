use fxhash::FxHashMap;
use lazy_static::lazy_static;

use crate::ruf_lifetime::RUSTC_VER_NUM;

lazy_static! {
    static ref RUSTC_VERSION: FxHashMap<u32, &'static str> = get_nightly_versions_raw();
}

pub fn get_nightly_version(rustc_version: u32) -> &'static str {
    assert!((rustc_version as usize) < RUSTC_VER_NUM);

    RUSTC_VERSION[&rustc_version]
}

fn get_nightly_versions_raw() -> FxHashMap<u32, &'static str> {
    // Notice: this rust version and dates are based on our test machine:
    // Linux ubuntu-7070 6.5.0-18-generic #18~22.04.1-Ubuntu SMP PREEMPT_DYNAMIC Wed Feb  7 11:40:03 UTC 2 x86_64 x86_64 x86_64 GNU/Linux
    let mut versions = FxHashMap::default();
    versions.insert(1, "nightly-2015-05-16");
    versions.insert(2, "nightly-2015-05-28");
    versions.insert(3, "nightly-2015-06-26");
    versions.insert(4, "nightly-2015-08-08");
    versions.insert(5, "nightly-2015-09-18");
    versions.insert(6, "nightly-2015-10-30");
    versions.insert(7, "nightly-2015-12-10");
    versions.insert(8, "nightly-2016-01-22");
    versions.insert(9, "nightly-2016-03-04");
    versions.insert(10, "nightly-2016-05-27");
    versions.insert(11, "nightly-2016-06-16");
    versions.insert(12, "nightly-2016-07-08");
    versions.insert(13, "nightly-2016-08-19");
    versions.insert(14, "nightly-2016-09-30");
    versions.insert(15, "nightly-2016-12-02");
    versions.insert(16, "nightly-2017-02-03");
    versions.insert(17, "nightly-2017-03-02");
    versions.insert(18, "nightly-2017-04-28");
    versions.insert(19, "nightly-2017-06-09");
    versions.insert(20, "nightly-2017-07-20");
    versions.insert(21, "nightly-2017-09-01");
    versions.insert(22, "nightly-2017-10-12");
    versions.insert(23, "nightly-2017-11-23");
    versions.insert(24, "nightly-2018-01-04");
    versions.insert(25, "nightly-2018-02-14");
    versions.insert(26, "nightly-2018-03-29");
    versions.insert(27, "nightly-2018-05-11");
    versions.insert(28, "nightly-2018-06-28");
    versions.insert(29, "nightly-2018-08-02");
    versions.insert(30, "nightly-2018-09-13");
    versions.insert(31, "nightly-2018-10-22");
    versions.insert(32, "nightly-2018-12-08");
    versions.insert(33, "nightly-2019-01-18");
    versions.insert(34, "nightly-2019-03-01");
    versions.insert(35, "nightly-2019-04-12");
    versions.insert(36, "nightly-2019-05-24");
    versions.insert(37, "nightly-2019-07-04");
    versions.insert(38, "nightly-2019-08-15");
    versions.insert(39, "nightly-2019-09-20");
    versions.insert(40, "nightly-2019-11-07");
    versions.insert(41, "nightly-2019-12-19");
    versions.insert(42, "nightly-2020-01-31");
    versions.insert(43, "nightly-2020-03-12");
    versions.insert(44, "nightly-2020-04-23");
    versions.insert(45, "nightly-2020-06-05");
    versions.insert(46, "nightly-2020-07-16");
    versions.insert(47, "nightly-2020-08-27");
    versions.insert(48, "nightly-2020-09-11");
    versions.insert(49, "nightly-2020-10-08");
    versions.insert(50, "nightly-2020-11-19");
    versions.insert(51, "nightly-2020-12-31");
    versions.insert(52, "nightly-2021-02-11");
    versions.insert(53, "nightly-2021-03-25");
    versions.insert(54, "nightly-2021-05-06");
    versions.insert(55, "nightly-2021-06-17");
    versions.insert(56, "nightly-2021-07-29");
    versions.insert(57, "nightly-2021-09-09");
    versions.insert(58, "nightly-2021-10-21");
    versions.insert(59, "nightly-2021-12-02");
    versions.insert(60, "nightly-2022-01-14");
    versions.insert(61, "nightly-2022-02-25");
    versions.insert(62, "nightly-2022-04-07");
    versions.insert(63, "nightly-2022-05-19");
    versions.insert(64, "nightly-2022-06-30");
    versions.insert(65, "nightly-2022-08-11");
    versions.insert(66, "nightly-2022-09-22");
    versions.insert(67, "nightly-2022-11-03");
    versions.insert(68, "nightly-2022-12-16");
    versions.insert(69, "nightly-2023-01-26");
    versions.insert(70, "nightly-2023-03-09");

    versions
}

#[allow(unused)]
fn install_toolchains() {
    for i in (1..=63).rev() {
        let name = format!("nightly-{}", get_nightly_version(i));

        let mut cmd = std::process::Command::new("rustup");
        cmd.args(["toolchain", "install", &name, "--profile", "minimal"]);

        let output = match cmd.output() {
            Ok(output) => output,
            Err(e) => {
                println!("[ERROR] install {} failed: {}", &name, e);
                continue;
            }
        };

        if !output.status.success() {
            println!(
                "[ERROR] install {} failed: {}",
                &name,
                String::from_utf8_lossy(&output.stderr)
            );
            continue;
        }

        let output = String::from_utf8_lossy(&output.stdout);
        for line in output.lines() {
            if line.contains("installed") || line.contains("unchanged") {
                println!("[DONE] {}", line);
                break;
            }
        }
    }
}
