mod commit_info;

use chrono::{DateTime, FixedOffset};
use git2::Repository;
use std::{env, process};

use commit_info::CommitInfo;

fn main() {
    let Some(arg) = env::args().nth(1) else {
        eprintln!("provide the repo path");
        process::exit(1);
    };

    let commit_list = match handle_repo(&arg) {
        Ok(list) => list,
        Err(e) => {
            eprintln!("{}", e.message());
            process::exit(1);
        }
    };
    for commit in commit_list {
        println!("{}", commit);
    }
}

fn handle_repo(repo_path: &str) -> Result<Vec<CommitInfo>, git2::Error> {
    let repo = Repository::open_bare(repo_path)?;
    let mut rev = repo.revwalk()?;
    rev.push_head()?;

    let mut commit_list = Vec::new();
    for oid in rev {
        let commit = repo.find_commit(oid?)?;

        let summary = commit.summary()?.unwrap_or_default().to_string();
        let author = commit.author().to_string();
        let time = format_time(commit.time());

        commit_list.push(CommitInfo::new(summary, author, time.unwrap()));
    }
    Ok(commit_list)
}

fn format_time(time: git2::Time) -> Option<String> {
    let offset = FixedOffset::east_opt(time.offset_minutes() * 60)?;
    let date = DateTime::from_timestamp(time.seconds(), 0)?;
    Some(
        date.with_timezone(&offset)
            .format("%Y-%m-%d %H:%M")
            .to_string(),
    )
}
