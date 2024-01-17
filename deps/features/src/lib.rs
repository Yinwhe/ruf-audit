use std::fmt::Display;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Ruf {
    pub cond: Option<String>,
    pub feature: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Rufs {
    pub crate_name: String,
    pub rufs: Vec<Ruf>,
}

#[derive(Debug)]
pub enum RufStatus {
    Unknown,
    Active,
    Incomplete,
    Accepted,
    Removed,
}

impl Ruf {
    pub fn new(cond: Option<String>, feature: String) -> Self {
        Ruf { cond, feature }
    }
}

impl Rufs {
    pub fn new(crate_name: String, rufs: Vec<Ruf>) -> Self {
        Rufs { crate_name, rufs }
    }

    pub fn from_vec(crate_name: String, no_cond_rufs: Vec<String>) -> Self {
        let mut rufs_vec = Vec::new();
        for ruf in no_cond_rufs {
            rufs_vec.push(Ruf::new(None, ruf));
        }
        Rufs {
            crate_name,
            rufs: rufs_vec,
        }
    }
}

impl Display for Rufs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "\nFDelimiter::{{{}}}::FDelimiter\n",
            serde_json::to_string(&self).expect("Fatal, serialize fails")
        )
    }
}

impl Into<String> for Rufs {
    fn into(self) -> String {
        format!("{self}")
    }
}

impl From<&str> for Rufs {
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