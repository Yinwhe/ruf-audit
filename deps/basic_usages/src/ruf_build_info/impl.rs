use std::fmt::Display;

use super::*;

impl CondRuf {
    pub fn new(cond: Option<String>, feature: String) -> Self {
        CondRuf { cond, feature }
    }
}

impl UsedRufs {
    pub fn new(rufs: Vec<String>) -> Self {
        UsedRufs(rufs)
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

impl Display for BuildInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "\nBDelimiter::{{{}}}::BDelimiter\n",
            serde_json::to_string(&self).expect("Fatal, serialize fails")
        )
    }
}

impl Into<String> for BuildInfo {
    fn into(self) -> String {
        format!("{self}")
    }
}

impl From<&str> for BuildInfo {
    fn from(value: &str) -> Self {
        serde_json::from_str(&value).expect("Fatal, deserialize fails")
    }
}

impl Display for UsedRufs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "\nFDelimiter::{{{}}}::FDelimiter\n",
            serde_json::to_string(&self).expect("Fatal, serialize fails")
        )
    }
}

impl Into<String> for UsedRufs {
    fn into(self) -> String {
        format!("{self}")
    }
}

impl From<&str> for UsedRufs {
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
