mod commit_info;

use git2::Repository;
use std::env;
use std::process;
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

        // TODO : Add date
        let t = commit.time();
        let secs = (t.seconds() + t.offset_minutes() as i64 * 60).rem_euclid(86400);
        let time = format!("{:02}:{:02}", secs / 3600, (secs % 3600) / 60).to_string();

        commit_list.push(CommitInfo::new(summary, author, time));
    }
    Ok(commit_list)
}
