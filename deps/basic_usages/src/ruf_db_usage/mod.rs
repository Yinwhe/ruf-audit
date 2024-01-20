use std::sync::Mutex;

use fxhash::FxHashMap as HashMap;
use lazy_static::lazy_static;
use postgres::{Client, NoTls};
use semver::Version;

use crate::ruf_check_info::CondRufs;

use super::ruf_check_info::CondRuf;

/*
CREATE VIEW version_ruf AS
SELECT versions.id, versions.num, versions.crate_id, version_feature_ori.conds, version_feature_ori.feature
FROM versions
JOIN version_feature_ori
ON versions.id = version_feature_ori.id
*/

lazy_static! {
    static ref CONN: Mutex<Client> = {
        let client = Client::connect(
            "host=localhost dbname=crates user=postgres password=postgres",
            NoTls,
        )
        .unwrap();
        Mutex::new(client)
    };
}

#[allow(unused)]
pub fn get_crate_id_with_name(crate_name: &str) -> Result<i32, String> {
    let crate_id = CONN
        .lock()
        .unwrap()
        .query(
            "SELECT id FROM crates WHERE name = $1 LIMIT 1",
            &[&crate_name],
        )
        .map_err(|e| e.to_string())?;

    if crate_id.len() == 0 {
        return Err(format!("No crate with name {} found", crate_name));
    }

    Ok(crate_id[0].get::<usize, i32>(0))
}

#[allow(unused)]
pub fn get_rufs_with_crate_id(crate_id: i32) -> Result<HashMap<Version, CondRufs>, String> {
    let rows = CONN
        .lock()
        .unwrap()
        .query(
            "SELECT * FROM version_ruf WHERE crate_id = $1 ORDER BY id desc",
            &[&crate_id],
        )
        .map_err(|e| e.to_string())?;

    let mut dep_rufs = HashMap::default();
    for row in rows {
        let ver: String = row.get(1);
        if let Ok(ver) = Version::parse(&ver) {
            if let Ok(ruf) = row.try_get::<usize, String>(4) {
                let cond = row.try_get::<usize, String>(3).map_err(|e| e.to_string())?;
                let cond = if cond.is_empty() { None } else { Some(cond) };

                let ruf = CondRuf {
                    cond: cond,
                    feature: ruf,
                };

                dep_rufs
                    .entry(ver)
                    .or_insert_with(CondRufs::empty)
                    .push(ruf);
            }
        }
    }

    Ok(dep_rufs)
}

#[allow(unused)]
pub fn get_rufs_with_crate_name(crate_name: &str) -> Result<HashMap<Version, CondRufs>, String> {
    let crate_id = get_crate_id_with_name(crate_name)?;
    get_rufs_with_crate_id(crate_id)
}

#[test]
fn test() {
    let res = get_rufs_with_crate_name("semver");
    println!("{res:#?}")
}
