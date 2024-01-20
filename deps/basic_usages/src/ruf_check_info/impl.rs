use std::fmt::Display;

use super::*;

impl UsedRufs {
    pub fn new(rufs: Vec<String>) -> Self {
        UsedRufs(rufs)
    }

    pub fn empty() -> Self {
        UsedRufs(Vec::new())
    }

    pub fn push(&mut self, ruf: String) {
        self.0.push(ruf);
    }

    pub fn extend(&mut self, rufs: impl IntoIterator<Item = String>) {
        self.0.extend(rufs.into_iter());
    }

    pub fn iter(&self) -> std::slice::Iter<'_, String> {
        self.0.iter()
    }
}

impl CondRufs {
    pub fn new(rufs: Vec<CondRuf>) -> Self {
        CondRufs(rufs)
    }

    pub fn empty() -> Self {
        CondRufs(Vec::new())
    }

    pub fn push(&mut self, ruf: CondRuf) {
        self.0.push(ruf);
    }

    pub fn extend(&mut self, rufs: impl IntoIterator<Item = CondRuf>) {
        self.0.extend(rufs.into_iter());
    }

    pub fn iter(&self) -> std::slice::Iter<'_, CondRuf> {
        self.0.iter()
    }
}


impl IntoIterator for UsedRufs {
    type Item = String;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl IntoIterator for CondRufs {
    type Item = CondRuf;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
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

impl Display for CheckInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "\nCDelimiter::{{{}}}::CDelimiter\n",
            serde_json::to_string(&self).expect("Fatal, serialize fails")
        )
    }
}

impl Into<String> for CheckInfo {
    fn into(self) -> String {
        format!("{self}")
    }
}

impl From<&str> for CheckInfo {
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
