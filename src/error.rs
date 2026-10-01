use std::fmt;

#[derive(Debug)]
pub enum WitError {
    Git(git2::Error),
    InvalidDate,
    Io(std::io::Error),
}

impl fmt::Display for WitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WitError::Git(e) => write!(f, "{}", e.message()),
            WitError::InvalidDate => write!(f, "invalid commit date"),
            WitError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for WitError {}

impl From<git2::Error> for WitError {
    fn from(e: git2::Error) -> Self {
        WitError::Git(e)
    }
}

impl From<std::io::Error> for WitError {
    fn from(e: std::io::Error) -> Self {
        WitError::Io(e)
    }
}
