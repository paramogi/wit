use std::fmt;

pub enum WitError {
    Usage,
    NoName(String),
    DuplicateName {
        name: String,
        first: String,
        second: String,
    },
    InvalidDate,
    Git(git2::Error),
    Io(std::io::Error),
}

impl fmt::Display for WitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WitError::Usage => write!(f, "usage: wit [-O owner] -o outdir repo.git..."),
            WitError::NoName(path) => write!(f, "{path}: could not work out the repo's name"),
            WitError::DuplicateName {
                name,
                first,
                second,
            } => write!(f, "{first} and {second} are both named {name}"),
            WitError::InvalidDate => write!(f, "invalid commit date"),
            WitError::Git(e) => write!(f, "{}", e.message()),
            WitError::Io(e) => write!(f, "{e}"),
        }
    }
}

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
