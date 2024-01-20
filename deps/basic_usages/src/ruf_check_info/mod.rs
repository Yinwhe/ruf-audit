use serde::{Deserialize, Serialize};

mod r#impl;

#[derive(Debug)]
pub struct CondRuf {
    pub cond: Option<String>,
    pub feature: String,
}

#[derive(Debug)]
pub struct CondRufs(Vec<CondRuf>);

#[derive(Debug)]
pub enum RufStatus {
    Unknown,
    Active,
    Incomplete,
    Accepted,
    Removed,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UsedRufs(Vec<String>);

#[derive(Debug, Serialize, Deserialize)]
pub struct CheckInfo{
    pub crate_name: String,
    pub used_rufs: UsedRufs,
    pub cfg: Vec<String>,
}