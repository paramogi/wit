mod html;
mod repo_info;

use anyhow::{Context, Result, bail};
use html::{
    COMMITS_PER_PAGE, STYLE, commits_page, diff_stat, gen_blob, gen_branches, gen_commit,
    gen_commits, gen_index, gen_summary, gen_tags, gen_tree, has_source_page,
};
use repo_info::RepoInfo;
use std::{
    collections::HashMap,
    env, fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    process,
    time::UNIX_EPOCH,
};

const USAGE: &str = "usage: wit [-O owner] -o outdir repo.git...";

const STATE_FILE: &str = ".wit-state";

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
    let wit = env::current_exe()
        .and_then(fs::metadata)
        .and_then(|meta| meta.modified())
        .context("wit's own binary")?;
    let wit = wit.duration_since(UNIX_EPOCH)?.as_nanos();
    write_file(&args.out_dir.join("style.css"), STYLE)?;
    write_file(&args.out_dir.join("index.html"), &gen_index(&repos))?;
    for repo in &repos {
        let repo_dir = args.out_dir.join(&repo.name);
        let mut state = format!(
            "{wit}\n{}\n{}\n{}\n{}\n",
            repo.name, repo.description, repo.owner, repo.head_dir
        );
        for git_ref in repo.branches.iter().chain(&repo.tags) {
            state.push_str(&format!(
                "{} {} {}\n",
                git_ref.id, git_ref.time, git_ref.dir
            ));
        }
        let state_file = repo_dir.join(STATE_FILE);
        if fs::read_to_string(&state_file).is_ok_and(|old| old == state) {
            continue;
        }
        match fs::remove_dir_all(&repo_dir) {
            Err(e) if e.kind() != ErrorKind::NotFound => {
                return Err(e).with_context(|| repo_dir.display().to_string());
            }
            _ => {}
        }
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
        let mut stats = HashMap::new();
        for git_ref in head_first {
            let commits = repo
                .read_commits(&git_ref.id)
                .with_context(|| repo.path.clone())?;
            for commit in &commits {
                if stats.contains_key(&commit.id) {
                    continue;
                }
                let diff = repo
                    .read_diff(&commit.id)
                    .with_context(|| repo.path.clone())?;
                write_file(
                    &repo_dir.join("commits").join(format!("{}.html", commit.id)),
                    &gen_commit(repo, git_ref, commit, &diff),
                )?;
                stats.insert(commit.id.clone(), diff_stat(&diff));
            }
            let commits_dir = repo_dir.join("commits").join(&git_ref.dir);
            for page in 1..=commits.len().div_ceil(COMMITS_PER_PAGE) {
                write_file(
                    &commits_dir.join(commits_page(page)),
                    &gen_commits(repo, git_ref, &commits, &stats, page),
                )?;
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
        write_file(&repo_dir.join("index.html"), &gen_summary(repo, &stats))?;
        write_file(&state_file, &state)?;
    }
    Ok(())
}
