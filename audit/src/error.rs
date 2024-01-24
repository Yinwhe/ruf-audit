use std::fmt::Display;

#[derive(Debug)]
pub enum AuditError {
    Unexpected(String),
    Functionality(String),
}

impl AuditError {
    pub fn is_unexpected(&self) -> bool {
        match self {
            Self::Unexpected(_) => true,
            Self::Functionality(_) => false,
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
