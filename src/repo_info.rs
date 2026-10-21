use crate::error::WitError;

use chrono::{DateTime, FixedOffset};
use git2::Repository;
use std::{fmt, fs::read_to_string, io::ErrorKind::NotFound, path::Path};

pub struct CommitInfo {
    pub id: String,
    pub summary: String,
    pub message: String,
    pub author_name: String,
    pub author_email: String,
    pub time: String,
}

pub struct RepoInfo {
    pub name: String,
    pub description: String,
    pub owner: String,
    pub commit_list: Vec<CommitInfo>,
}

impl RepoInfo {
    pub fn new(
        name: String,
        description: String,
        owner: String,
        commit_list: Vec<CommitInfo>,
    ) -> Self {
        RepoInfo {
            name,
            description,
            owner,
            commit_list,
        }
    }

    pub fn open(path: &str) -> Result<Self, WitError> {
        let repo = Repository::open_bare(path)?;
        let repo_path = repo.path();
        let name = repo_path
            .file_name()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default();
        let name = name.strip_suffix(".git").unwrap_or(name).to_string();
        let description = Self::read_optional_file(repo_path, "description")?;
        let owner = Self::read_optional_file(repo_path, "owner")?;
        let commit_list = Self::handle_commits(&repo)?;
        Ok(Self::new(name, description, owner, commit_list))
    }

    fn handle_commits(repo: &Repository) -> Result<Vec<CommitInfo>, WitError> {
        let mut rev = repo.revwalk()?;
        rev.push_head()?;

        let mut commit_list = Vec::new();
        for oid in rev {
            let commit = repo.find_commit(oid?)?;

            let id = commit.id().to_string();
            let summary = commit.summary()?.unwrap_or_default().to_string();
            let message = commit.message().unwrap_or_default().to_string();
            let sig = commit.author();
            let author_name = sig.name().unwrap_or_default().to_string();
            let author_email = sig.email().unwrap_or_default().to_string();
            let time = Self::format_time(commit.time()).ok_or(WitError::InvalidDate)?;

            commit_list.push(CommitInfo::new(
                id,
                summary,
                message,
                author_name,
                author_email,
                time,
            ));
        }
        Ok(commit_list)
    }

    fn format_time(time: git2::Time) -> Option<String> {
        let offset = FixedOffset::east_opt(time.offset_minutes() * 60)?;
        let date = DateTime::from_timestamp(time.seconds(), 0)?;
        Some(
            date.with_timezone(&offset)
                .format("%d-%m-%Y %H:%M")
                .to_string(),
        )
    }

    fn read_optional_file(repo_path: &Path, file_name: &str) -> Result<String, WitError> {
        let real_path = repo_path.join(file_name);
        match read_to_string(real_path) {
            Ok(s) => Ok(s.trim().to_string()),
            Err(e) if e.kind() == NotFound => Ok(String::new()),
            Err(e) => Err(e.into()),
        }
    }
}

impl fmt::Display for RepoInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "\
name: {}\n\
description: {}\n\
owner: {}\n\
last commit time: {}\n\
",
            self.name, self.description, self.owner, self.commit_list[0].time
        )?;
        for commit in &self.commit_list {
            write!(f, "{commit}")?;
        }
        Ok(())
    }
}

impl CommitInfo {
    pub fn new(
        id: String,
        summary: String,
        message: String,
        author_name: String,
        author_email: String,
        time: String,
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
        write!(
            f,
            "\
id: {}\n\
summary: {}\n\
message: {}\
author: {} <{}>\n\
time: {}\
",
            self.id, self.summary, self.message, self.author_name, self.author_email, self.time
        )
    }
}
