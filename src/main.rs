mod html;
mod repo_info;

use anyhow::{Context, Result, bail};
use html::{
    COMMITS_PER_PAGE, commits_page, diff_stat, gen_blob, gen_branches, gen_commit, gen_commits,
    gen_index, gen_summary, gen_tags, gen_tree,
};
use repo_info::{CommitInfo, RepoInfo};
use std::{
    collections::HashMap,
    env, fs,
    io::ErrorKind,
    iter,
    path::{Path, PathBuf},
    process,
};

const USAGE: &str = "usage: wit [-O owner] [-u repo.git] -o outdir repo.git...";

struct Args {
    owner: Option<String>,
    out_dir: PathBuf,
    update: Option<String>,
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
    let mut update = None;
    let mut repos = Vec::new();
    let mut args = env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-O" => owner = Some(args.next().context(USAGE)?),
            "-o" => out_dir = Some(PathBuf::from(args.next().context(USAGE)?)),
            "-u" => update = Some(args.next().context(USAGE)?),
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
        update,
        repos,
    })
}

fn write_file(path: &Path, contents: &str) -> Result<()> {
    let dir = path.parent().unwrap_or(Path::new(""));
    fs::create_dir_all(dir)
        .and_then(|()| fs::write(path, contents))
        .with_context(|| path.display().to_string())
}

fn last_changes<'a>(
    commits: &'a [CommitInfo],
    changes: &HashMap<String, Vec<String>>,
) -> HashMap<String, &'a CommitInfo> {
    let mut last = HashMap::new();
    for commit in commits {
        for path in changes.get(&commit.id).into_iter().flatten() {
            let mut path = path.as_str();
            loop {
                let newest = last.entry(path.to_string()).or_insert(commit);
                if commit.committer_date > newest.committer_date {
                    *newest = commit;
                }
                match path.rsplit_once('/') {
                    Some((dir, _)) => path = dir,
                    None => break,
                }
            }
        }
    }
    last
}

fn page_clash(repo: &RepoInfo) -> Option<String> {
    for git_ref in repo.branches.iter().chain(&repo.tags) {
        for dir in git_ref.dirs.values() {
            for entry in dir.entries.iter().filter(|e| e.is_dir) {
                let clashes = entry.name == "index.html"
                    || entry.name.strip_suffix(".html").is_some_and(|file| {
                        dir.entries
                            .iter()
                            .any(|e| e.blob.is_some() && e.name == file)
                    });
                if clashes {
                    let path = if dir.path.is_empty() {
                        entry.name.clone()
                    } else {
                        format!("{}/{}", dir.path, entry.name)
                    };
                    return Some(format!("dir {path} in {} clashes with a page", git_ref.dir));
                }
            }
        }
    }
    None
}

fn run() -> Result<()> {
    let args = parse_args()?;
    let update = args
        .update
        .as_ref()
        .map(|path| fs::canonicalize(path).with_context(|| path.clone()))
        .transpose()?;
    let is_update = |path: &str| fs::canonicalize(path).ok() == update;
    if let Some(path) = &args.update
        && !args.repos.iter().any(|repo| is_update(repo))
    {
        bail!("-u {path}: not one of the repos");
    }
    let mut repos: Vec<RepoInfo> = Vec::new();
    for path in &args.repos {
        let repo = RepoInfo::open(path, args.owner.as_deref()).with_context(|| path.clone())?;
        let repo = match repo {
            Ok(repo) => repo,
            Err(reason) => {
                eprintln!("{path}: {reason}, skipped");
                continue;
            }
        };
        if let Some(clash) = page_clash(&repo) {
            eprintln!("{path}: {clash}, skipped");
            continue;
        }
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
    write_file(&args.out_dir.join("index.html"), &gen_index(&repos))?;
    for repo in &repos {
        let repo_dir = args.out_dir.join(&repo.name);
        if update.is_some() && !is_update(&repo.path) && repo_dir.exists() {
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
        let refs = || repo.branches.iter().chain(&repo.tags);
        let head_first = refs()
            .filter(|r| r.dir == repo.head_dir)
            .chain(refs().filter(|r| r.dir != repo.head_dir));
        let mut stats = HashMap::new();
        let mut changes = HashMap::new();
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
                if diff.parents.len() < 2 {
                    let paths: Vec<String> = diff
                        .files
                        .iter()
                        .flat_map(|f| iter::once(&f.path).chain(&f.old_path))
                        .cloned()
                        .collect();
                    changes.insert(commit.id.clone(), paths);
                }
            }
            let last = last_changes(&commits, &changes);
            let commits_dir = repo_dir.join("commits").join(&git_ref.dir);
            for page in 1..=commits.len().div_ceil(COMMITS_PER_PAGE) {
                write_file(
                    &commits_dir.join(commits_page(page)),
                    &gen_commits(repo, git_ref, &commits, &stats, page),
                )?;
            }
            for dir in git_ref.dirs.values() {
                let tree_dir = repo_dir.join("tree").join(&git_ref.dir).join(&dir.path);
                write_file(
                    &tree_dir.join("index.html"),
                    &gen_tree(repo, git_ref, dir, &last),
                )?;
                let blob_dir = repo_dir.join("blob").join(&git_ref.dir).join(&dir.path);
                for entry in &dir.entries {
                    let Some(id) = entry.blob else {
                        continue;
                    };
                    let blob = repo.git.find_blob(id).with_context(|| repo.path.clone())?;
                    write_file(
                        &blob_dir.join(format!("{}.html", entry.name)),
                        &gen_blob(repo, git_ref, dir, entry, blob.content(), &last),
                    )?;
                }
            }
        }
        let head_last = last_changes(&repo.commit_list, &changes);
        write_file(
            &repo_dir.join("index.html"),
            &gen_summary(repo, &stats, &head_last),
        )?;
    }
    Ok(())
}
