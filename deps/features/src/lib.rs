use serde::{Deserialize, Serialize};

mod r#impl;

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
