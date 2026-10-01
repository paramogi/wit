use std::fmt;

pub struct CommitInfo {
    pub summary: String,
    pub author: String,
    pub time: String,
}

impl CommitInfo {
    pub fn new(summary: String, author: String, time: String) -> Self {
        CommitInfo {
            summary,
            author,
            time,
        }
    }
}

impl fmt::Display for CommitInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {}, {})", self.summary, self.author, self.time)
    }
}
