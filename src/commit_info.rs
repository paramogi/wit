use std::fmt;

pub struct CommitInfo {
    pub id: String,
    pub summary: String,
    pub message: String,
    pub author_name: String,
    pub author_email: String,
    pub time: String,
}

impl CommitInfo {
    pub fn new(
        id: String, summary: String, message: String, author_name: String, author_email: String, time: String
    ) -> Self {
        CommitInfo {
            id,
            summary,
            message,
            author_name,
            author_email,
            time,
        }
    }
}

impl fmt::Display for CommitInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "
---\n\
id: {}\n\
summary: {}\n\
message: {}\
author: {} <{}>\n\
time: {}\n\
---",
            self.id, self.summary, self.message, self.author_name, self.author_email, self.time)
    }
}
