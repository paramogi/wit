use git2::Repository;
use std::env;
use std::process;

fn main() {
    let Some(arg) = env::args().nth(1) else {
        eprintln!("provide the repo path");
        process::exit(1);
    };

    let commit = match handle_repo(&arg) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("{}", e.message());
            process::exit(1);
        }
    };
    println!("{commit}");
}

fn handle_repo(repo_path: &str) -> Result<String, git2::Error> {
    let repo = Repository::open_bare(repo_path)?;
    let mut rev = repo.revwalk()?;
    rev.push_head()?;
    let mut commit_string = String::new();
    for r in rev {
        let oid = r?;
        let commit = repo.find_commit(oid)?;
        // println!("commit info");
        // println!("-----------");
        // println!("msg=\"{}\"", commit.message()?.trim());
        // println!("summary=\"{}\"", commit.summary()?.unwrap_or_default());
        // println!("author=\"{}\"", commit.author());
        commit_string.push_str(commit.summary()?.unwrap_or_default());
        commit_string.push_str(&commit.author().to_string());

        // TODO : Add date
        let commit_time = commit.time();
        let commit_timezone = commit_time.offset_minutes();
        let commit_time_seconds = (commit_time.seconds() + commit_timezone as i64 * 60) % 86400;
        let hour = commit_time_seconds / 3600;
        let minutes = (commit_time_seconds - hour * 3600) / 60;

        commit_string.push_str(&format!(" {:02}:{:02}\n", hour, minutes).to_string());
        // println!("time={:02}:{:02}", hour, minutes);
        // println!("-----------\n");
    }

    Ok(commit_string)
}
