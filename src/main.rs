mod html;
mod repo_info;

use anyhow::{Context, Result, bail};
use html::{
    LOG_COUNT, STYLE, gen_blob, gen_branches, gen_commit, gen_index, gen_log, gen_summary,
    gen_tags, gen_tree, has_source_page, log_page,
};
use repo_info::RepoInfo;
use std::{
    collections::HashMap,
    env, fs,
    path::{Path, PathBuf},
    process,
};

const USAGE: &str = "usage: wit [-O owner] -o outdir repo.git...";

struct Args {
    owner: Option<String>,
    out_dir: PathBuf,
    repos: Vec<String>,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("{e:#}");
        process::exit(1);
    }
}

fn parse_args() -> Result<Args> {
    let mut owner = None;
    let mut out_dir = None;
    let mut repos = Vec::new();
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-O" => owner = Some(args.next().context(USAGE)?),
            "-o" => out_dir = Some(PathBuf::from(args.next().context(USAGE)?)),
            s if s.starts_with('-') => bail!(USAGE),
            _ => repos.push(arg),
        }
    }
    let out_dir = out_dir.context(USAGE)?;
    if repos.is_empty() {
        bail!(USAGE);
    }
    Ok(Args {
        owner,
        out_dir,
        repos,
    })
}

fn write_file(path: &Path, contents: &str) -> Result<()> {
    let dir = path.parent().unwrap_or(Path::new(""));
    fs::create_dir_all(dir)
        .and_then(|()| fs::write(path, contents))
        .with_context(|| path.display().to_string())
}

fn run() -> Result<()> {
    let args = parse_args()?;
    let mut repos: Vec<RepoInfo> = Vec::new();
    for path in &args.repos {
        let repo = RepoInfo::open(path, args.owner.as_deref()).with_context(|| path.clone())?;
        let Some(repo) = repo else {
            eprintln!("{path}: HEAD's branch has no commits, skipped");
            continue;
        };
        if let Some(first) = repos.iter().find(|r| r.name == repo.name) {
            bail!(
                "{} and {} are both named {}",
                first.path,
                repo.path,
                repo.name
            );
        }
        repos.push(repo);
    }
    write_file(&args.out_dir.join("style.css"), STYLE)?;
    write_file(&args.out_dir.join("index.html"), &gen_index(&repos))?;
    for repo in &repos {
        let repo_dir = args.out_dir.join(&repo.name);
        write_file(&repo_dir.join("index.html"), &gen_summary(repo))?;
        write_file(&repo_dir.join("branches.html"), &gen_branches(repo))?;
        if !repo.tags.is_empty() {
            write_file(&repo_dir.join("tags.html"), &gen_tags(repo))?;
        }
        let head_first = repo
            .branches
            .iter()
            .filter(|b| b.dir == repo.head_dir)
            .chain(repo.branches.iter().filter(|b| b.dir != repo.head_dir))
            .chain(&repo.tags);
        let mut commit_refs = HashMap::new();
        for git_ref in head_first {
            let commits = repo
                .read_log(&git_ref.id)
                .with_context(|| repo.path.clone())?;
            let log_dir = repo_dir.join("logs").join(&git_ref.dir);
            for page in 1..=commits.len().div_ceil(LOG_COUNT) {
                write_file(
                    &log_dir.join(log_page(page)),
                    &gen_log(repo, git_ref, &commits, page),
                )?;
            }
            for commit in commits {
                commit_refs
                    .entry(commit.id.clone())
                    .or_insert((commit, git_ref));
            }
            for folder in &git_ref.folders {
                let tree_dir = repo_dir.join("tree").join(&git_ref.dir).join(&folder.path);
                write_file(
                    &tree_dir.join("index.html"),
                    &gen_tree(repo, git_ref, folder),
                )?;
                let blob_dir = repo_dir.join("blob").join(&git_ref.dir).join(&folder.path);
                let source_dir = repo_dir
                    .join("source")
                    .join(&git_ref.dir)
                    .join(&folder.path);
                for entry in &folder.entries {
                    let Some(id) = entry.blob else {
                        continue;
                    };
                    let blob = repo.git.find_blob(id).with_context(|| repo.path.clone())?;
                    let page_name = format!("{}.html", entry.name);
                    write_file(
                        &blob_dir.join(&page_name),
                        &gen_blob(repo, git_ref, folder, entry, blob.content(), false),
                    )?;
                    if has_source_page(entry) {
                        write_file(
                            &source_dir.join(&page_name),
                            &gen_blob(repo, git_ref, folder, entry, blob.content(), true),
                        )?;
                    }
                }
            }
        }
        for (commit, git_ref) in commit_refs.values() {
            let diff = repo
                .read_diff(&commit.id)
                .with_context(|| repo.path.clone())?;
            write_file(
                &repo_dir.join("commits").join(format!("{}.html", commit.id)),
                &gen_commit(repo, git_ref, commit, &diff),
            )?;
        }
    }
    Ok(())
}
