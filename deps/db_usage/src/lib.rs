use std::{collections::HashMap, sync::Mutex};

use features::Ruf;
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
        let client = Client::connect("host=localhost user=postgres dbname=crates", NoTls).unwrap();
        Mutex::new(client)
    };
}

#[test]
fn test() {
    print!("{:?}", get_crate_id_with_name("serde"));
}

#[allow(unused)]
pub fn get_crate_id_with_name(crate_name: &str) -> Result<u32, String> {
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

    Ok(crate_id[0].get::<usize, i32>(0) as u32)
}

#[allow(unused)]
pub fn get_rufs_with_crate_id(crate_id: u32) -> Result<HashMap<String, Vec<Ruf>>, String> {
    let rows = CONN
        .lock()
        .unwrap()
        .query(
            "SELECT * FROM version_ruf WHERE crate_id = '$1' ORDER BY id desc",
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
