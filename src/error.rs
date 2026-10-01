use std::fmt;

#[derive(Debug)]
pub enum WitError {
    Git(git2::Error),
    InvalidDate,
}

impl fmt::Display for WitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WitError::Git(e) => write!(f, "{}", e.message()),
            WitError::InvalidDate => write!(f, "invalid commit date"),
        }
    }
}

impl std::error::Error for WitError {}

impl From<git2::Error> for WitError {
    fn from(e: git2::Error) -> Self {
        WitError::Git(e)
    }
}
