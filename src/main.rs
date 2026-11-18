mod error;
mod html;
mod repo_info;

use html::{STYLE, gen_branches, gen_index, gen_summary, gen_tags, gen_tree};
use repo_info::RepoInfo;
use std::{env, fs, path::PathBuf, process};

use crate::error::WitError;

struct Args {
    owner: Option<String>,
    out_dir: PathBuf,
    repos: Vec<String>,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        process::exit(1);
    }
}

fn parse_args() -> Result<Args, WitError> {
    let mut owner = None;
    let mut out_dir = None;
    let mut repos = Vec::new();
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-O" => owner = Some(args.next().ok_or(WitError::Usage)?),
            "-o" => out_dir = Some(PathBuf::from(args.next().ok_or(WitError::Usage)?)),
            s if s.starts_with('-') => return Err(WitError::Usage),
            _ => repos.push(arg),
        }
    }
    let out_dir = out_dir.ok_or(WitError::Usage)?;
    if repos.is_empty() {
        return Err(WitError::Usage);
    }
    Ok(Args {
        owner,
        out_dir,
        repos,
    })
}

fn run() -> Result<(), WitError> {
    let args = parse_args()?;
    let mut repos: Vec<RepoInfo> = Vec::new();
    for path in &args.repos {
        let Some(repo) = RepoInfo::open(path, args.owner.as_deref())? else {
            eprintln!("{path}: HEAD's branch has no commits, skipped");
            continue;
        };
        if let Some(first) = repos.iter().find(|r| r.name == repo.name) {
            return Err(WitError::DuplicateName {
                name: repo.name,
                first: first.path.clone(),
                second: repo.path,
            });
        }
        repos.push(repo);
    }
    fs::create_dir_all(&args.out_dir)?;
    fs::write(args.out_dir.join("style.css"), STYLE)?;
    fs::write(args.out_dir.join("index.html"), gen_index(&repos))?;
    for repo in &repos {
        let repo_dir = args.out_dir.join(&repo.name);
        fs::create_dir_all(&repo_dir)?;
        fs::write(repo_dir.join("index.html"), gen_summary(repo))?;
        fs::write(repo_dir.join("branches.html"), gen_branches(repo))?;
        if !repo.tags.is_empty() {
            fs::write(repo_dir.join("tags.html"), gen_tags(repo))?;
        }
        for git_ref in repo.branches.iter().chain(&repo.tags) {
            let tree_dir = repo_dir.join("tree").join(&git_ref.dir);
            fs::create_dir_all(&tree_dir)?;
            fs::write(tree_dir.join("index.html"), gen_tree(repo, git_ref))?;
        }
    }
    Ok(())
}
