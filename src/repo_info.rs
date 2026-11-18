use crate::error::WitError;

use chrono::{DateTime, FixedOffset};
use git2::{BranchType, Commit, ErrorCode, ObjectType, Oid, Repository, Sort, Tree};
use std::{fs::read_to_string, io::ErrorKind::NotFound, path::Path};

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

pub struct TreeEntry {
    pub name: String,
    pub is_dir: bool,
    pub blob: Option<Oid>,
    pub lines: Option<usize>,
    pub bytes: Option<usize>,
}

pub struct Folder {
    pub path: String,
    pub entries: Vec<TreeEntry>,
}

pub struct RefInfo {
    pub name: String,
    pub dir: String,
    pub id: String,
    pub folders: Vec<Folder>,
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
    pub fn open(path: &str, default_owner: Option<&str>) -> Result<Option<Self>, WitError> {
        let repo = Repository::open(path)?;
        let head_dir = match repo.head() {
            Ok(head) => format!("heads/{}", head.shorthand()?),
            Err(e) if e.code() == ErrorCode::UnbornBranch => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let repo_path = repo.path();
        let name = Self::repo_name(&repo).ok_or_else(|| WitError::NoName(path.to_string()))?;
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

    fn handle_branches(repo: &Repository) -> Result<Vec<RefInfo>, WitError> {
        let mut branches = Vec::new();
        for branch in repo.branches(Some(BranchType::Local))? {
            let (branch, _) = branch?;
            let Some(name) = branch.name()? else {
                continue;
            };
            let commit = branch.get().peel_to_commit()?;
            let info = RefInfo {
                name: name.to_string(),
                dir: format!("heads/{name}"),
                id: commit.id().to_string(),
                folders: Self::read_folders(repo, &commit)?,
            };
            branches.push((commit.committer().when().seconds(), info));
        }
        Ok(Self::newest_first(branches))
    }

    fn handle_tags(repo: &Repository) -> Result<Vec<RefInfo>, WitError> {
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
            let info = RefInfo {
                name: name.to_string(),
                dir: format!("tags/{name}"),
                id: commit.id().to_string(),
                folders: Self::read_folders(repo, &commit)?,
            };
            tags.push((time.seconds(), info));
        }
        Ok(Self::newest_first(tags))
    }

    fn read_folders(repo: &Repository, commit: &Commit) -> Result<Vec<Folder>, WitError> {
        let mut folders = Vec::new();
        Self::read_folder(repo, &commit.tree()?, "", &mut folders)?;
        Ok(folders)
    }

    fn read_folder(
        repo: &Repository,
        tree: &Tree,
        path: &str,
        folders: &mut Vec<Folder>,
    ) -> Result<(), WitError> {
        let mut entries = Vec::new();
        for entry in tree.iter() {
            let Ok(name) = entry.name() else {
                continue;
            };
            let (blob, lines, bytes) = match entry.kind() {
                Some(ObjectType::Blob) => {
                    let blob = repo.find_blob(entry.id())?;
                    let lines = (!blob.is_binary()).then(|| Self::count_lines(blob.content()));
                    (Some(entry.id()), lines, Some(blob.size()))
                }
                Some(ObjectType::Tree) => {
                    let sub_path = if path.is_empty() {
                        name.to_string()
                    } else {
                        format!("{path}/{name}")
                    };
                    Self::read_folder(repo, &repo.find_tree(entry.id())?, &sub_path, folders)?;
                    (None, None, None)
                }
                _ => (None, None, None),
            };
            entries.push(TreeEntry {
                name: name.to_string(),
                is_dir: entry.kind() == Some(ObjectType::Tree),
                blob,
                lines,
                bytes,
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

    fn newest_first(mut refs: Vec<(i64, RefInfo)>) -> Vec<RefInfo> {
        refs.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.name.cmp(&b.1.name)));
        refs.into_iter().map(|(_, info)| info).collect()
    }

    fn handle_commits(repo: &Repository) -> Result<Vec<CommitInfo>, WitError> {
        let mut rev = repo.revwalk()?;
        rev.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;
        rev.push_head()?;

        let mut commit_list = Vec::new();
        for oid in rev {
            let commit = repo.find_commit(oid?)?;
            let author = commit.author();
            let committer = commit.committer();
            commit_list.push(CommitInfo {
                id: commit.id().to_string(),
                summary: commit
                    .summary()?
                    .filter(|s| !s.is_empty())
                    .unwrap_or(NO_MESSAGE)
                    .to_string(),
                message: commit.message().unwrap_or_default().to_string(),
                author_name: author.name().unwrap_or_default().to_string(),
                author_email: author.email().unwrap_or_default().to_string(),
                author_date: Self::to_date(author.when()).ok_or(WitError::InvalidDate)?,
                committer_name: committer.name().unwrap_or_default().to_string(),
                committer_email: committer.email().unwrap_or_default().to_string(),
                committer_date: Self::to_date(committer.when()).ok_or(WitError::InvalidDate)?,
            });
        }
        Ok(commit_list)
    }

    fn to_date(time: git2::Time) -> Option<DateTime<FixedOffset>> {
        let offset = FixedOffset::east_opt(time.offset_minutes() * 60)?;
        let date = DateTime::from_timestamp(time.seconds(), 0)?;
        Some(date.with_timezone(&offset))
    }

    fn read_optional_file(repo_path: &Path, file_name: &str) -> Result<String, WitError> {
        let real_path = repo_path.join(file_name);
        match read_to_string(real_path) {
            Ok(s) => Ok(s.trim().to_string()),
            Err(e) if e.kind() == NotFound => Ok(String::new()),
            Err(e) => Err(e.into()),
        }
    }
}
