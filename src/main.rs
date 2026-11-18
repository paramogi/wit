mod error;
mod html;
mod repo_info;

use html::gen_page;
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
    for path in &args.repos {
        let repo = RepoInfo::open(path, args.owner.as_deref())?;
        let page = gen_page(&repo)?;
        let repo_dir = args.out_dir.join(&repo.name);
        fs::create_dir_all(&repo_dir)?;
        fs::write(repo_dir.join("index.html"), page)?;
    }
    Ok(())
}
