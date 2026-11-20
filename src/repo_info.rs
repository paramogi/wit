use anyhow::{Context, Result};

use chrono::{DateTime, FixedOffset};
use git2::{
    BranchType, Commit, Delta, DiffFile, DiffFindOptions, DiffOptions, ErrorCode, ObjectType, Oid,
    Patch, Repository, Sort, Tree,
};
use std::{cmp::Ordering, fs::read_to_string, io::ErrorKind::NotFound, path::Path};

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

pub struct Folder {
    pub path: String,
    pub entries: Vec<TreeEntry>,
}

pub struct RefInfo {
    pub name: String,
    pub dir: String,
    pub id: String,
    pub time: i64,
    pub folders: Vec<Folder>,
    pub submodules: Vec<(String, String)>,
}

impl RefInfo {
    pub fn newest_first(a: &RefInfo, b: &RefInfo) -> Ordering {
        b.time.cmp(&a.time).then_with(|| a.name.cmp(&b.name))
    }
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
    pub commit_list: Vec<CommitInfo>,
}

impl RepoInfo {
    pub fn open(path: &str, default_owner: Option<&str>) -> Result<Option<Self>> {
        let repo = Repository::open(path)?;
        let head_dir = match repo.head() {
            Ok(head) => format!("heads/{}", head.shorthand()?),
            Err(e) if e.code() == ErrorCode::UnbornBranch => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let repo_path = repo.path();
        let name = Self::repo_name(&repo).context("could not work out the repo's name")?;
        let mut description = Self::read_optional_file(repo_path, "description")?;
        if description.is_empty() {
            description = DEFAULT_DESCRIPTION.to_string();
        }
        let mut owner = Self::read_optional_file(repo_path, "owner")?;
        if owner.is_empty() {
            owner = default_owner.unwrap_or_default().to_string();
        }
        let branches = Self::handle_branches(&repo)?;
        let tags = Self::handle_tags(&repo)?;
        let commit_list = Self::handle_commits(&repo)?;
        Ok(Some(Self {
            git: repo,
            path: path.to_string(),
            name,
            description,
            owner,
            head_dir,
            branches,
            tags,
            commit_list,
        }))
    }

    fn repo_name(repo: &Repository) -> Option<String> {
        let dir = repo.workdir().unwrap_or(repo.path());
        let name = dir.file_name()?.to_str()?;
        let name = name.strip_suffix(".git").unwrap_or(name);
        (!name.is_empty()).then(|| name.to_string())
    }

    fn handle_branches(repo: &Repository) -> Result<Vec<RefInfo>> {
        let mut branches = Vec::new();
        for branch in repo.branches(Some(BranchType::Local))? {
            let (branch, _) = branch?;
            let Some(name) = branch.name()? else {
                continue;
            };
            let commit = branch.get().peel_to_commit()?;
            branches.push(RefInfo {
                name: name.to_string(),
                dir: format!("heads/{name}"),
                id: commit.id().to_string(),
                time: commit.committer().when().seconds(),
                folders: Self::read_folders(repo, &commit)?,
                submodules: Self::read_gitmodules(repo, &commit)?,
            });
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
            tags.push(RefInfo {
                name: name.to_string(),
                dir: format!("tags/{name}"),
                id: commit.id().to_string(),
                time: time.seconds(),
                folders: Self::read_folders(repo, &commit)?,
                submodules: Self::read_gitmodules(repo, &commit)?,
            });
        }
        tags.sort_by(RefInfo::newest_first);
        Ok(tags)
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

    fn read_folders(repo: &Repository, commit: &Commit) -> Result<Vec<Folder>> {
        let mut folders = Vec::new();
        Self::read_folder(repo, &commit.tree()?, "", &mut folders)?;
        Ok(folders)
    }

    fn read_folder(
        repo: &Repository,
        tree: &Tree,
        path: &str,
        folders: &mut Vec<Folder>,
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
                    let lines = (!blob.is_binary()).then(|| Self::count_lines(blob.content()));
                    (Some(entry.id()), lines, Some(blob.size()), None)
                }
                Some(ObjectType::Tree) => {
                    let sub_path = if path.is_empty() {
                        name.to_string()
                    } else {
                        format!("{path}/{name}")
                    };
                    Self::read_folder(repo, &repo.find_tree(entry.id())?, &sub_path, folders)?;
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
        folders.push(Folder {
            path: path.to_string(),
            entries,
        });
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

    fn handle_commits(repo: &Repository) -> Result<Vec<CommitInfo>> {
        Self::walk(repo, repo.head()?.peel_to_commit()?.id())
    }

    pub fn read_commits(&self, id: &str) -> Result<Vec<CommitInfo>> {
        Self::walk(&self.git, Oid::from_str(id)?)
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

    fn walk(repo: &Repository, start: Oid) -> Result<Vec<CommitInfo>> {
        let mut rev = repo.revwalk()?;
        rev.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;
        rev.push(start)?;

        let mut commit_list = Vec::new();
        for oid in rev {
            commit_list.push(Self::commit_info(&repo.find_commit(oid?)?)?);
        }
        Ok(commit_list)
    }

    fn commit_info(commit: &Commit) -> Result<CommitInfo> {
        let author = commit.author();
        let committer = commit.committer();
        Ok(CommitInfo {
            id: commit.id().to_string(),
            summary: commit
                .summary()?
                .filter(|s| !s.is_empty())
                .unwrap_or(NO_MESSAGE)
                .to_string(),
            message: commit.message().unwrap_or_default().to_string(),
            author_name: author.name().unwrap_or_default().to_string(),
            author_email: author.email().unwrap_or_default().to_string(),
            author_date: Self::to_date(author.when())
                .with_context(|| format!("commit {}: invalid author date", commit.id()))?,
            committer_name: committer.name().unwrap_or_default().to_string(),
            committer_email: committer.email().unwrap_or_default().to_string(),
            committer_date: Self::to_date(committer.when())
                .with_context(|| format!("commit {}: invalid committer date", commit.id()))?,
        })
    }

    fn to_date(time: git2::Time) -> Option<DateTime<FixedOffset>> {
        let offset = FixedOffset::east_opt(time.offset_minutes() * 60)?;
        let date = DateTime::from_timestamp(time.seconds(), 0)?;
        Some(date.with_timezone(&offset))
    }

    fn read_optional_file(repo_path: &Path, file_name: &str) -> Result<String> {
        let real_path = repo_path.join(file_name);
        match read_to_string(real_path) {
            Ok(s) => Ok(s.trim().to_string()),
            Err(e) if e.kind() == NotFound => Ok(String::new()),
            Err(e) => Err(e).context(file_name.to_string()),
        }
    }
}
