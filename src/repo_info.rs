use anyhow::{Context, Result};

use chrono::{DateTime, FixedOffset};
use git2::{
    BranchType, Commit, Delta, DiffFile, DiffFindOptions, DiffOptions, ErrorCode, ObjectType, Oid,
    Patch, Repository, Revwalk, Sort, Tree,
};
use std::{
    cmp::Ordering,
    collections::{BTreeMap, HashMap},
    fs::{self, read_to_string},
    io::ErrorKind::NotFound,
    path::{Path, PathBuf},
};

const DEFAULT_DESCRIPTION: &str = "[no description]";

const NO_MESSAGE: &str = "(no message)";

pub struct CommitInfo {
    pub id: String,
    pub summary: String,
    pub message: String,
    pub author_name: String,
    pub author_email: String,
    pub author_date: DateTime<FixedOffset>,
    pub committer_name: String,
    pub committer_email: String,
    pub committer_date: DateTime<FixedOffset>,
}

pub struct FileChange {
    pub status: char,
    pub path: String,
    pub old_path: Option<String>,
    pub added: usize,
    pub removed: usize,
    pub binary: Option<(usize, usize)>,
    pub lines: Vec<Line>,
}

pub struct Line {
    pub origin: char,
    pub old: Option<u32>,
    pub new: Option<u32>,
    pub text: String,
}

pub struct CommitDiff {
    pub parents: Vec<String>,
    pub files: Vec<FileChange>,
}

pub struct TreeEntry {
    pub name: String,
    pub is_dir: bool,
    pub blob: Option<Oid>,
    pub lines: Option<usize>,
    pub bytes: Option<usize>,
    pub link: Option<String>,
    pub commit: Option<String>,
}

pub struct Dir {
    pub path: String,
    pub entries: Vec<TreeEntry>,
    by_name: HashMap<String, usize>,
}

impl Dir {
    pub fn entry(&self, name: &str) -> Option<&TreeEntry> {
        self.by_name.get(name).map(|&i| &self.entries[i])
    }
}

pub fn archive_name(repo: &str, tag: &str) -> String {
    let tag = match tag.strip_prefix('v') {
        Some(rest) if rest.starts_with(|c: char| c.is_ascii_digit()) => rest,
        _ => tag,
    };
    format!("{repo}-{}", tag.replace('/', "-"))
}

pub fn join_path(parent: &str, child: &str) -> String {
    if parent.is_empty() || child.is_empty() {
        format!("{parent}{child}")
    } else {
        format!("{parent}/{child}")
    }
}

pub struct RefInfo {
    pub name: String,
    pub dir: String,
    pub id: String,
    pub time: i64,
    pub date: DateTime<FixedOffset>,
    pub summary: String,
    pub dirs: BTreeMap<String, Dir>,
    pub submodules: Vec<(String, String)>,
}

impl RefInfo {
    pub fn newest_first(a: &RefInfo, b: &RefInfo) -> Ordering {
        b.time.cmp(&a.time).then_with(|| a.name.cmp(&b.name))
    }

    pub fn is_tag(&self) -> bool {
        self.dir.starts_with("tags/")
    }
}

pub struct Release {
    pub tag: String,
    pub date: DateTime<FixedOffset>,
    pub notes: String,
    pub dir: PathBuf,
    pub files: Vec<String>,
}

pub struct RepoInfo {
    pub git: Repository,
    pub path: String,
    pub name: String,
    pub description: String,
    pub owner: String,
    pub head_dir: String,
    pub branches: Vec<RefInfo>,
    pub tags: Vec<RefInfo>,
    pub releases: Vec<Release>,
    pub commit_list: Vec<CommitInfo>,
}

impl RepoInfo {
    pub fn open(path: &str, default_owner: Option<&str>) -> Result<Result<Self, &'static str>> {
        let repo = Repository::open(path)?;
        let (head_dir, head_id) = match repo.head() {
            Ok(head) if head.is_branch() => (
                format!("heads/{}", head.shorthand()?),
                head.peel_to_commit()?.id(),
            ),
            Ok(_) => return Ok(Err("HEAD is detached")),
            Err(e) if e.code() == ErrorCode::UnbornBranch => {
                return Ok(Err("HEAD's branch has no commits"));
            }
            Err(e) => return Err(e.into()),
        };
        let repo_path = repo.path();
        let name = repo_name(&repo).context("could not work out the repo's name")?;
        let mut description = read_optional_file(repo_path, "description")?;
        if description.is_empty() {
            description = DEFAULT_DESCRIPTION.to_string();
        }
        let mut owner = read_optional_file(repo_path, "owner")?;
        if owner.is_empty() {
            owner = default_owner.unwrap_or_default().to_string();
        }
        let branches = handle_branches(&repo)?;
        let tags = handle_tags(&repo)?;
        let releases = read_releases(&repo, path, &tags)?;
        let commit_list = walk(&repo, &mut repo.revwalk()?, head_id)?;
        Ok(Ok(Self {
            git: repo,
            path: path.to_string(),
            name,
            description,
            owner,
            head_dir,
            branches,
            tags,
            releases,
            commit_list,
        }))
    }

    pub fn read_commits(&self, rev: &mut Revwalk, id: &str) -> Result<Vec<CommitInfo>> {
        walk(&self.git, rev, Oid::from_str(id)?)
    }

    pub fn read_diff(&self, id: &str) -> Result<CommitDiff> {
        let commit = self.git.find_commit(Oid::from_str(id)?)?;
        let parent_tree = commit.parents().next().map(|p| p.tree()).transpose()?;
        let mut options = DiffOptions::new();
        options.indent_heuristic(true).include_typechange(true);
        let mut diff = self.git.diff_tree_to_tree(
            parent_tree.as_ref(),
            Some(&commit.tree()?),
            Some(&mut options),
        )?;
        diff.find_similar(Some(
            DiffFindOptions::new().renames(true).rename_threshold(37),
        ))?;
        let mut files = Vec::new();
        for (i, delta) in diff.deltas().enumerate() {
            let path = |file: DiffFile| {
                let path = file.path().unwrap_or(Path::new(""));
                path.to_string_lossy().into_owned()
            };
            let (status, file) = match delta.status() {
                Delta::Added => ('A', delta.new_file()),
                Delta::Deleted => ('D', delta.old_file()),
                Delta::Renamed => ('R', delta.new_file()),
                Delta::Typechange => ('T', delta.new_file()),
                _ => ('M', delta.new_file()),
            };
            let mut change = FileChange {
                status,
                path: path(file),
                old_path: (status == 'R').then(|| path(delta.old_file())),
                added: 0,
                removed: 0,
                binary: None,
                lines: Vec::new(),
            };
            let content = |file: DiffFile| match self.git.find_blob(file.id()) {
                Ok(blob) => blob.content().to_vec(),
                Err(_) => format!("Subproject commit {}\n", file.id()).into_bytes(),
            };
            let (old, new);
            let patches = if status == 'T' {
                (old, new) = (content(delta.old_file()), content(delta.new_file()));
                vec![
                    Patch::from_buffers(&old, None, &[], None, Some(&mut options))?,
                    Patch::from_buffers(&[], None, &new, None, Some(&mut options))?,
                ]
            } else {
                Patch::from_diff(&diff, i)?.into_iter().collect()
            };
            if patches.is_empty() || patches.iter().any(|p| p.delta().flags().is_binary()) {
                let size = |id| self.git.find_blob(id).map_or(0, |b| b.size());
                change.binary = Some((size(delta.old_file().id()), size(delta.new_file().id())));
            } else {
                for patch in &patches {
                    let (_, added, removed) = patch.line_stats()?;
                    change.added += added;
                    change.removed += removed;
                    for h in 0..patch.num_hunks() {
                        let (hunk, count) = patch.hunk(h)?;
                        let header = String::from_utf8_lossy(hunk.header());
                        change.lines.push(Line {
                            origin: '@',
                            old: None,
                            new: None,
                            text: format!("{}\n", header.trim_end()),
                        });
                        for l in 0..count {
                            let line = patch.line_in_hunk(h, l)?;
                            change.lines.push(Line {
                                origin: line.origin(),
                                old: line.old_lineno(),
                                new: line.new_lineno(),
                                text: String::from_utf8_lossy(line.content()).into_owned(),
                            });
                        }
                    }
                }
            }
            files.push(change);
        }
        Ok(CommitDiff {
            parents: commit.parent_ids().map(|id| id.to_string()).collect(),
            files,
        })
    }
}

fn repo_name(repo: &Repository) -> Option<String> {
    let dir = repo.workdir().unwrap_or(repo.path());
    let name = dir.file_name()?.to_str()?;
    let name = name.strip_suffix(".git").unwrap_or(name);
    (!matches!(name, "" | "." | "..")).then(|| name.to_string())
}

fn handle_branches(repo: &Repository) -> Result<Vec<RefInfo>> {
    let mut branches = Vec::new();
    for branch in repo.branches(Some(BranchType::Local))? {
        let (branch, _) = branch?;
        let Some(name) = branch.name()? else {
            continue;
        };
        let commit = branch.get().peel_to_commit()?;
        let time = commit.committer().when();
        branches.push(ref_info(repo, "heads", name, &commit, time)?);
    }
    branches.sort_by(RefInfo::newest_first);
    Ok(branches)
}

fn handle_tags(repo: &Repository) -> Result<Vec<RefInfo>> {
    let mut tags = Vec::new();
    for tag_ref in repo.references_glob("refs/tags/*")? {
        let tag_ref = tag_ref?;
        let Ok(name) = tag_ref.shorthand() else {
            continue;
        };
        let Ok(commit) = tag_ref.peel_to_commit() else {
            continue;
        };
        let tagger_time = tag_ref
            .peel_to_tag()
            .ok()
            .and_then(|tag| tag.tagger().map(|sig| sig.when()));
        let time = tagger_time.unwrap_or(commit.committer().when());
        tags.push(ref_info(repo, "tags", name, &commit, time)?);
    }
    tags.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| a.name.cmp(&b.name)));
    Ok(tags)
}

fn ref_info(
    repo: &Repository,
    kind: &str,
    name: &str,
    commit: &Commit,
    time: git2::Time,
) -> Result<RefInfo> {
    Ok(RefInfo {
        name: name.to_string(),
        dir: format!("{kind}/{name}"),
        id: commit.id().to_string(),
        time: time.seconds(),
        date: commit_date(commit)?,
        summary: commit_summary(commit),
        dirs: read_dirs(repo, commit)?,
        submodules: read_gitmodules(repo, commit)?,
    })
}

fn read_gitmodules(repo: &Repository, commit: &Commit) -> Result<Vec<(String, String)>> {
    let Some(blob) = commit
        .tree()?
        .get_name(".gitmodules")
        .and_then(|entry| repo.find_blob(entry.id()).ok())
    else {
        return Ok(Vec::new());
    };
    let mut sections: Vec<(Option<String>, Option<String>)> = Vec::new();
    for line in String::from_utf8_lossy(blob.content())
        .lines()
        .map(str::trim)
    {
        if line.starts_with('[') {
            sections.push((None, None));
        } else if let (Some((key, value)), Some(section)) =
            (line.split_once('='), sections.last_mut())
        {
            let value = Some(value.trim().trim_matches('"').to_string());
            match key.trim().to_ascii_lowercase().as_str() {
                "path" => section.0 = value,
                "url" => section.1 = value,
                _ => {}
            }
        }
    }
    Ok(sections
        .into_iter()
        .filter_map(|(path, url)| Some((path?, url?)))
        .collect())
}

fn read_dirs(repo: &Repository, commit: &Commit) -> Result<BTreeMap<String, Dir>> {
    let mut dirs = BTreeMap::new();
    read_dir(repo, &commit.tree()?, "", &mut dirs)?;
    Ok(dirs)
}

fn read_dir(
    repo: &Repository,
    tree: &Tree,
    path: &str,
    dirs: &mut BTreeMap<String, Dir>,
) -> Result<()> {
    let mut entries = Vec::new();
    for entry in tree.iter() {
        let Ok(name) = entry.name() else {
            continue;
        };
        let (blob, lines, bytes, link) = match entry.kind() {
            Some(ObjectType::Blob) if entry.filemode() == 0o120000 => {
                let target = repo.find_blob(entry.id())?;
                let target = String::from_utf8_lossy(target.content()).into_owned();
                (None, None, None, Some(target))
            }
            Some(ObjectType::Blob) => {
                let blob = repo.find_blob(entry.id())?;
                let lines = (!blob.is_binary()).then(|| count_lines(blob.content()));
                (Some(entry.id()), lines, Some(blob.size()), None)
            }
            Some(ObjectType::Tree) => {
                let sub_path = join_path(path, name);
                read_dir(repo, &repo.find_tree(entry.id())?, &sub_path, dirs)?;
                (None, None, None, None)
            }
            _ => (None, None, None, None),
        };
        entries.push(TreeEntry {
            name: name.to_string(),
            is_dir: entry.kind() == Some(ObjectType::Tree),
            blob,
            lines,
            bytes,
            link,
            commit: (entry.kind() == Some(ObjectType::Commit)).then(|| entry.id().to_string()),
        });
    }
    entries.sort_by_key(|entry| !entry.is_dir);
    let by_name = entries
        .iter()
        .enumerate()
        .map(|(i, entry)| (entry.name.clone(), i))
        .collect();
    let dir = Dir {
        path: path.to_string(),
        entries,
        by_name,
    };
    dirs.insert(dir.path.clone(), dir);
    Ok(())
}

fn count_lines(content: &[u8]) -> usize {
    let newlines = content.iter().filter(|&&b| b == b'\n').count();
    if content.last().is_some_and(|&b| b != b'\n') {
        newlines + 1
    } else {
        newlines
    }
}

fn walk(repo: &Repository, rev: &mut Revwalk, start: Oid) -> Result<Vec<CommitInfo>> {
    rev.reset()?;
    rev.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;
    rev.push(start)?;

    let mut commit_list = Vec::new();
    for oid in rev.by_ref() {
        commit_list.push(commit_info(&repo.find_commit(oid?)?)?);
    }
    Ok(commit_list)
}

fn commit_info(commit: &Commit) -> Result<CommitInfo> {
    let author = commit.author();
    let committer = commit.committer();
    let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
    Ok(CommitInfo {
        id: commit.id().to_string(),
        summary: commit_summary(commit),
        message: text(commit.message_bytes()),
        author_name: text(author.name_bytes()),
        author_email: text(author.email_bytes()),
        author_date: to_date(author.when())
            .with_context(|| format!("commit {}: invalid author date", commit.id()))?,
        committer_name: text(committer.name_bytes()),
        committer_email: text(committer.email_bytes()),
        committer_date: commit_date(commit)?,
    })
}

fn commit_summary(commit: &Commit) -> String {
    let summary = String::from_utf8_lossy(commit.summary_bytes().unwrap_or_default());
    if summary.is_empty() {
        NO_MESSAGE.to_string()
    } else {
        summary.into_owned()
    }
}

fn commit_date(commit: &Commit) -> Result<DateTime<FixedOffset>> {
    to_date(commit.committer().when())
        .with_context(|| format!("commit {}: invalid committer date", commit.id()))
}

fn to_date(time: git2::Time) -> Option<DateTime<FixedOffset>> {
    let offset = FixedOffset::east_opt(time.offset_minutes() * 60)?;
    let date = DateTime::from_timestamp(time.seconds(), 0)?;
    Some(date.with_timezone(&offset))
}

fn read_releases(repo: &Repository, path: &str, tags: &[RefInfo]) -> Result<Vec<Release>> {
    let repo_path = repo.path();
    let releases_dir = repo_path.join("releases");
    let folders = match fs::read_dir(&releases_dir) {
        Ok(folders) => folders,
        Err(e) if e.kind() == NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).context("releases"),
    };
    for folder in folders {
        let name = folder.context("releases")?.file_name();
        let name = name.to_string_lossy();
        let nested = format!("{name}/");
        if !tags
            .iter()
            .any(|t| t.name == name || t.name.starts_with(&nested))
        {
            eprintln!("{path}: releases/{name}: no such tag, skipped");
        }
    }
    let mut releases = Vec::new();
    for tag in tags {
        let dir = releases_dir.join(&tag.name);
        let context = || format!("releases/{}", tag.name);
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == NotFound => continue,
            Err(e) => return Err(e).with_context(context),
        };
        let mut files = Vec::new();
        for entry in entries {
            let entry = entry.with_context(context)?;
            if entry.file_type().with_context(context)?.is_file()
                && let Ok(name) = entry.file_name().into_string()
            {
                files.push(name);
            }
        }
        if files.is_empty() {
            continue;
        }
        files.sort();
        let tag_ref = repo.find_reference(&format!("refs/tags/{}", tag.name))?;
        let annotated = tag_ref.peel_to_tag().ok();
        let notes = annotated
            .as_ref()
            .and_then(|t| t.message_bytes())
            .map(|m| String::from_utf8_lossy(m).trim().to_string())
            .unwrap_or_default();
        let date = annotated
            .as_ref()
            .and_then(|t| t.tagger())
            .and_then(|sig| to_date(sig.when()))
            .unwrap_or(tag.date);
        releases.push(Release {
            tag: tag.name.clone(),
            date,
            notes,
            dir,
            files,
        });
    }
    releases.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| a.tag.cmp(&b.tag)));
    Ok(releases)
}

fn read_optional_file(repo_path: &Path, file_name: &str) -> Result<String> {
    let real_path = repo_path.join(file_name);
    match read_to_string(real_path) {
        Ok(s) => Ok(s.trim().to_string()),
        Err(e) if e.kind() == NotFound => Ok(String::new()),
        Err(e) => Err(e).context(file_name.to_string()),
    }
}
