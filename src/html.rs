use crate::repo_info::{
    CommitDiff, CommitInfo, FileChange, Folder, Line, RefInfo, RepoInfo, TreeEntry,
};

use chrono::{DateTime, FixedOffset};
use pulldown_cmark::{
    BlockQuoteKind, Event, LinkType, Options, Parser, Tag, TagEnd, html::push_html,
};
use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
};

const BRAND: &str = "wit";

const CLONE_URL_BASE: &str = "https://git.paramogi.world/";

const BRIEF_COUNT: usize = 5;

const DATE: &str = "%d-%m-%Y";

const DATE_TIME: &str = "%d-%m-%Y&nbsp;%H:%M:%S";

pub const COMMITS_PER_PAGE: usize = 2000;

const SUMMARY_REF_COUNT: usize = 3;

const BYTE_UNITS: [&str; 5] = ["b", "kb", "mb", "gb", "tb"];

const README_NAMES: [&str; 2] = ["README.md", "README"];

const REPO_FILES: [(&str, [&str; 2]); 2] = [
    ("README", README_NAMES),
    ("LICENSE", ["LICENSE", "COPYING"]),
];

const IMAGE_TYPES: [(&str, &str); 6] = [
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("gif", "image/gif"),
    ("webp", "image/webp"),
    ("svg", "image/svg+xml"),
];

const REF_MENU_ROWS: usize = 8;

const REF_CHIPS: usize = 2;

const MAX_LINKS: usize = 40;

const PAGE_IDS: [&str; 10] = [
    "header",
    "got_link",
    "site_path",
    "site_link",
    "briefs_content",
    "branches_content",
    "tags_content",
    "tree_content",
    "ref-branches",
    "ref-tags",
];

const BASE64_CHARS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

const FILE_ICON: &str = include_str!("icons/file-16.svg");

const FOLDER_ICON: &str = include_str!("icons/file-directory-16.svg");

const SYMLINK_ICON: &str = include_str!("icons/file-symlink-file-16.svg");

const SUBMODULE_ICON: &str = include_str!("icons/file-submodule-16.svg");

const BRANCH_ICON: &str = include_str!("icons/git-branch-16.svg");

const COMMIT_ICON: &str = include_str!("icons/git-commit-16.svg");

const TAG_ICON: &str = include_str!("icons/tag-16.svg");

const CHEVRON_DOWN: &str = include_str!("icons/chevron-down-16.svg");
const CHEVRON_LEFT: &str = include_str!("icons/chevron-left-16.svg");

const CHEVRON_RIGHT: &str = include_str!("icons/chevron-right-16.svg");
const CHEVRON_UP: &str = include_str!("icons/chevron-up-16.svg");

const DIFF_ADDED_ICON: &str = include_str!("icons/diff-added-16.svg");

const DIFF_REMOVED_ICON: &str = include_str!("icons/diff-removed-16.svg");

const DIFF_MODIFIED_ICON: &str = include_str!("icons/diff-modified-16.svg");

const DIFF_RENAMED_ICON: &str = include_str!("icons/diff-renamed-16.svg");

const LINK_ICON: &str = include_str!("icons/link-16.svg");

const INFO_ICON: &str = include_str!("icons/info-16.svg");

const LIGHT_BULB_ICON: &str = include_str!("icons/light-bulb-16.svg");

const REPORT_ICON: &str = include_str!("icons/report-16.svg");

const ALERT_ICON: &str = include_str!("icons/alert-16.svg");

const STOP_ICON: &str = include_str!("icons/stop-16.svg");

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Summary,
    Tree,
    Commits,
    Branches,
    Tags,
}

const TABS: [Tab; 5] = [
    Tab::Summary,
    Tab::Tree,
    Tab::Commits,
    Tab::Branches,
    Tab::Tags,
];

impl Tab {
    fn label(self) -> &'static str {
        match self {
            Tab::Summary => "summary",
            Tab::Tree => "tree",
            Tab::Commits => "commits",
            Tab::Branches => "branches",
            Tab::Tags => "tags",
        }
    }

    fn main_class(self) -> &'static str {
        match self {
            Tab::Summary => "action-readme",
            Tab::Tree => "action-code",
            Tab::Commits => "action-commits",
            Tab::Branches => "action-branches",
            Tab::Tags => "action-tags",
        }
    }

    fn href(self, git_ref: &str) -> String {
        match self {
            Tab::Summary => "index.html".to_string(),
            Tab::Tree => format!("tree/{}/index.html", percent_encode(git_ref)),
            Tab::Commits => format!("commits/{}/index.html", percent_encode(git_ref)),
            Tab::Branches => "branches.html".to_string(),
            Tab::Tags => "tags.html".to_string(),
        }
    }
}

fn repo_tabs(repo: &RepoInfo) -> impl Iterator<Item = Tab> {
    TABS.into_iter()
        .filter(move |&tab| tab != Tab::Tags || !repo.tags.is_empty())
}

struct RepoNav<'a> {
    repo: &'a RepoInfo,
    git_ref: &'a str,
    tab: Tab,
    depth: usize,
    path: &'a str,
}

fn ref_icon(git_ref: &RefInfo) -> &'static str {
    if git_ref.dir.starts_with("tags/") {
        TAG_ICON
    } else {
        BRANCH_ICON
    }
}

fn crumb(text: &str, href: Option<String>) -> String {
    match href {
        Some(href) => format!("<a href=\"{href}\">{}</a>", escape_str(text)),
        None => format!("<a class=\"btn-active\" href=\"\">{}</a>", escape_str(text)),
    }
}

fn escape_str(s: &str) -> String {
    let mut escaped_str = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => escaped_str.push_str("&amp;"),
            '<' => escaped_str.push_str("&lt;"),
            '>' => escaped_str.push_str("&gt;"),
            '"' => escaped_str.push_str("&quot;"),
            '\'' => escaped_str.push_str("&#39;"),
            _ => escaped_str.push(c),
        }
    }
    escaped_str
}

fn format_bytes(bytes: usize) -> String {
    if bytes < 1000 {
        return format!("{bytes} b");
    }
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit + 1 < BYTE_UNITS.len() {
        value /= 1000.0;
        unit += 1;
    }
    let value = (value * 10.0 + 0.5).floor() / 10.0;
    if value < 10.0 {
        format!("{value:.1} {}", BYTE_UNITS[unit])
    } else {
        format!("{value:.0} {}", BYTE_UNITS[unit])
    }
}

fn percent_encode(s: &str) -> String {
    let mut encoded = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~/".contains(&b) {
            encoded.push(b as char);
        } else {
            encoded.push_str(&format!("%{b:02X}"));
        }
    }
    encoded
}

fn find_ref<'a>(repo: &'a RepoInfo, dir: &str) -> Option<&'a RefInfo> {
    repo.branches
        .iter()
        .chain(&repo.tags)
        .find(|git_ref| git_ref.dir == dir)
}

fn root_folder(git_ref: &RefInfo) -> Option<&Folder> {
    git_ref.folders.iter().find(|f| f.path.is_empty())
}

fn repo_file<'a>(folder: &'a Folder, names: &[&str]) -> Option<&'a TreeEntry> {
    names
        .iter()
        .find_map(|name| folder.entries.iter().find(|e| !e.is_dir && e.name == *name))
}

fn file_links(nav: &RepoNav, repo_root: &str) -> Vec<String> {
    let Some(git_ref) = find_ref(nav.repo, nav.git_ref) else {
        return Vec::new();
    };
    let Some(root) = root_folder(git_ref) else {
        return Vec::new();
    };
    REPO_FILES
        .iter()
        .filter_map(|(label, names)| {
            let path = follow_links(git_ref, &repo_file(root, names)?.name)?;
            let href = path_href(git_ref, repo_root, &path)?;
            let class = if path == nav.path {
                " class=\"btn-active\""
            } else {
                ""
            };
            Some(format!(
                "<a{class} href=\"{}\">{label}</a>",
                escape_str(&href)
            ))
        })
        .collect()
}

fn write_readme(
    body: &mut String,
    repo: &RepoInfo,
    git_ref: &RefInfo,
    folder: &Folder,
    repo_root: &str,
) {
    let Some(path) = repo_file(folder, &README_NAMES)
        .and_then(|entry| follow_links(git_ref, &join_path(&folder.path, &entry.name)))
    else {
        return;
    };
    let Some(entry) = find_file(git_ref, &path) else {
        return;
    };
    let Some(blob) = entry
        .blob
        .filter(|_| entry.lines.is_some())
        .and_then(|id| repo.git.find_blob(id).ok())
    else {
        return;
    };
    body.push_str(
        "\
<div class=\"summary-readme\">\n\
<header class=\"subtitle\">\n\
<h2>readme</h2>\n\
</header>\n\
",
    );
    if is_markdown(&entry.name) {
        let base = path.rsplit_once('/').map_or("", |(dir, _)| dir);
        write_markdown(body, repo, git_ref, base, repo_root, blob.content());
    } else {
        body.push_str(&format!(
            "<pre class=\"white-space-bs\">{}</pre>\n",
            escape_str(&String::from_utf8_lossy(blob.content()))
        ));
    }
    body.push_str("</div>\n");
}

fn wrap_page(title: &str, nav: Option<&RepoNav>, body: &str) -> String {
    let (root, title) = match nav {
        Some(nav) => (
            "../".repeat(nav.depth + 1),
            format!("{} - {title}", nav.tab.label()),
        ),
        None => (String::new(), title.to_string()),
    };
    let mut page = format!(
        "\
<!DOCTYPE html>\n\
<html lang=\"en\">\n\
<head>\n\
<meta charset=\"utf-8\">\n\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
<title>{}</title>\n\
<link rel=\"stylesheet\" href=\"{root}style.css\">\n\
</head>\n\
<body>\n\
<header id=\"header\">\n\
<div class=\"root-header-shell\">\n\
<div id=\"got_link\"><a class=\"brand\" href=\"{root}index.html\">{BRAND}</a></div>\n\
</div>\n\
</header>\n\
",
        escape_str(&title)
    );

    let main_class = match nav {
        Some(nav) => {
            let repo_root = "../".repeat(nav.depth);
            page.push_str(&format!(
                "\
<nav id=\"site_path\"><div id=\"site_link\">{}</div></nav>\n\
<div class=\"navs_wrapper flex flex-wrap items-center gap\"><nav class=\"navs buttons flex\">\n\
",
                crumb(&nav.repo.name, Some(format!("{repo_root}index.html")))
            ));
            let links: Vec<String> = repo_tabs(nav.repo)
                .map(|tab| {
                    let class = if tab == nav.tab {
                        " class=\"btn-active\""
                    } else {
                        ""
                    };
                    format!(
                        "<a{class} href=\"{repo_root}{}\">{}</a>",
                        tab.href(nav.git_ref),
                        tab.label()
                    )
                })
                .collect();
            page.push_str(&format!("{}\n</nav>\n", links.join("\n")));
            let files = file_links(nav, &repo_root);
            if !files.is_empty() {
                page.push_str(&format!(
                    "<nav class=\"navs buttons flex ml-auto\">\n{}\n</nav>\n",
                    files.join("\n")
                ));
            }
            page.push_str("</div>\n");
            nav.tab.main_class()
        }
        None => "action-index",
    };

    page.push_str(&format!(
        "\
<main class=\"{main_class}\">\n\
{body}\
</main>\n\
</body>\n\
</html>\n\
"
    ));

    page
}

pub fn gen_index(repos: &[RepoInfo]) -> String {
    let mut sorted: Vec<&RepoInfo> = repos.iter().collect();
    sorted.sort_by_key(|repo| Reverse(repo.commit_list.first().map(|c| c.committer_date)));

    let mut body = String::from("<div class=\"home-grid\">\n");
    for repo in sorted {
        let name = escape_str(&repo.name);
        let dir = percent_encode(&repo.name);
        let links: Vec<String> = repo_tabs(repo)
            .map(|tab| {
                format!(
                    "<a href=\"{dir}/{}\">{}</a>",
                    tab.href(&repo.head_dir),
                    tab.label()
                )
            })
            .collect();
        let date = &repo.commit_list[0].committer_date;
        body.push_str(&format!(
            "\
<div class=\"box home-card\">\n\
<div>\n\
<div class=\"flex flex-wrap items-center gap\">\n\
<p class=\"repo-name\"><a href=\"{dir}/index.html\">{name}</a></p>\n\
<div class=\"ml-auto text-sm\" title=\"{}\">last commit {}</div>\n\
</div>\n\
<div class=\"repo-desc\">{}</div>\n\
</div>\n\
<nav class=\"repo-links\">{}</nav>\n\
</div>\n\
",
            format_zoned(date, DATE_TIME),
            date.format(DATE),
            escape_str(&repo.description),
            links.join(" | ")
        ));
    }
    body.push_str("</div>\n");

    wrap_page("repositories", None, &body)
}

pub fn gen_summary(
    repo: &RepoInfo,
    stats: &HashMap<String, String>,
    last: &HashMap<String, &CommitInfo>,
) -> String {
    let nav = RepoNav {
        repo,
        git_ref: &repo.head_dir,
        tab: Tab::Summary,
        depth: 0,
        path: "",
    };
    let last_commit = repo
        .commit_list
        .first()
        .map(|commit| commit.committer_date.format(DATE).to_string());
    let clone_url = format!("{CLONE_URL_BASE}{}.git", repo.name);
    let mut rows: Vec<(&str, &str)> =
        vec![("name", &repo.name), ("description", &repo.description)];
    if !repo.owner.is_empty() {
        rows.push(("owner", &repo.owner));
    }
    if let Some(date) = &last_commit {
        rows.push(("last commit", date));
    }
    rows.push(("clone url", &clone_url));

    let mut body = String::from("<div class=\"box summary-meta-box\">\n");
    for (key, val) in rows {
        body.push_str(&format!(
            "\
<div class=\"summary-meta-row border-b\">\n\
<span class=\"summary-meta-key\">{key}</span>\n\
<span class=\"summary-meta-val\">{}</span>\n\
</div>\n\
",
            escape_str(val)
        ));
    }
    body.push_str("</div>\n");
    write_briefs(&mut body, repo, stats);
    if repo.branches.len() > 1 {
        write_summary_refs(
            &mut body,
            Tab::Branches,
            &repo.branches,
            "branches_wrapper",
            "branch",
        );
    }
    if !repo.tags.is_empty() {
        write_summary_refs(&mut body, Tab::Tags, &repo.tags, "tag_wrapper", "tag_name");
    }
    let head = find_ref(repo, &repo.head_dir);
    if let Some(head) = head
        && let Some(root) = root_folder(head)
    {
        body.push_str(
            "\
<div class=\"summary-tree\">\n\
<header class=\"subtitle\">\n\
<h2>tree</h2>\n\
</header>\n\
<div id=\"tree_content\">\n\
",
        );
        let details = bar_details("", &head.id, &head.summary, &head.date);
        body.push_str(&ref_bar(repo, head, "", false, false, "", &details));
        let tree_dir = format!("tree/{}/", repo.head_dir);
        let blob_dir = format!("blob/{}/", repo.head_dir);
        write_listing(&mut body, head, root, "", &tree_dir, &blob_dir, last);
        body.push_str("</div>\n</div>\n");
        write_readme(&mut body, repo, head, root, "");
    }

    wrap_page(&repo.name, Some(&nav), &body)
}

pub fn gen_branches(repo: &RepoInfo) -> String {
    gen_refs(repo, Tab::Branches, &repo.branches)
}

pub fn gen_tags(repo: &RepoInfo) -> String {
    gen_refs(repo, Tab::Tags, &repo.tags)
}

fn gen_refs(repo: &RepoInfo, tab: Tab, refs: &[RefInfo]) -> String {
    let nav = RepoNav {
        repo,
        git_ref: &repo.head_dir,
        tab,
        depth: 0,
        path: "",
    };

    let mut body = String::from("<div class=\"box mt-0 p-0\">\n");
    for git_ref in refs {
        body.push_str(&format!(
            "\
<div class=\"flex justify-between items-center gap-2 py px tree-row border-b\">\n\
<div class=\"flex-1 flex items-center gap\">{}<a href=\"{}\">{}</a></div>\n\
{}\
{}\
</div>\n\
",
            ref_icon(git_ref),
            Tab::Tree.href(&git_ref.dir),
            escape_str(&git_ref.name),
            summary_cell("", &git_ref.id, &git_ref.summary),
            date_cell(&git_ref.date)
        ));
    }
    body.push_str("</div>\n");

    wrap_page(&repo.name, Some(&nav), &body)
}

fn summary_cell(repo_root: &str, id: &str, summary: &str) -> String {
    let summary = escape_str(summary);
    format!(
        "<div class=\"flex-2 truncate tree-commit\"><a href=\"{repo_root}commits/{id}.html\" title=\"{summary}\">{summary}</a></div>\n"
    )
}

fn bar_details(repo_root: &str, id: &str, summary: &str, date: &DateTime<FixedOffset>) -> String {
    format!(
        "\
{}\
<div class=\"ml-auto flex items-center gap\">\n\
<div class=\"tree-commit mono\"><a href=\"{repo_root}commits/{id}.html\">{}</a></div>\n\
{}\
</div>\n\
",
        summary_cell(repo_root, id, summary),
        &id[..7],
        date_cell(date)
    )
}

fn date_cell(date: &DateTime<FixedOffset>) -> String {
    format!(
        "<div class=\"tree-date\" title=\"{}\">{}</div>\n",
        format_zoned(date, DATE_TIME),
        date.format(DATE)
    )
}

fn format_zoned(date: &DateTime<FixedOffset>, format: &str) -> String {
    let seconds = date.offset().local_minus_utc();
    let sign = if seconds < 0 { '-' } else { '+' };
    let (hours, minutes) = (seconds.abs() / 3600, seconds.abs() % 3600 / 60);
    let zone = match (hours, minutes) {
        (0, 0) => String::new(),
        (_, 0) => format!("{sign}{hours}"),
        _ => format!("{sign}{hours}:{minutes:02}"),
    };
    format!("{}&nbsp;UTC{zone}", date.format(format))
}

pub fn gen_tree(
    repo: &RepoInfo,
    git_ref: &RefInfo,
    folder: &Folder,
    last: &HashMap<String, &CommitInfo>,
) -> String {
    let tree_path = join_path(&git_ref.dir, &folder.path);
    let depth = 1 + tree_path.split('/').count();
    let nav = RepoNav {
        repo,
        git_ref: &git_ref.dir,
        tab: Tab::Tree,
        depth,
        path: "",
    };
    let repo_root = "../".repeat(depth);
    let blob_dir = format!("{repo_root}blob/{tree_path}/");

    let details = if folder.path.is_empty() {
        bar_details(&repo_root, &git_ref.id, &git_ref.summary, &git_ref.date)
    } else {
        last.get(&folder.path).map_or_else(String::new, |commit| {
            bar_details(
                &repo_root,
                &commit.id,
                &commit.summary,
                &commit.committer_date,
            )
        })
    };

    let mut body = ref_bar(
        repo,
        git_ref,
        &folder.path,
        false,
        false,
        &repo_root,
        &details,
    );
    write_listing(&mut body, git_ref, folder, &repo_root, "", &blob_dir, last);
    write_readme(&mut body, repo, git_ref, folder, &repo_root);

    let title = format!("{}/{}", repo.name, git_ref.name);
    wrap_page(&title, Some(&nav), &body)
}

pub fn gen_blob(
    repo: &RepoInfo,
    git_ref: &RefInfo,
    folder: &Folder,
    entry: &TreeEntry,
    content: &[u8],
    source: bool,
    last: &HashMap<String, &CommitInfo>,
) -> String {
    let depth = 1 + join_path(&git_ref.dir, &folder.path).split('/').count();
    let path = join_path(&folder.path, &entry.name);
    let nav = RepoNav {
        repo,
        git_ref: &git_ref.dir,
        tab: Tab::Tree,
        depth,
        path: &path,
    };
    let repo_root = "../".repeat(depth);
    let details = last.get(&path).map_or_else(String::new, |commit| {
        bar_details(
            &repo_root,
            &commit.id,
            &commit.summary,
            &commit.committer_date,
        )
    });

    let mut body = ref_bar(repo, git_ref, &path, true, source, &repo_root, &details);
    let lines = entry
        .lines
        .map(|n| format!("<span>{n} loc</span>\n"))
        .unwrap_or_default();
    let bytes = entry.bytes.map(format_bytes).unwrap_or_default();
    body.push_str(&format!(
        "<div class=\"flex items-center file-head\">\n<div class=\"flex gap file-size\">\n{lines}<span>{bytes}</span>\n</div>\n"
    ));
    if has_source_page(entry) {
        let root = if source { "blob" } else { "source" };
        let other = format!(
            "{repo_root}{}",
            percent_encode(&format!("{root}/{}/{path}.html", git_ref.dir))
        );
        let (source_href, rendered_href) = if source {
            (None, Some(other))
        } else {
            (Some(other), None)
        };
        body.push_str(&format!(
            "<div class=\"buttons flex ml-auto\">{}\n{}</div>\n",
            crumb("source", source_href),
            crumb("rendered", rendered_href)
        ));
    }
    body.push_str("</div>\n");
    let text = entry.lines.is_some();
    match image_type(&entry.name) {
        Some(mime) if !source => body.push_str(&format!(
            "<div class=\"box\"><img src=\"{}\" alt=\"{}\"></div>\n",
            data_uri(mime, content),
            escape_str(&entry.name)
        )),
        None if !source && text && is_markdown(&entry.name) => {
            write_markdown(&mut body, repo, git_ref, &folder.path, &repo_root, content)
        }
        _ if text => write_code(&mut body, content),
        _ => body.push_str("binary file, cannot display\n"),
    }

    let title = format!("{}/{}/{path}", repo.name, git_ref.name);
    wrap_page(&title, Some(&nav), &body)
}

pub fn commits_page(page: usize) -> String {
    if page == 1 {
        "index.html".to_string()
    } else {
        format!("{page}.html")
    }
}

pub fn gen_commits(
    repo: &RepoInfo,
    git_ref: &RefInfo,
    all: &[CommitInfo],
    stats: &HashMap<String, String>,
    page: usize,
) -> String {
    let depth = 1 + git_ref.dir.split('/').count();
    let nav = RepoNav {
        repo,
        git_ref: &git_ref.dir,
        tab: Tab::Commits,
        depth,
        path: "",
    };
    let repo_root = "../".repeat(depth);
    let total = all.len();
    let start = (page - 1) * COMMITS_PER_PAGE;
    let end = total.min(start + COMMITS_PER_PAGE);
    let commits = &all[start..end];
    let count = if page > 1 {
        format!(
            "<span class=\"font-bold\">{}&ndash;{end}</span> of {total} commits",
            start + 1
        )
    } else if end < total {
        format!("<span class=\"font-bold\">{end}</span> of {total} commits")
    } else if total == 1 {
        "<span class=\"font-bold\">1</span> commit".to_string()
    } else {
        format!("<span class=\"font-bold\">{total}</span> commits")
    };

    let mut body = format!(
        "\
<div class=\"group-2\">\n\
<div class=\"box sticky flex items-center gap border-visited flex-wrap m-0\">\n\
{}\
<div>{count}</div>\n\
</div>\n\
",
        ref_menu(repo, git_ref, &repo_root, Tab::Commits, "", false)
    );
    let mut day = String::new();
    for commit in commits {
        let date = commit.committer_date.format(DATE).to_string();
        let joined = if date == day {
            " joined"
        } else {
            if !day.is_empty() {
                body.push_str("</div>\n");
            }
            body.push_str(&format!(
                "\
<div>\n\
<div class=\"flex items-center gap mb text-sm\">\n\
{COMMIT_ICON}\n\
<span>Commits on {date}</span>\n\
</div>\n\
"
            ));
            ""
        };
        write_commit_row(
            &mut body,
            repo,
            commit,
            &stats[&commit.id],
            &repo_root,
            joined,
            &format_zoned(&commit.committer_date, "%H:%M"),
        );
        day = date;
    }
    body.push_str("</div>\n");
    let pages = total.div_ceil(COMMITS_PER_PAGE);
    if pages > 1 {
        body.push_str(&commits_pager(page, pages));
    }
    body.push_str("</div>\n");

    let title = format!("{}/{}", repo.name, git_ref.name);
    wrap_page(&title, Some(&nav), &body)
}

pub fn diff_stat(diff: &CommitDiff) -> String {
    let s = if diff.files.len() == 1 { "" } else { "s" };
    format!(
        "<strong>{}</strong> file{s} changed,&nbsp;<span class=\"color-green\">+{}</span>, <span class=\"color-red\">-{}</span>",
        diff.files.len(),
        diff.files.iter().map(|f| f.added).sum::<usize>(),
        diff.files.iter().map(|f| f.removed).sum::<usize>()
    )
}

pub fn gen_commit(
    repo: &RepoInfo,
    git_ref: &RefInfo,
    commit: &CommitInfo,
    diff: &CommitDiff,
) -> String {
    let nav = RepoNav {
        repo,
        git_ref: &git_ref.dir,
        tab: Tab::Commits,
        depth: 1,
        path: "",
    };
    let short = &commit.id[..7];
    let parents: Vec<String> = diff
        .parents
        .iter()
        .map(|id| format!("<a href=\"{id}.html\">{}</a>", &id[..7]))
        .collect();
    let parents = match parents.len() {
        0 => String::new(),
        1 => format!("<div class=\"people-id\">parent {}</div>\n", parents[0]),
        _ => format!(
            "<div class=\"people-id\">parents {}</div>\n",
            parents.join(", ")
        ),
    };
    let format = "%d-%m-%Y&nbsp;%H:%M";

    let mut body = format!(
        "\
<div class=\"people\">\n\
<div>author:</div>\n\
<div>{} &lt;{}&gt; on {}</div>\n\
<div class=\"people-id\">commit {short}</div>\n\
<div>committer:</div>\n\
<div>{} &lt;{}&gt; on {}</div>\n\
{parents}\
</div>\n\
",
        escape_str(&commit.author_name),
        escape_str(&commit.author_email),
        format_zoned(&commit.author_date, format),
        escape_str(&commit.committer_name),
        escape_str(&commit.committer_email),
        format_zoned(&commit.committer_date, format)
    );
    body.push_str(&format!(
        "\
<div class=\"box flex gap mono mb-0\">\n\
<div class=\"flex-1 white-space-bs\">{}</div>\n\
<div>{}</div>\n\
</div>\n\
",
        escape_str(commit.message.trim_end()),
        diff_stat(diff)
    ));
    if !diff.files.is_empty() {
        body.push_str("<div class=\"box mono joined\">\n");
        for (n, file) in diff.files.iter().enumerate() {
            body.push_str(&format!(
                "<div class=\"my flex items-center gap\">{}<a href=\"#diff-{n}\">{}</a></div>\n",
                status_icon(file.status),
                change_path(file)
            ));
        }
        body.push_str("</div>\n");
    }
    for (n, file) in diff.files.iter().enumerate() {
        body.push_str(&format!(
            "\
<div>\n\
<div id=\"diff-{n}\" class=\"box sticky border-visited flex items-center justify-between mono py mb-0\">\n\
<div class=\"flex-1 flex items-center gap\">{}<a href=\"#diff-{n}\">{}</a></div>\n\
<div class=\"diff-stat\"><span class=\"color-green\">+{}</span>, <span class=\"color-red\">-{}</span></div>\n\
</div>\n\
",
            status_icon(file.status),
            change_path(file),
            file.added,
            file.removed
        ));
        match file.binary {
            Some((old, new)) => {
                let sizes = match file.status {
                    'A' => format_bytes(new),
                    'D' => format_bytes(old),
                    _ => format!("{} &rarr; {}", format_bytes(old), format_bytes(new)),
                };
                body.push_str(&format!(
                    "<div class=\"box joined\">binary file, {sizes}</div>\n"
                ));
            }
            None if !file.lines.is_empty() => write_diff(&mut body, n, &file.lines),
            None if file.status == 'R' => {
                body.push_str("<div class=\"box joined\">renamed without changes</div>\n")
            }
            None => {}
        }
        body.push_str("</div>\n");
    }

    let title = format!("{}/{short}: {}", repo.name, commit.summary);
    wrap_page(&title, Some(&nav), &body)
}

fn status_icon(status: char) -> String {
    let (icon, class, title) = match status {
        'A' => (DIFF_ADDED_ICON, " color-green", "added"),
        'D' => (DIFF_REMOVED_ICON, " color-red", "deleted"),
        'R' => (DIFF_RENAMED_ICON, "", "renamed"),
        'T' => (DIFF_MODIFIED_ICON, " color-orange", "type changed"),
        _ => (DIFF_MODIFIED_ICON, " color-orange", "modified"),
    };
    format!("<span class=\"flex{class}\" title=\"{title}\">{icon}</span>")
}

fn change_path(file: &FileChange) -> String {
    match &file.old_path {
        Some(old) => format!("{} &rarr; {}", escape_str(old), escape_str(&file.path)),
        None => escape_str(&file.path),
    }
}

fn write_diff(body: &mut String, n: usize, lines: &[Line]) {
    let max = lines.iter().filter_map(|l| l.old.max(l.new)).max();
    let width = max.unwrap_or(0).to_string().len();
    let number = |side: &str, num: Option<u32>| match num {
        Some(num) => {
            let id = format!("d{n}-{side}{num}");
            format!(
                "<span class=\"ln\" id=\"{id}\"><a class=\"lnlinks\" href=\"#{id}\">{num:>width$}</a></span>"
            )
        }
        None => format!("<span class=\"ln\">{}</span>", " ".repeat(width)),
    };
    body.push_str("<pre class=\"chroma joined\"><code>");
    for line in lines {
        let text = escape_str(&line.text);
        let (class, text, old, new) = match line.origin {
            '@' => ("", format!("<span class=\"gu\">{text}</span>"), None, None),
            '+' => (" gi", format!("+{text}"), None, line.new),
            '-' => (" gd", format!("-{text}"), line.old, None),
            ' ' => ("", format!(" {text}"), line.old, line.new),
            _ => ("", "\\ No newline at end of file\n".to_string(), None, None),
        };
        body.push_str(&format!(
            "<span class=\"line{class}\">{}{}<span class=\"cl\">{text}</span></span>",
            number("L", old),
            number("R", new)
        ));
    }
    body.push_str("</code></pre>\n");
}

fn commits_pager(page: usize, pages: usize) -> String {
    let first = page
        .saturating_sub(2)
        .clamp(1, pages.saturating_sub(4).max(1));
    let last = pages.min(first + 4);
    let arrow = |n: usize, title: &str, icon: &str| {
        format!(
            "<a href=\"{}\" title=\"{title}\">{icon}</a>",
            commits_page(n)
        )
    };
    let mut buttons = Vec::new();
    if first > 1 {
        buttons.push(arrow(1, "first page", &CHEVRON_LEFT.repeat(2)));
    }
    if page > 1 {
        buttons.push(arrow(page - 1, "previous page", CHEVRON_LEFT));
    }
    for n in first..=last {
        buttons.push(crumb(&n.to_string(), (n != page).then(|| commits_page(n))));
    }
    if page < pages {
        buttons.push(arrow(page + 1, "next page", CHEVRON_RIGHT));
    }
    if last < pages {
        buttons.push(arrow(pages, "last page", &CHEVRON_RIGHT.repeat(2)));
    }
    format!(
        "<nav class=\"navs buttons flex flex-wrap pager\">\n{}\n</nav>\n",
        buttons.join("\n")
    )
}

pub fn has_source_page(entry: &TreeEntry) -> bool {
    entry.lines.is_some() && (image_type(&entry.name).is_some() || is_markdown(&entry.name))
}

fn is_markdown(name: &str) -> bool {
    name == "README"
        || name
            .rsplit_once('.')
            .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("md"))
}

fn write_markdown(
    body: &mut String,
    repo: &RepoInfo,
    git_ref: &RefInfo,
    base: &str,
    repo_root: &str,
    content: &[u8],
) {
    let text = String::from_utf8_lossy(content);
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_GFM;
    let parsed = Parser::new_ext(&text, options).map(|event| match event {
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) if link_type != LinkType::Email => {
            let dest_url =
                link_target(git_ref, base, repo_root, &dest_url).map_or(dest_url, Into::into);
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                title,
                id,
            })
        }
        Event::Start(Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        }) => {
            let dest_url = image_data(repo, git_ref, base, &dest_url).map_or(dest_url, Into::into);
            Event::Start(Tag::Image {
                link_type,
                dest_url,
                title,
                id,
            })
        }
        Event::Html(html) => {
            Event::Html(rewrite_html(&html, repo, git_ref, base, repo_root).into())
        }
        Event::InlineHtml(html) => {
            Event::InlineHtml(rewrite_html(&html, repo, git_ref, base, repo_root).into())
        }
        _ => event,
    });
    let mut events: Vec<Event> = Vec::new();
    for event in parsed {
        if let (Some(Event::Html(block)), Event::Html(line)) = (events.last_mut(), &event) {
            *block = format!("{block}{line}").into();
        } else {
            events.push(event);
        }
    }
    let events = gather_footnotes(events);
    let mut ids = heading_ids(&events).into_iter();
    let mut html_events = Vec::with_capacity(events.len());
    for event in events {
        match event {
            Event::Start(Tag::Heading {
                level,
                classes,
                attrs,
                ..
            }) => {
                let id = ids.next().unwrap_or_default();
                let has_id = !id.is_empty();
                let anchor = anchor_link(&id);
                html_events.push(Event::Start(Tag::Heading {
                    level,
                    id: has_id.then(|| id.into()),
                    classes,
                    attrs,
                }));
                if has_id {
                    html_events.push(Event::InlineHtml(anchor.into()));
                }
            }
            Event::Start(Tag::BlockQuote(Some(kind))) => {
                let (name, label, icon) = match kind {
                    BlockQuoteKind::Note => ("note", "Note", INFO_ICON),
                    BlockQuoteKind::Tip => ("tip", "Tip", LIGHT_BULB_ICON),
                    BlockQuoteKind::Important => ("important", "Important", REPORT_ICON),
                    BlockQuoteKind::Warning => ("warning", "Warning", ALERT_ICON),
                    BlockQuoteKind::Caution => ("caution", "Caution", STOP_ICON),
                };
                html_events.push(Event::Html(
                    format!(
                        "<blockquote class=\"markdown-alert markdown-alert-{name}\">\n<p class=\"markdown-alert-title\">{icon}{label}</p>\n"
                    )
                    .into(),
                ));
            }
            Event::Html(html) => {
                let mut linked = String::with_capacity(html.len());
                let mut copied = 0;
                for (at, _) in raw_headings(&html) {
                    let id = ids.next().unwrap_or_default();
                    linked.push_str(&html[copied..at]);
                    if id.is_empty() {
                        linked.push('>');
                    } else {
                        linked.push_str(&format!(" id=\"{id}\">{}", anchor_link(&id)));
                    }
                    copied = at + 1;
                }
                linked.push_str(&html[copied..]);
                html_events.push(Event::Html(linked.into()));
            }
            _ => html_events.push(event),
        }
    }
    body.push_str("<div class=\"box markdown-body\">");
    push_html(body, html_events.into_iter());
    body.push_str("</div>\n");
}

fn gather_footnotes(events: Vec<Event<'_>>) -> Vec<Event<'_>> {
    let mut main = Vec::with_capacity(events.len());
    let mut definitions: HashMap<String, Vec<Event>> = HashMap::new();
    let mut referenced: Vec<(String, usize)> = Vec::new();
    let mut definition: Option<(String, Vec<Event>)> = None;
    for event in events {
        let event = match event {
            Event::Start(Tag::FootnoteDefinition(label)) => {
                definition = Some((label.to_string(), Vec::new()));
                continue;
            }
            Event::End(TagEnd::FootnoteDefinition) => {
                if let Some((label, body)) = definition.take() {
                    definitions.entry(label).or_insert(body);
                }
                continue;
            }
            Event::FootnoteReference(label) => {
                let number = match referenced.iter().position(|(seen, _)| *seen == *label) {
                    Some(i) => {
                        referenced[i].1 += 1;
                        i + 1
                    }
                    None => {
                        referenced.push((label.to_string(), 1));
                        referenced.len()
                    }
                };
                let count = referenced[number - 1].1;
                Event::InlineHtml(
                    format!(
                        "<sup class=\"footnote-reference\" id=\"{}\"><a href=\"#{}\">{number}</a></sup>",
                        escape_str(&footnote_ref_id(&label, count)),
                        percent_encode(&format!("fn-{label}"))
                    )
                    .into(),
                )
            }
            event => event,
        };
        match &mut definition {
            Some((_, body)) => body.push(event),
            None => main.push(event),
        }
    }
    if referenced.is_empty() {
        return main;
    }
    main.push(Event::Html("<section class=\"footnotes\">\n<ol>\n".into()));
    for (label, count) in referenced {
        let mut body = definitions.remove(&label).unwrap_or_default();
        let arrows: String = (1..=count)
            .map(|k| {
                let number = if k == 1 {
                    String::new()
                } else {
                    format!("<sup>{k}</sup>")
                };
                format!(
                    " <a href=\"#{}\" class=\"footnote-backref\">↑{number}</a>",
                    percent_encode(&footnote_ref_id(&label, k))
                )
            })
            .collect();
        let at = if matches!(body.last(), Some(Event::End(TagEnd::Paragraph))) {
            body.len() - 1
        } else {
            body.len()
        };
        body.insert(at, Event::InlineHtml(arrows.into()));
        main.push(Event::Html(
            format!("<li id=\"{}\">\n", escape_str(&format!("fn-{label}"))).into(),
        ));
        main.extend(body);
        main.push(Event::Html("</li>\n".into()));
    }
    main.push(Event::Html("</ol>\n</section>\n".into()));
    main
}

fn footnote_ref_id(label: &str, k: usize) -> String {
    if k == 1 {
        format!("fnref-{label}")
    } else {
        format!("fnref-{label}-{k}")
    }
}

fn heading_ids(events: &[Event]) -> Vec<String> {
    let mut used: HashSet<String> = PAGE_IDS.iter().map(|id| id.to_string()).collect();
    let mut texts = Vec::new();
    let mut heading: Option<String> = None;
    for event in events {
        match event {
            Event::Start(Tag::Heading { .. }) => heading = Some(String::new()),
            Event::Text(text) | Event::Code(text) => {
                if let Some(heading) = &mut heading {
                    heading.push_str(text);
                }
            }
            Event::End(TagEnd::Heading(_)) => texts.extend(heading.take()),
            Event::Html(html) => texts.extend(raw_headings(html).into_iter().map(|(_, text)| text)),
            _ => {}
        }
    }
    texts
        .iter()
        .map(|text| {
            let slug: String = text
                .to_lowercase()
                .chars()
                .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'))
                .map(|c| if c == ' ' { '-' } else { c })
                .collect();
            let mut id = slug.clone();
            let mut n = 0;
            while !slug.is_empty() && used.contains(&id) {
                n += 1;
                id = format!("{slug}-{n}");
            }
            used.insert(id.clone());
            id
        })
        .collect()
}

fn raw_headings(html: &str) -> Vec<(usize, String)> {
    let lower = html.to_ascii_lowercase();
    let bytes = lower.as_bytes();
    let mut headings = Vec::new();
    let mut from = 0;
    while let Some(i) = lower[from..].find("<h") {
        let start = from + i;
        from = start + 2;
        let Some(&level @ b'1'..=b'6') = bytes.get(from) else {
            continue;
        };
        if !bytes
            .get(from + 1)
            .is_some_and(|b| *b == b'>' || b.is_ascii_whitespace())
        {
            continue;
        }
        let Some(open_end) = lower[from..].find('>').map(|i| from + i) else {
            break;
        };
        let close_tag = format!("</h{}", level as char);
        let Some(close) = lower[open_end..].find(&close_tag).map(|i| open_end + i) else {
            break;
        };
        from = close;
        if lower[start..open_end]
            .split_whitespace()
            .any(|attr| attr.starts_with("id="))
        {
            continue;
        }
        let mut text = String::new();
        let mut in_tag = false;
        for c in html[open_end + 1..close].chars() {
            match c {
                '<' => in_tag = true,
                '>' => in_tag = false,
                _ if !in_tag => text.push(c),
                _ => {}
            }
        }
        headings.push((open_end, text.trim().to_string()));
    }
    headings
}

fn anchor_link(id: &str) -> String {
    format!("<a class=\"anchor\" href=\"#{id}\">{LINK_ICON}</a>")
}

fn rewrite_html(
    html: &str,
    repo: &RepoInfo,
    git_ref: &RefInfo,
    base: &str,
    repo_root: &str,
) -> String {
    let bytes = html.as_bytes();
    let mut rewritten = String::with_capacity(html.len());
    let mut copied = 0;
    for i in 1..bytes.len() {
        if i < copied || !bytes[i - 1].is_ascii_whitespace() {
            continue;
        }
        let starts = |attr: &str| {
            bytes[i..]
                .get(..attr.len())
                .is_some_and(|b| b.eq_ignore_ascii_case(attr.as_bytes()))
        };
        let (is_src, start) = if starts("src=") {
            (true, i + 4)
        } else if starts("href=") {
            (false, i + 5)
        } else {
            continue;
        };
        let Some(&quote) = bytes.get(start).filter(|&&q| q == b'"' || q == b'\'') else {
            continue;
        };
        let Some(len) = html[start + 1..].find(quote as char) else {
            continue;
        };
        let value = &html[start + 1..start + 1 + len];
        let target = if is_src {
            image_data(repo, git_ref, base, value)
        } else {
            link_target(git_ref, base, repo_root, value)
        };
        if let Some(target) = target {
            rewritten.push_str(&html[copied..start + 1]);
            rewritten.push_str(&escape_str(&target));
            copied = start + 1 + len;
        }
    }
    rewritten.push_str(&html[copied..]);
    rewritten
}

fn link_target(git_ref: &RefInfo, base: &str, repo_root: &str, url: &str) -> Option<String> {
    let path = repo_path(base, url.split(['?', '#']).next()?)?;
    let fragment = url
        .split_once('#')
        .map(|(_, fragment)| format!("#{fragment}"))
        .unwrap_or_default();
    Some(format!(
        "{}{fragment}",
        path_href(git_ref, repo_root, &path)?
    ))
}

fn path_href(git_ref: &RefInfo, repo_root: &str, path: &str) -> Option<String> {
    let path = follow_links(git_ref, path)?;
    if git_ref.folders.iter().any(|folder| folder.path == path) {
        let href = Tab::Tree.href(&join_path(&git_ref.dir, &path));
        return Some(format!("{repo_root}{href}"));
    }
    let entry = find_entry(git_ref, &path)?;
    if entry.blob.is_some() {
        let href = percent_encode(&format!("blob/{}/{path}.html", git_ref.dir));
        return Some(format!("{repo_root}{href}"));
    }
    entry.commit.as_ref()?;
    git_ref
        .submodules
        .iter()
        .find(|(module, url)| *module == path && url.starts_with("https://"))
        .map(|(_, url)| url.clone())
}

fn image_data(repo: &RepoInfo, git_ref: &RefInfo, base: &str, url: &str) -> Option<String> {
    let path = repo_path(base, url.split(['?', '#']).next()?)?;
    let path = follow_links(git_ref, &path)?;
    let mime = image_type(&path)?;
    let blob = repo.git.find_blob(find_file(git_ref, &path)?.blob?).ok()?;
    Some(data_uri(mime, blob.content()))
}

fn repo_path(base: &str, url: &str) -> Option<String> {
    if url.is_empty() || url.starts_with("//") || url.split('/').next()?.contains(':') {
        return None;
    }
    let url = percent_decode(url)?;
    resolve_path(if url.starts_with('/') { "" } else { base }, &url)
}

fn follow_links(git_ref: &RefInfo, path: &str) -> Option<String> {
    let mut path = path.to_string();
    let mut folder_only = false;
    for _ in 0..=MAX_LINKS {
        let dir = path.rsplit_once('/').map_or("", |(dir, _)| dir);
        let link = find_entry(git_ref, &path).and_then(|entry| entry.link.as_deref());
        match link {
            Some(target) if target.is_empty() || target.starts_with('/') => return None,
            Some(target) => {
                folder_only |= target.ends_with('/');
                path = resolve_path(dir, target)?;
            }
            None => {
                let is_folder = git_ref.folders.iter().any(|folder| folder.path == path);
                return (is_folder || !folder_only).then_some(path);
            }
        }
    }
    None
}

fn resolve_path(base: &str, path: &str) -> Option<String> {
    let mut parts: Vec<&str> = base.split('/').filter(|part| !part.is_empty()).collect();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            _ => parts.push(part),
        }
    }
    Some(parts.join("/"))
}

fn find_entry<'a>(git_ref: &'a RefInfo, path: &str) -> Option<&'a TreeEntry> {
    let (dir, name) = path.rsplit_once('/').unwrap_or(("", path));
    git_ref
        .folders
        .iter()
        .find(|folder| folder.path == dir)?
        .entries
        .iter()
        .find(|entry| entry.name == name)
}

fn find_file<'a>(git_ref: &'a RefInfo, path: &str) -> Option<&'a TreeEntry> {
    find_entry(git_ref, path).filter(|entry| entry.blob.is_some())
}

fn percent_decode(s: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        let hex = s
            .get(i + 1..i + 3)
            .filter(|hex| s.as_bytes()[i] == b'%' && hex.bytes().all(|b| b.is_ascii_hexdigit()));
        match hex.and_then(|hex| u8::from_str_radix(hex, 16).ok()) {
            Some(byte) => {
                bytes.push(byte);
                i += 3;
            }
            None => {
                bytes.push(s.as_bytes()[i]);
                i += 1;
            }
        }
    }
    String::from_utf8(bytes).ok()
}

fn data_uri(mime: &str, content: &[u8]) -> String {
    format!("data:{mime};base64,{}", base64(content))
}

fn image_type(name: &str) -> Option<&'static str> {
    let (_, ext) = name.rsplit_once('.')?;
    IMAGE_TYPES
        .iter()
        .find(|(known, _)| ext.eq_ignore_ascii_case(known))
        .map(|(_, mime)| *mime)
}

fn base64(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let byte = |i: usize| chunk.get(i).copied().unwrap_or(0);
        let n = u32::from_be_bytes([0, byte(0), byte(1), byte(2)]);
        for i in 0..4 {
            if i <= chunk.len() {
                encoded.push(BASE64_CHARS[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                encoded.push('=');
            }
        }
    }
    encoded
}

fn join_path(parent: &str, child: &str) -> String {
    if parent.is_empty() || child.is_empty() {
        format!("{parent}{child}")
    } else {
        format!("{parent}/{child}")
    }
}

fn ref_bar(
    repo: &RepoInfo,
    git_ref: &RefInfo,
    path: &str,
    file: bool,
    source: bool,
    repo_root: &str,
    details: &str,
) -> String {
    let mut bar = String::from(
        "<div class=\"box sticky flex items-center gap border-visited flex-wrap ref-bar\">\n",
    );
    bar.push_str(&ref_menu(repo, git_ref, repo_root, Tab::Tree, path, source));
    if !path.is_empty() {
        let mut parts = vec![(repo.name.as_str(), git_ref.dir.clone())];
        for part in path.split('/').filter(|p| !p.is_empty()) {
            let tree_path = format!("{}/{part}", parts[parts.len() - 1].1);
            parts.push((part, tree_path));
        }
        for (i, (part, tree_path)) in parts.iter().enumerate() {
            if i > 0 {
                bar.push_str("<span>/</span>\n");
            }
            if i == parts.len() - 1 {
                let grow = if file { " flex-1" } else { "" };
                bar.push_str(&format!(
                    "<span class=\"font-bold truncate{grow}\">{}</span>\n",
                    escape_str(part)
                ));
            } else {
                let href = format!("{repo_root}{}", Tab::Tree.href(tree_path));
                bar.push_str(&format!("{}\n", crumb(part, Some(href))));
            }
        }
    }
    bar.push_str(details);
    bar.push_str("</div>\n");
    bar
}

fn menu_target(git_ref: &RefInfo, path: &str, source: bool) -> (String, bool) {
    if let Some(entry) = find_file(git_ref, path) {
        let root = if source && has_source_page(entry) {
            "source"
        } else {
            "blob"
        };
        let href = percent_encode(&format!("{root}/{}/{path}.html", git_ref.dir));
        return (href, true);
    }
    let mut dir = path;
    while !dir.is_empty() && !git_ref.folders.iter().any(|folder| folder.path == dir) {
        dir = dir.rsplit_once('/').map_or("", |(parent, _)| parent);
    }
    (Tab::Tree.href(&join_path(&git_ref.dir, dir)), dir == path)
}

fn ref_menu(
    repo: &RepoInfo,
    git_ref: &RefInfo,
    repo_root: &str,
    tab: Tab,
    path: &str,
    source: bool,
) -> String {
    let mut menu = format!(
        "<details class=\"ref-menu\">\n<summary><code>{}{}{CHEVRON_DOWN}{CHEVRON_UP}</code></summary>\n<div class=\"ref-menu-list\">\n",
        ref_icon(git_ref),
        escape_str(&git_ref.name)
    );
    let list = |class: &str, refs: &[RefInfo]| -> String {
        let long = if refs.len() > REF_MENU_ROWS {
            " long"
        } else {
            ""
        };
        let entries: String = refs
            .iter()
            .map(|r| {
                let active = if r.dir == git_ref.dir {
                    " btn-active"
                } else {
                    ""
                };
                let (href, found) = match tab {
                    Tab::Tree => menu_target(r, path, source),
                    _ => (tab.href(&r.dir), true),
                };
                let kind = if r.dir.starts_with("tags/") {
                    "tag"
                } else {
                    "branch"
                };
                let (missing, title) = if found {
                    ("", String::new())
                } else {
                    (" missing", format!(" title=\"not in this {kind}\""))
                };
                format!(
                    "<a class=\"flex items-center gap{active}{missing}\"{title} href=\"{repo_root}{href}\">{}{}</a>\n",
                    ref_icon(r),
                    escape_str(&r.name)
                )
            })
            .collect();
        format!("<div class=\"{class} buttons{long}\">\n{entries}</div>\n")
    };
    if !repo.tags.is_empty() {
        let (branches, tags) = if git_ref.dir.starts_with("tags/") {
            ("", " checked")
        } else {
            (" checked", "")
        };
        menu.push_str(&format!(
            "\
<input type=\"radio\" name=\"ref-kind\" id=\"ref-branches\"{branches}>\n\
<input type=\"radio\" name=\"ref-kind\" id=\"ref-tags\"{tags}>\n\
<div class=\"ref-kinds buttons flex\"><label for=\"ref-branches\">branches</label><label for=\"ref-tags\">tags</label></div>\n\
{}\
",
            list("ref-tags", &repo.tags)
        ));
    }
    menu.push_str(&list("ref-branches", &repo.branches));
    menu.push_str("</div>\n</details>\n");
    menu
}

fn write_code(body: &mut String, content: &[u8]) {
    let text = String::from_utf8_lossy(content);
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let width = lines.len().to_string().len();
    body.push_str("<pre class=\"chroma\"><code>");
    for (i, line) in lines.iter().enumerate() {
        let n = i + 1;
        body.push_str(&format!(
            "<span class=\"line\"><span class=\"ln\" id=\"{n}\"><a class=\"lnlinks\" href=\"#{n}\">{n:>width$}</a></span><span class=\"cl\">{}</span></span>",
            escape_str(line)
        ));
    }
    body.push_str("</code></pre>\n");
}

fn write_listing(
    body: &mut String,
    git_ref: &RefInfo,
    folder: &Folder,
    repo_root: &str,
    tree_dir: &str,
    blob_dir: &str,
    last: &HashMap<String, &CommitInfo>,
) {
    body.push_str("<div class=\"box p-0\">\n");
    for entry in &folder.entries {
        let name = &entry.name;
        let path = join_path(&folder.path, name);
        let link = |href: String, text: String| {
            format!(
                "<a class=\"truncate\" href=\"{}\">{text}</a>",
                escape_str(&href)
            )
        };
        let leads = |text: String| match path_href(git_ref, repo_root, &path) {
            Some(href) => link(href, text),
            None => format!("<span class=\"truncate\">{text}</span>"),
        };
        let last_change = match last.get(&path) {
            Some(commit) => format!(
                "{}{}",
                summary_cell(repo_root, &commit.id, &commit.summary),
                date_cell(&commit.committer_date)
            ),
            None => "<div class=\"flex-2 tree-commit\"></div>\n<div class=\"tree-date\"></div>\n"
                .to_string(),
        };
        let (icon, name) = if entry.is_dir {
            let href = percent_encode(&format!("{tree_dir}{name}/index.html"));
            (FOLDER_ICON, link(href, escape_str(name)))
        } else if let Some(target) = &entry.link {
            let text = format!("{} &rarr; {}", escape_str(name), escape_str(target));
            (SYMLINK_ICON, leads(text))
        } else if let Some(commit) = &entry.commit {
            let text = format!("{} @ {}", escape_str(name), &commit[..7]);
            (SUBMODULE_ICON, leads(text))
        } else {
            let href = percent_encode(&format!("{blob_dir}{name}.html"));
            (FILE_ICON, link(href, escape_str(name)))
        };
        body.push_str(&format!(
            "\
<div class=\"flex justify-between items-center gap-2 py px tree-row border-b\">\n\
<div class=\"flex-1 tree-path flex items-center gap\">\n\
{icon}\n\
{name}\n\
</div>\n\
{last_change}\
</div>\n\
"
        ));
    }
    body.push_str("</div>\n");
}

fn ref_labels(repo: &RepoInfo, id: &str, repo_root: &str) -> String {
    let mut refs: Vec<&RefInfo> = repo
        .branches
        .iter()
        .chain(&repo.tags)
        .filter(|git_ref| git_ref.id == id)
        .collect();
    refs.sort_by(|a, b| RefInfo::newest_first(a, b));
    let mut labels: String = refs
        .iter()
        .take(REF_CHIPS)
        .map(|git_ref| {
            format!(
                "<code>{}<a href=\"{repo_root}{}\">{}</a></code>\n",
                ref_icon(git_ref),
                Tab::Tree.href(&git_ref.dir),
                escape_str(&git_ref.name)
            )
        })
        .collect();
    if refs.len() > REF_CHIPS {
        let rest: String = refs[REF_CHIPS..]
            .iter()
            .map(|git_ref| {
                format!(
                    "<a class=\"flex items-center gap\" href=\"{repo_root}{}\">{}{}</a>\n",
                    Tab::Tree.href(&git_ref.dir),
                    ref_icon(git_ref),
                    escape_str(&git_ref.name)
                )
            })
            .collect();
        labels.push_str(&format!(
            "<details class=\"ref-menu ref-more\">\n<summary><code>more{CHEVRON_DOWN}{CHEVRON_UP}</code></summary>\n<div class=\"ref-menu-list buttons\">\n{rest}</div>\n</details>\n"
        ));
    }
    labels
}

fn write_commit_row(
    body: &mut String,
    repo: &RepoInfo,
    commit: &CommitInfo,
    stat: &str,
    repo_root: &str,
    joined: &str,
    date: &str,
) {
    let href = format!("{repo_root}commits/{}.html", commit.id);
    let labels = ref_labels(repo, &commit.id, repo_root);
    let chips = if labels.is_empty() {
        String::new()
    } else {
        format!("<div class=\"ml-auto flex flex-wrap justify-end gap text-sm\">\n{labels}</div>\n")
    };
    body.push_str(&format!(
        "\
<div class=\"box m-0{joined}\">\n\
<div class=\"flex flex-wrap items-center gap text-sm\">\n\
<a href=\"{href}\" class=\"mono\">{}</a>\n\
<div class=\"flex items-center gap-xs\">\n\
<span>{}</span>\n\
<span>&nbsp;&centerdot;&nbsp;</span>\n\
<span>{}</span>\n\
</div>\n\
<div class=\"fits flex-1\"><div class=\"mono\">{stat}</div></div>\n\
</div>\n\
<div class=\"mt-sm flex flex-wrap items-center gap\">\n\
<div class=\"flex-1 white-space-bs\"><a href=\"{href}\">{}</a></div>\n\
{chips}\
</div>\n\
</div>\n\
",
        &commit.id[..7],
        escape_str(&commit.author_name),
        date,
        escape_str(&commit.summary)
    ));
}

fn write_briefs(body: &mut String, repo: &RepoInfo, stats: &HashMap<String, String>) {
    body.push_str(
        "\
<div class=\"summary-briefs\">\n\
<header class=\"subtitle\">\n\
<h2>commit briefs</h2>\n\
</header>\n\
<div id=\"briefs_content\">\n\
",
    );
    for (n, commit) in repo.commit_list.iter().take(BRIEF_COUNT).enumerate() {
        let joined = if n == 0 { "" } else { " joined" };
        let stat = stats.get(&commit.id).map_or("", String::as_str);
        let date = commit.committer_date.format(DATE).to_string();
        write_commit_row(body, repo, commit, stat, "", joined, &date);
    }
    body.push_str("</div>\n</div>\n");
}

fn write_summary_refs(body: &mut String, tab: Tab, refs: &[RefInfo], wrapper: &str, inner: &str) {
    let label = tab.label();
    body.push_str(&format!(
        "\
<div class=\"summary-{label}\">\n\
<header class=\"subtitle\">\n\
<h2>{label}</h2>\n\
</header>\n\
<div id=\"{label}_content\" class=\"box p-0\">\n\
"
    ));
    for git_ref in refs.iter().take(SUMMARY_REF_COUNT) {
        body.push_str(&format!(
            "\
<section class=\"{wrapper} flex justify-between items-center gap-2 border-b py px\">\n\
<div class=\"{inner} flex-1 flex items-center gap\">{}<a href=\"{}\">{}</a></div>\n\
{}\
{}\
</section>\n\
",
            ref_icon(git_ref),
            Tab::Tree.href(&git_ref.dir),
            escape_str(&git_ref.name),
            summary_cell("", &git_ref.id, &git_ref.summary),
            date_cell(&git_ref.date)
        ));
    }
    body.push_str(&format!(
        "\
<div class=\"navs_wrapper py px\">\n\
<div class=\"navs\"><a href=\"{}\">all {label}</a></div>\n\
</div>\n\
</div>\n\
</div>\n\
",
        tab.href("")
    ));
}
