//! This file describe errors may meet.

use std::fmt::Display;

#[derive(Debug)]
pub enum AuditError {
    /// Not our issues, maybe cargo or other commands fails.
    Unexpected(String),
    /// Our tool fails
    Functionality(String),
}

impl AuditError {
    pub fn is_unexpected(&self) -> bool {
        match self {
            Self::Unexpected(_) => true,
            Self::Functionality(_) => false,
        }
    }

    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Unexpected(_) => -1,
            Self::Functionality(_) => -2,
        }
    }
}

impl Display for AuditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unexpected(msg) => write!(f, "unexpected error: {msg}"),
            Self::Functionality(msg) => write!(f, "functionality error: {msg}"),
        }
    }
}
