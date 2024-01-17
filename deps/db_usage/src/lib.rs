use std::{collections::HashMap, sync::Mutex};

use features::{Ruf, RufStatus};
use lazy_static::lazy_static;
use postgres::{Client, NoTls};

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
pub fn get_rufs_with_crate_id(crate_id: i32) -> Result<HashMap<String, Vec<Ruf>>, String> {
    let rows = CONN
        .lock()
        .unwrap()
        .query(
            "SELECT * FROM version_ruf WHERE crate_id = $1 ORDER BY id desc",
            &[&crate_id],
        )
        .map_err(|e| e.to_string())?;

    let mut dep_rufs = HashMap::new();
    for row in rows {
        let semver: String = row.get(1);
        if let Ok(ruf) = row.try_get::<usize, String>(4) {
            let cond = row
                .try_get::<usize, String>(3)
                .map_or(None, |cond| Some(cond));
            let ruf = Ruf::new(cond, ruf);

            dep_rufs.entry(semver).or_insert_with(Vec::new).push(ruf);
        }
    }

    Ok(dep_rufs)
}

#[allow(unused)]
pub fn get_ruf_status(ruf_name: &str, rustc_ver: u32) -> Result<RufStatus, String> {
    let status = CONN
        .lock()
        .unwrap()
        .query(
            &format!(
                "SELECT v1_{}_0 FROM feature_timeline WHERE name = '{}'",
                rustc_ver, ruf_name
            ),
            &[],
        )
        .map_err(|e| e.to_string())?;

    assert!(status.len() <= 1);
    if status.len() == 0 {
        return Ok(RufStatus::Unknown);
    } else {
        let status: String = status[0].get(0);
        return Ok(RufStatus::from(status.as_str()));
    }
}

// #[test]
// fn test() {
//     let res = get_ruf_status("123", 10);
//     print!("{res:#?}");
// }
