use std::fmt::Display;

use super::*;

impl Ruf {
    pub fn new(cond: Option<String>, feature: String) -> Self {
        Ruf { cond, feature }
    }
}

impl CrateRufs {
    pub fn new(crate_name: String, rufs: Vec<Ruf>) -> Self {
        CrateRufs { crate_name, rufs }
    }

    pub fn from_vec(crate_name: String, no_cond_rufs: Vec<String>) -> Self {
        let mut rufs_vec = Vec::new();
        for ruf in no_cond_rufs {
            rufs_vec.push(Ruf::new(None, ruf));
        }
        CrateRufs {
            crate_name,
            rufs: rufs_vec,
        }
    }
}

impl RufStatus {
    pub fn is_usable(&self) -> bool {
        match self {
            Self::Removed | Self::Unknown => false,
            _ => true,
        }
    }
}


impl Display for CrateRufs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "\nFDelimiter::{{{}}}::FDelimiter\n",
            serde_json::to_string(&self).expect("Fatal, serialize fails")
        )
    }
}

impl Into<String> for CrateRufs {
    fn into(self) -> String {
        format!("{self}")
    }
}

impl From<&str> for CrateRufs {
    fn from(value: &str) -> Self {
        serde_json::from_str(&value).expect("Fatal, deserialize fails")
    }
}

impl From<&str> for RufStatus {
    fn from(value: &str) -> Self {
        match value {
            "active" => RufStatus::Active,
            "incomplete" => RufStatus::Incomplete,
            "accepted" => RufStatus::Accepted,
            "removed" => RufStatus::Removed,
            "" => RufStatus::Unknown,
            _ => unreachable!("Fatal, unknown ruf status: {}", value),
        }
    }
}

impl From<u32> for RufStatus {
    fn from(value: u32) -> Self {
        match value {
            0 => RufStatus::Unknown,
            1 => RufStatus::Active,
            2 => RufStatus::Incomplete,
            3 => RufStatus::Accepted,
            4 => RufStatus::Removed,
            _ => unreachable!("Fatal, unknown ruf status: {}", value),
        }
    }
}
