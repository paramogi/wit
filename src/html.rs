use crate::markdown::{is_markdown, write_markdown};
use crate::repo_info::{
    CommitDiff, CommitInfo, Dir, FileChange, Line, RefInfo, RepoInfo, TreeEntry, archive_name,
    join_path,
};

use chrono::{DateTime, FixedOffset};
use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
    fmt::Write,
};

const BRAND: &str = "wit";

const CLONE_URL_BASE: &str = "https://git.paramogi.world/";

const NO_DESCRIPTION: &str = "[no description]";

const BRIEF_COUNT: usize = 5;

const DATE: &str = "%d-%m-%Y";

pub const COMMITS_PER_PAGE: usize = 200;

const SUMMARY_REF_COUNT: usize = 3;

const BYTE_UNITS: [&str; 5] = ["B", "kB", "MB", "GB", "TB"];

const README_NAMES: [&str; 2] = ["README.md", "README"];

const MARKDOWN_BOX: &str = "box markdown-body";

pub const ARCHIVE_FORMATS: [&str; 2] = ["zip", "tar.gz"];

const REPO_FILES: [(&str, &str, [&str; 2]); 2] = [
    ("README", "book", README_NAMES),
    ("LICENSE", "law", ["LICENSE", "COPYING"]),
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

const BASE64_CHARS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

const ICONS: [(&str, &str); 25] = [
    ("book", include_str!("icons/book.svg")),
    ("law", include_str!("icons/law.svg")),
    ("file-directory", include_str!("icons/file-directory.svg")),
    ("file", include_str!("icons/file.svg")),
    ("file-archive", include_str!("icons/file-archive.svg")),
    (
        "file-symlink-file",
        include_str!("icons/file-symlink-file.svg"),
    ),
    ("file-submodule", include_str!("icons/file-submodule.svg")),
    ("git-branch", include_str!("icons/git-branch.svg")),
    ("git-commit", include_str!("icons/git-commit.svg")),
    ("tag", include_str!("icons/tag.svg")),
    ("release", include_str!("icons/release.svg")),
    ("chevron-down", include_str!("icons/chevron-down.svg")),
    ("chevron-right", include_str!("icons/chevron-right.svg")),
    ("chevron-up", include_str!("icons/chevron-up.svg")),
    ("chevron-left", include_str!("icons/chevron-left.svg")),
    ("diff-added", include_str!("icons/diff-added.svg")),
    ("diff-removed", include_str!("icons/diff-removed.svg")),
    ("diff-modified", include_str!("icons/diff-modified.svg")),
    ("diff-renamed", include_str!("icons/diff-renamed.svg")),
    ("link", include_str!("icons/link.svg")),
    ("info", include_str!("icons/info.svg")),
    ("light-bulb", include_str!("icons/light-bulb.svg")),
    ("report", include_str!("icons/report.svg")),
    ("alert", include_str!("icons/alert.svg")),
    ("stop", include_str!("icons/stop.svg")),
];

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Summary,
    Tree,
    Commits,
    Branches,
    Tags,
    Releases,
}

const TABS: [Tab; 6] = [
    Tab::Summary,
    Tab::Tree,
    Tab::Commits,
    Tab::Branches,
    Tab::Tags,
    Tab::Releases,
];

impl Tab {
    fn label(self) -> &'static str {
        match self {
            Tab::Summary => "summary",
            Tab::Tree => "tree",
            Tab::Commits => "commits",
            Tab::Branches => "branches",
            Tab::Tags => "tags",
            Tab::Releases => "releases",
        }
    }

    fn main_class(self) -> &'static str {
        match self {
            Tab::Summary => "",
            Tab::Tree => "action-code",
            Tab::Commits => "action-commits",
            Tab::Branches => "action-branches",
            Tab::Tags => "",
            Tab::Releases => "action-releases",
        }
    }

    fn href(self, git_ref: &str) -> String {
        match self {
            Tab::Summary => "index.html".to_string(),
            Tab::Tree => format!("tree/{}/index.html", percent_encode(git_ref)),
            Tab::Commits => format!("commits/{}/index.html", percent_encode(git_ref)),
            Tab::Branches => "branches.html".to_string(),
            Tab::Tags => "tags.html".to_string(),
            Tab::Releases => "releases.html".to_string(),
        }
    }
}

fn repo_tabs(repo: &RepoInfo) -> impl Iterator<Item = Tab> {
    TABS.into_iter().filter(move |&tab| match tab {
        Tab::Tags => !repo.tags.is_empty(),
        Tab::Releases => !repo.releases.is_empty(),
        _ => true,
    })
}

struct RepoNav<'a> {
    repo: &'a RepoInfo,
    git_ref: &'a str,
    tab: Tab,
    depth: usize,
    path: &'a str,
    commits: usize,
}

pub fn icon(name: &str) -> String {
    format!(
        "<svg width=\"16\" height=\"16\" aria-hidden=\"true\"><use href=\"#icon.{name}\"/></svg>"
    )
}

fn ref_icon(git_ref: &RefInfo) -> String {
    if git_ref.is_tag() {
        icon("tag")
    } else {
        icon("git-branch")
    }
}

pub fn escape_str(s: &str) -> String {
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
        return format!("{bytes} B");
    }
    let round = |value: f64| (value * 10.0 + 0.5).floor() / 10.0;
    let mut value = bytes as f64;
    let mut unit = 0;
    while round(value) >= 999.5 && unit + 1 < BYTE_UNITS.len() {
        value /= 1000.0;
        unit += 1;
    }
    let value = round(value);
    if value < 10.0 {
        format!("{value:.1} {}", BYTE_UNITS[unit])
    } else {
        format!("{value:.0} {}", BYTE_UNITS[unit])
    }
}

pub fn percent_encode(s: &str) -> String {
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

fn root_dir(git_ref: &RefInfo) -> Option<&Dir> {
    git_ref.dirs.get("")
}

fn repo_file<'a>(dir: &'a Dir, names: &[&str]) -> Option<&'a TreeEntry> {
    names
        .iter()
        .find_map(|name| dir.entry(name).filter(|e| !e.is_dir))
}

fn file_links(nav: &RepoNav, repo_root: &str) -> Vec<(bool, String)> {
    let Some(git_ref) = find_ref(nav.repo, nav.git_ref) else {
        return Vec::new();
    };
    let Some(root) = root_dir(git_ref) else {
        return Vec::new();
    };
    REPO_FILES
        .iter()
        .filter_map(|(label, icon_name, names)| {
            let path = follow_links(git_ref, &repo_file(root, names)?.name)?;
            let href = path_href(git_ref, repo_root, &path)?;
            let current = path == nav.path;
            Some((
                current,
                format!(
                    "<a class=\"doc-link\"{} href=\"{}\">{}{label}</a>",
                    if current {
                        " aria-current=\"page\""
                    } else {
                        ""
                    },
                    escape_str(&href),
                    icon(icon_name)
                ),
            ))
        })
        .collect()
}

fn write_readme(body: &mut String, repo: &RepoInfo, git_ref: &RefInfo, dir: &Dir, repo_root: &str) {
    let Some(path) = repo_file(dir, &README_NAMES)
        .and_then(|entry| follow_links(git_ref, &join_path(&dir.path, &entry.name)))
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
    let content = blob.content();
    let markdown = is_markdown(&entry.name);
    write_file_head(body, entry, content, markdown);
    if markdown {
        let base = path.rsplit_once('/').map_or("", |(parent, _)| parent);
        write_markdown(body, MARKDOWN_BOX, repo, git_ref, base, repo_root, content);
        write_code(body, content);
    } else {
        body.push_str(&format!(
            "<pre class=\"white-space-bs\">{}</pre>\n",
            escape_str(&String::from_utf8_lossy(content))
        ));
    }
    body.push_str("</div>\n");
}

fn icon_symbols(parts: &[&str]) -> String {
    let used: HashSet<&str> = parts
        .iter()
        .flat_map(|part| {
            part.match_indices("href=\"#icon.")
                .filter_map(|(at, found)| part[at + found.len()..].split('"').next())
        })
        .collect();
    let symbols: String = ICONS
        .iter()
        .filter(|(id, _)| used.contains(id))
        .map(|(id, svg)| {
            let view_box = svg
                .split("viewBox=\"")
                .nth(1)
                .and_then(|s| s.split('"').next());
            let start = svg.find('>').map_or(0, |i| i + 1);
            let end = svg.rfind("</svg>").unwrap_or(svg.len());
            format!(
                "<symbol id=\"icon.{id}\" viewBox=\"{}\">{}</symbol>",
                view_box.unwrap_or_default(),
                &svg[start..end]
            )
        })
        .collect();
    if symbols.is_empty() {
        String::new()
    } else {
        format!("<svg style=\"display: none\" aria-hidden=\"true\">{symbols}</svg>\n")
    }
}

fn wrap_page(title: &str, nav: Option<&RepoNav>, body: &str) -> String {
    let (root, title) = match nav {
        Some(nav) => (
            "../".repeat(nav.depth + 1),
            format!("{} - {title}", nav.tab.label()),
        ),
        None => (String::new(), title.to_string()),
    };
    let mut header = String::from("<header class=\"topbar\">\n<div id=\"header\">\n");
    let (main_class, footer) = match nav {
        Some(nav) => {
            let repo_root = "../".repeat(nav.depth);
            let name = escape_str(&nav.repo.name);
            header.push_str(&format!(
                "\
<a class=\"brand\" href=\"{root}index.html\">{BRAND}</a>\n\
<h1 id=\"site_path\"><a class=\"truncate\" href=\"{repo_root}index.html\" title=\"{name}\">{name}</a></h1>\n\
</div>\n\
<nav class=\"repo-tabs\" aria-label=\"repository\">\n\
"
            ));
            let files = file_links(nav, &repo_root);
            let on_file = files.iter().any(|(current, _)| *current);
            for tab in repo_tabs(nav.repo) {
                let current = match (tab == nav.tab, on_file) {
                    (false, _) => "",
                    (true, false) => " aria-current=\"page\"",
                    (true, true) => " aria-current=\"true\"",
                };
                let count = match tab {
                    Tab::Commits => Some(nav.commits),
                    Tab::Branches => Some(nav.repo.branches.len()),
                    Tab::Tags => Some(nav.repo.tags.len()),
                    Tab::Releases => Some(nav.repo.releases.len()),
                    _ => None,
                };
                let label = tab.label();
                header.push_str(&format!(
                    "<a class=\"tab\"{current} href=\"{repo_root}{}\"><span class=\"tab-label\" data-label=\"{label}\">{label}</span>",
                    tab.href(nav.git_ref)
                ));
                if let Some(count) = count {
                    header.push_str(&format!(
                        "<span class=\"tab-count\"><span class=\"vh\">, </span>{count}</span>"
                    ));
                }
                header.push_str("</a>\n");
            }
            header.push_str("</nav>\n");
            if !files.is_empty() {
                header.push_str("<nav class=\"repo-docs\" aria-label=\"documents\">\n");
                for (_, link) in &files {
                    header.push_str(link);
                    header.push('\n');
                }
                header.push_str("</nav>\n");
            }
            (
                nav.tab.main_class(),
                "<footer class=\"site-footer\">powered by <a href=\"#\">wit</a></footer>\n",
            )
        }
        None => {
            header.push_str(&format!(
                "<a class=\"brand\" aria-current=\"page\" href=\"index.html\">{BRAND}</a>\n</div>\n"
            ));
            ("action-index", "")
        }
    };
    header.push_str("</header>\n");
    let main = if main_class.is_empty() {
        "<main>".to_string()
    } else {
        format!("<main class=\"{main_class}\">")
    };
    let icons = icon_symbols(&[&header, body]);
    format!(
        "\
<!DOCTYPE html>\n\
<html lang=\"en\">\n\
<head>\n\
<meta charset=\"utf-8\">\n\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
<title>{}</title>\n\
<link rel=\"stylesheet\" href=\"{root}style.css\">\n\
<link rel=\"icon\" href=\"{root}favicon.ico\">\n\
</head>\n\
<body>\n\
{icons}\
{header}\
{main}\n\
{body}\
</main>\n\
{footer}\
</body>\n\
</html>\n\
",
        escape_str(&title)
    )
}

fn description(repo: &RepoInfo) -> (&'static str, String) {
    if repo.description.is_empty() {
        (" empty", NO_DESCRIPTION.to_string())
    } else {
        ("", escape_str(&repo.description))
    }
}

pub fn gen_index(repos: &[RepoInfo]) -> String {
    let mut sorted: Vec<&RepoInfo> = repos.iter().collect();
    sorted.sort_by_key(|repo| Reverse(repo.commit_list[0].committer_date));

    let mut body = String::from("<h1 class=\"vh\">repositories</h1>\n<ul class=\"repo-list\">\n");
    for repo in sorted {
        let name = escape_str(&repo.name);
        let dir = percent_encode(&repo.name);
        let links: String = repo_tabs(repo)
            .map(|tab| {
                format!(
                    "<a href=\"{dir}/{}\">{}</a>",
                    tab.href(&repo.head_dir),
                    tab.label()
                )
            })
            .collect();
        let (empty, description) = description(repo);
        body.push_str(&format!(
            "\
<li class=\"repo-row\">\n\
<h2 class=\"flex repo-name\"><a class=\"truncate row-link\" href=\"{dir}/index.html\">{name}</a></h2>\n\
<p class=\"repo-date\"><span class=\"repo-date-label\">last commit </span>{}</p>\n\
<p class=\"repo-desc{empty}\">{description}</p>\n\
<div class=\"repo-links\">{links}</div>\n\
</li>\n\
",
            repo.commit_list[0].committer_date.format(DATE),
        ));
    }
    body.push_str("</ul>\n");

    wrap_page("repositories", None, &body)
}

pub fn gen_summary(repo: &RepoInfo, last: &HashMap<String, &CommitInfo>) -> String {
    let nav = RepoNav {
        repo,
        git_ref: &repo.head_dir,
        tab: Tab::Summary,
        depth: 0,
        path: "",
        commits: repo.commit_list.len(),
    };
    let mut body = String::from("<div class=\"summary-layout\">\n<div class=\"summary-main\">\n");
    let head = find_ref(repo, &repo.head_dir);
    if let Some(head) = head
        && let Some(root) = root_dir(head)
    {
        body.push_str("<div class=\"summary-tree\">\n");
        body.push_str(&ref_bar(repo, head, "", "", repo.commit_list.len(), "true"));
        let tree_dir = format!("tree/{}/", repo.head_dir);
        write_listing(&mut body, head, root, "", &tree_dir, last);
        body.push_str("</div>\n");
        write_readme(&mut body, repo, head, root, "");
    }
    body.push_str("</div>\n<aside class=\"summary-side\">\n");

    let meta_row = |key: &str, class: &str, val: &str| {
        let key = if key.is_empty() {
            String::new()
        } else {
            format!("<span class=\"summary-meta-key\">{key}</span>\n")
        };
        format!(
            "<div class=\"summary-meta-row border-b\">\n{key}<span class=\"summary-meta-val{class}\">{val}</span>\n</div>\n"
        )
    };
    let dot = "&nbsp;&centerdot;&nbsp;";
    let newest = &repo.commit_list[0];
    let (empty, description) = description(repo);
    body.push_str("<div class=\"box summary-meta-box\">\n");
    body.push_str(&meta_row("", empty, &description));
    if !repo.owner.is_empty() {
        body.push_str(&meta_row("owner", " truncate", &escape_str(&repo.owner)));
    }
    let last_commit = format!(
        "{}{dot}{}",
        short_id("", &newest.id),
        newest.committer_date.format(DATE)
    );
    body.push_str(&meta_row("last commit", "", &last_commit));
    if let Some(release) = repo.releases.first() {
        let link = format!(
            "<a href=\"{}\" class=\"release-id\">{}<span class=\"truncate\">{}</span></a>{dot}{}",
            Tab::Releases.href(""),
            icon("release"),
            escape_str(&release.tag),
            release.date.format(DATE)
        );
        body.push_str(&meta_row("latest release", "", &link));
    }
    let clone_url = format!("{CLONE_URL_BASE}{}.git", repo.name);
    body.push_str(&meta_row("clone url", "", &escape_str(&clone_url)));
    body.push_str("</div>\n");

    write_briefs(&mut body, repo);
    if repo.branches.len() > 1 {
        let rows: String = repo
            .branches
            .iter()
            .take(SUMMARY_REF_COUNT)
            .map(|r| side_row(&ref_icon(r), &Tab::Tree.href(&r.dir), &r.name, &r.date))
            .collect();
        write_side_list(&mut body, Tab::Branches, &rows);
    }
    if !repo.tags.is_empty() {
        let rows: String = repo
            .tags
            .iter()
            .take(SUMMARY_REF_COUNT)
            .map(|r| side_row(&ref_icon(r), &Tab::Tree.href(&r.dir), &r.name, &r.date))
            .collect();
        write_side_list(&mut body, Tab::Tags, &rows);
    }
    if !repo.releases.is_empty() {
        let rows: String = repo
            .releases
            .iter()
            .take(SUMMARY_REF_COUNT)
            .enumerate()
            .map(|(n, release)| {
                let href = format!("{}#release-{}", Tab::Releases.href(""), n + 1);
                side_row(&icon("release"), &href, &release.tag, &release.date)
            })
            .collect();
        write_side_list(&mut body, Tab::Releases, &rows);
    }
    body.push_str("</aside>\n</div>\n");

    wrap_page(&repo.name, Some(&nav), &body)
}

fn side_row(icon: &str, href: &str, name: &str, date: &DateTime<FixedOffset>) -> String {
    format!(
        "\
<section class=\"flex items-center gap-2 px row tree-row border-b\">\n\
<div class=\"branch-name flex-1 flex items-center gap\">{icon}<a class=\"truncate row-link\" href=\"{href}\">{}</a></div>\n\
{}\
</section>\n\
",
        escape_str(name),
        date_cell(date)
    )
}

fn write_side_list(body: &mut String, tab: Tab, rows: &str) {
    let label = tab.label();
    body.push_str(&format!(
        "\
<div>\n\
<header class=\"subtitle\">\n\
<h2>{label}</h2>\n\
</header>\n\
<div class=\"box p-0 side-list\">\n\
{rows}\
<div class=\"row side-all px\">\n\
<a class=\"row-link\" href=\"{}\">all {label}</a>\n\
</div>\n\
</div>\n\
</div>\n\
",
        tab.href("")
    ));
}

pub fn gen_branches(repo: &RepoInfo, counts: &HashMap<String, usize>) -> String {
    let (head, others): (Vec<&RefInfo>, Vec<&RefInfo>) = repo
        .branches
        .iter()
        .partition(|git_ref| git_ref.dir == repo.head_dir);
    let mut body = String::new();
    write_refs(&mut body, "default branch", &head, counts);
    if !others.is_empty() {
        let heading = format!(
            "other branches <span class=\"count\">{}</span>",
            others.len()
        );
        write_refs(&mut body, &heading, &others, counts);
    }
    gen_refs(repo, Tab::Branches, &body)
}

pub fn gen_tags(repo: &RepoInfo, counts: &HashMap<String, usize>) -> String {
    let tags: Vec<&RefInfo> = repo.tags.iter().collect();
    let heading = format!("tags <span class=\"count\">{}</span>", tags.len());
    let mut body = String::new();
    write_refs(&mut body, &heading, &tags, counts);
    gen_refs(repo, Tab::Tags, &body)
}

fn gen_refs(repo: &RepoInfo, tab: Tab, body: &str) -> String {
    let nav = RepoNav {
        repo,
        git_ref: &repo.head_dir,
        tab,
        depth: 0,
        path: "",
        commits: repo.commit_list.len(),
    };
    wrap_page(&repo.name, Some(&nav), body)
}

fn write_refs(
    body: &mut String,
    heading: &str,
    refs: &[&RefInfo],
    counts: &HashMap<String, usize>,
) {
    if refs.is_empty() {
        return;
    }
    body.push_str(&format!(
        "<header class=\"subtitle\"><h2>{heading}</h2></header>\n<div class=\"box mt-0 p-0\">\n"
    ));
    for git_ref in refs {
        body.push_str(&format!(
            "\
<div class=\"flex items-center gap-2 px row tree-row border-b branch-row\">\n\
<div class=\"flex-1 flex items-center gap branch-name\">{}<a class=\"truncate row-link\" href=\"{}\">{}</a></div>\n\
<div class=\"tree-commit short-id branch-sha\">{}</div>\n\
{}\
{}\
<a class=\"branch-log\" href=\"{}\">{}</a>\n\
</div>\n\
",
            ref_icon(git_ref),
            Tab::Tree.href(&git_ref.dir),
            escape_str(&git_ref.name),
            short_id("", &git_ref.id),
            summary_cell("", &git_ref.id, &git_ref.summary),
            date_cell(&git_ref.date),
            Tab::Commits.href(&git_ref.dir),
            commit_count(counts[&git_ref.dir])
        ));
    }
    body.push_str("</div>\n");
}

pub fn gen_releases(repo: &RepoInfo, sums: &[Vec<(usize, String)>]) -> String {
    let nav = RepoNav {
        repo,
        git_ref: &repo.head_dir,
        tab: Tab::Releases,
        depth: 0,
        path: "",
        commits: repo.commit_list.len(),
    };

    let mut body = format!(
        "<header class=\"subtitle\"><h2>releases <span class=\"count\">{}</span></h2></header>\n",
        repo.releases.len()
    );
    for (n, (release, sums)) in repo.releases.iter().zip(sums).enumerate() {
        let Some(tag) = repo.tags.iter().find(|t| t.name == release.tag) else {
            continue;
        };
        body.push_str(&format!(
            "\
<section id=\"release-{}\" class=\"box mt-0 p-0 rel\">\n\
<div class=\"flex items-center gap-2 px border-b rel-head\">\n\
<div class=\"flex-1 flex items-center gap rel-name\">{}<a class=\"truncate\" href=\"{}\">{}</a></div>\n\
<div class=\"ml-auto flex items-center\">\n\
<div class=\"flex items-center tree-commit short-id\">{}</div>\n\
{}\
</div>\n\
</div>\n\
",
            n + 1,
            icon("release"),
            Tab::Tree.href(&format!("tags/{}", release.tag)),
            escape_str(&release.tag),
            short_id("", &tag.id),
            date_cell(&release.date)
        ));
        if !release.notes.is_empty() {
            let notes = release.notes.as_bytes();
            write_markdown(
                &mut body,
                "markdown-body border-b",
                repo,
                tag,
                "",
                "",
                notes,
            );
        }
        let dir = percent_encode(&release.tag);
        let (file_sums, archive_sums) = sums.split_at(release.files.len());
        for (name, sum) in release.files.iter().zip(file_sums) {
            let href = format!("releases/{dir}/{}", percent_encode(name));
            body.push_str(&release_row("file", &href, name, name, sum));
        }
        let archive = archive_name(&repo.name, &release.tag);
        for (format, sum) in ARCHIVE_FORMATS.iter().zip(archive_sums) {
            let href = format!("archive/{dir}.{format}");
            let download = format!("{archive}.{format}");
            body.push_str(&release_row(
                "file-archive",
                &href,
                &download,
                &download,
                sum,
            ));
        }
        body.push_str("</section>\n");
    }

    wrap_page(&repo.name, Some(&nav), &body)
}

fn release_row(
    icon_name: &str,
    href: &str,
    download: &str,
    label: &str,
    (bytes, sha256): &(usize, String),
) -> String {
    format!(
        "\
<div class=\"flex items-center gap-2 px row tree-row border-b rel-file\">\n\
<div class=\"flex-1 flex items-center gap rel-name\">{}<a class=\"truncate row-link\" download=\"{}\" href=\"{href}\">{}</a></div>\n\
<div class=\"truncate rel-sum\"><span class=\"rel-sum-label\">sha256</span> {sha256}</div>\n\
<div class=\"rel-size\">{}</div>\n\
</div>\n\
",
        icon(icon_name),
        escape_str(download),
        escape_str(label),
        format_bytes(*bytes)
    )
}

fn summary_cell(repo_root: &str, id: &str, summary: &str) -> String {
    let summary = escape_str(summary);
    format!(
        "<div class=\"flex flex-2 tree-commit\"><a class=\"truncate\" href=\"{repo_root}commits/{id}.html\" title=\"{summary}\">{summary}</a></div>\n"
    )
}

fn commit_cells(repo_root: &str, id: &str, summary: &str, date: &DateTime<FixedOffset>) -> String {
    format!(
        "\
{}\
<div class=\"ml-auto flex items-center\">\n\
<div class=\"flex items-center tree-commit short-id\">{}</div>\n\
{}\
</div>\n\
",
        summary_cell(repo_root, id, summary),
        short_id(repo_root, id),
        date_cell(date)
    )
}

fn short_id(repo_root: &str, id: &str) -> String {
    format!(
        "<a href=\"{repo_root}commits/{id}.html\" class=\"commit-id\">{}{}</a>",
        icon("git-commit"),
        &id[..7]
    )
}

fn date_cell(date: &DateTime<FixedOffset>) -> String {
    format!("<div class=\"tree-date\">{}</div>\n", date.format(DATE))
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

fn tree_depth(git_ref: &RefInfo, dir: &Dir) -> usize {
    1 + join_path(&git_ref.dir, &dir.path).split('/').count()
}

fn last_details(repo_root: &str, last: &HashMap<String, &CommitInfo>, path: &str) -> String {
    last.get(path).map_or_else(String::new, |commit| {
        commit_cells(
            repo_root,
            &commit.id,
            &commit.summary,
            &commit.committer_date,
        )
    })
}

pub fn gen_tree(
    repo: &RepoInfo,
    git_ref: &RefInfo,
    dir: &Dir,
    last: &HashMap<String, &CommitInfo>,
    commits: usize,
) -> String {
    let depth = tree_depth(git_ref, dir);
    let nav = RepoNav {
        repo,
        git_ref: &git_ref.dir,
        tab: Tab::Tree,
        depth,
        path: "",
        commits,
    };
    let repo_root = "../".repeat(depth);

    let mut body = ref_bar(repo, git_ref, &dir.path, &repo_root, commits, "page");
    write_listing(&mut body, git_ref, dir, &repo_root, "", last);
    write_readme(&mut body, repo, git_ref, dir, &repo_root);

    let title = format!("{}/{}", repo.name, git_ref.name);
    wrap_page(&title, Some(&nav), &body)
}

pub fn gen_blob(
    repo: &RepoInfo,
    git_ref: &RefInfo,
    dir: &Dir,
    entry: &TreeEntry,
    content: &[u8],
    last: &HashMap<String, &CommitInfo>,
    commits: usize,
) -> String {
    let depth = tree_depth(git_ref, dir);
    let path = join_path(&dir.path, &entry.name);
    let nav = RepoNav {
        repo,
        git_ref: &git_ref.dir,
        tab: Tab::Tree,
        depth,
        path: &path,
        commits,
    };
    let repo_root = "../".repeat(depth);
    let details = last_details(&repo_root, last, &path);

    let mut body = ref_bar(repo, git_ref, &path, &repo_root, commits, "page");
    if !details.is_empty() {
        body.push_str(&format!(
            "<div class=\"box joined flex items-center gap last-commit\">\n{details}</div>\n"
        ));
    }
    let views = has_source_view(entry);
    write_file_head(&mut body, entry, content, views);
    let text = entry.lines.is_some();
    match image_type(&entry.name) {
        Some(mime) => body.push_str(&format!(
            "<div class=\"box\"><img src=\"{}\" alt=\"{}\"></div>\n",
            data_uri(mime, content),
            escape_str(&entry.name)
        )),
        None if text && is_markdown(&entry.name) => write_markdown(
            &mut body,
            MARKDOWN_BOX,
            repo,
            git_ref,
            &dir.path,
            &repo_root,
            content,
        ),
        _ if text => write_code(&mut body, content),
        _ => body.push_str("binary file, cannot display\n"),
    }
    if views {
        write_code(&mut body, content);
    }

    let title = format!("{}/{}/{path}", repo.name, git_ref.name);
    wrap_page(&title, Some(&nav), &body)
}

fn write_file_head(body: &mut String, entry: &TreeEntry, content: &[u8], views: bool) {
    if views {
        body.push_str(
            "\
<input type=\"radio\" class=\"vh\" name=\"view\" id=\"view-source\">\n\
<input type=\"radio\" class=\"vh\" name=\"view\" id=\"view-rendered\" checked>\n\
<div class=\"buttons flex view-switch\"><label for=\"view-source\">source</label>\n\
<label for=\"view-rendered\">rendered</label></div>\n\
",
        );
    }
    let lines = entry
        .lines
        .map(|n| {
            let s = if n == 1 { "" } else { "s" };
            format!("<span>{n} line{s}</span>\n")
        })
        .unwrap_or_default();
    let bytes = entry.bytes.map(format_bytes).unwrap_or_default();
    let pixels = image_type(&entry.name)
        .and_then(|mime| image_size(mime, content))
        .map(|(width, height)| format!("<span>{width}x{height}px</span>\n"))
        .unwrap_or_default();
    body.push_str(&format!(
        "<div class=\"flex items-center file-head\">\n<div class=\"flex gap file-size\">\n{lines}<span>{bytes}</span>\n{pixels}</div>\n</div>\n"
    ));
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
        commits: all.len(),
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
    } else {
        commit_count(total)
    };

    let mut body = format!(
        "\
<div class=\"log\">\n\
<div class=\"box sticky flex items-center gap flex-wrap m-0\">\n\
{}\
<div>{count}</div>\n\
</div>\n\
",
        ref_menu(repo, git_ref, &repo_root, Tab::Commits, "", "page")
    );
    let mut day = String::new();
    let mut menus = 1;
    for commit in commits {
        let date = commit.committer_date.format(DATE).to_string();
        let joined = if date == day {
            " joined"
        } else {
            if !day.is_empty() {
                body.push_str("</div>\n");
            }
            body.push_str(&format!(
                "<div class=\"log-day\">\n<div class=\"text-xs log-date\">{date}</div>\n"
            ));
            ""
        };
        let labels = ref_labels(repo, &commit.id, &repo_root, &mut menus);
        write_commit_row(
            &mut body,
            commit,
            Some(&stats[&commit.id]),
            &labels,
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
    if diff.files.is_empty() {
        return "no changes".to_string();
    }
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
    commits: usize,
) -> String {
    let nav = RepoNav {
        repo,
        git_ref: &git_ref.dir,
        tab: Tab::Commits,
        depth: 1,
        path: "",
        commits,
    };
    let short = &commit.id[..7];
    let parents: Vec<String> = diff
        .parents
        .iter()
        .map(|id| format!("<a href=\"{id}.html\">{}</a>", &id[..7]))
        .collect();
    let parents = match parents.len() {
        0 => String::new(),
        1 => format!(
            "<div class=\"commit-parent\"><span>parent</span><span>{}</span></div>\n",
            parents[0]
        ),
        _ => format!(
            "<div class=\"commit-parent\"><span>parents</span><span>{}</span></div>\n",
            parents.join(", ")
        ),
    };
    let format = "%d-%m-%Y&nbsp;%H:%M";
    let rest = commit
        .message
        .trim()
        .split_once("\n\n")
        .map_or("", |(_, rest)| rest.trim());
    let message_body = if rest.is_empty() {
        String::new()
    } else {
        format!(
            "<div class=\"white-space-bs commit-body\">{}</div>\n",
            escape_str(rest)
        )
    };

    let mut body = format!(
        "\
<div class=\"box commit-head\">\n\
<div class=\"flex gap commit-msg\">\n\
<div class=\"flex-1\">\n\
<div class=\"commit-title\"><span class=\"commit-id\">{}{short}</span>{}</div>\n\
{message_body}\
</div>\n\
<div>{}</div>\n\
</div>\n\
<div class=\"commit-meta\">\n\
<div>author</div>\n\
<div>{} &lt;{}&gt; on {}</div>\n\
<div>committer</div>\n\
<div>{} &lt;{}&gt; on {}</div>\n\
{parents}\
</div>\n\
</div>\n\
",
        icon("git-commit"),
        escape_str(&commit.summary),
        diff_stat(diff),
        escape_str(&commit.author_name),
        escape_str(&commit.author_email),
        format_zoned(&commit.author_date, format),
        escape_str(&commit.committer_name),
        escape_str(&commit.committer_email),
        format_zoned(&commit.committer_date, format)
    );
    if !diff.files.is_empty() {
        body.push_str("<div class=\"box joined commit-files\">\n");
        for (n, file) in diff.files.iter().enumerate() {
            body.push_str(&format!(
                "<div class=\"flex items-center gap\">{}<a href=\"#diff-{n}\">{}</a><div class=\"ml-auto diff-stat\">{}</div></div>\n",
                status_icon(file.status),
                change_path(file),
                file_stat(file)
            ));
        }
        body.push_str("</div>\n");
    }
    for (n, file) in diff.files.iter().enumerate() {
        body.push_str(&format!(
            "\
<div>\n\
<div id=\"diff-{n}\" class=\"box sticky flex items-center diff-head\">\n\
<div class=\"flex-1 flex items-center gap\">{}<a href=\"#diff-{n}\">{}</a></div>\n\
<div class=\"diff-stat\">{}</div>\n\
</div>\n\
",
            status_icon(file.status),
            change_path(file),
            file_stat(file)
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
    let (name, class, title) = match status {
        'A' => ("diff-added", " color-green", "added"),
        'D' => ("diff-removed", " color-red", "deleted"),
        'R' => ("diff-renamed", "", "renamed"),
        'T' => ("diff-modified", " color-orange", "type changed"),
        _ => ("diff-modified", " color-orange", "modified"),
    };
    format!(
        "<span class=\"flex{class}\" title=\"{title}\" role=\"img\" aria-label=\"{title}\">{}</span>",
        icon(name)
    )
}

fn file_stat(file: &FileChange) -> String {
    format!(
        "<span class=\"color-green\">+{}</span>, <span class=\"color-red\">-{}</span>",
        file.added, file.removed
    )
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
    let number = |body: &mut String, side: &str, num: Option<u32>| {
        let _ = match num {
            Some(num) => write!(
                body,
                "<a class=\"ln\" id=\"d{n}-{side}{num}\" href=\"#d{n}-{side}{num}\" tabindex=\"-1\">{num:>width$}</a>"
            ),
            None => write!(body, "<span class=\"ln\">{:width$}</span>", ""),
        };
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
        let _ = write!(body, "<span class=\"line{class}\">");
        number(body, "L", old);
        number(body, "R", new);
        body.push_str("<span class=\"cl\">");
        push_line(body, &text);
        body.push_str("</span></span>");
    }
    body.push_str("</code></pre>\n");
}

fn commits_pager(page: usize, pages: usize) -> String {
    let arrow = |n: usize, label: &str, name: &str| {
        format!(
            "<a href=\"{}\" title=\"{label}\" aria-label=\"{label}\">{}</a>\n",
            commits_page(n),
            icon(name)
        )
    };
    let mut pager =
        String::from("<nav class=\"buttons flex flex-wrap pager\" aria-label=\"pages\">\n");
    if page > 1 {
        pager.push_str(&arrow(page - 1, "previous page", "chevron-left"));
    }
    let mut shown = 0;
    for n in 1..=pages {
        if n != 1 && n != pages && n.abs_diff(page) > 1 {
            continue;
        }
        if n > shown + 1 {
            pager.push_str("<span class=\"pager-gap\">&hellip;</span>\n");
        }
        if n == page {
            pager.push_str(&format!("<span aria-current=\"page\">{n}</span>\n"));
        } else {
            pager.push_str(&format!("<a href=\"{}\">{n}</a>\n", commits_page(n)));
        }
        shown = n;
    }
    if page < pages {
        pager.push_str(&arrow(page + 1, "next page", "chevron-right"));
    }
    pager.push_str("</nav>\n");
    pager
}

fn has_source_view(entry: &TreeEntry) -> bool {
    entry.lines.is_some() && (image_type(&entry.name).is_some() || is_markdown(&entry.name))
}

fn blob_href(git_ref: &RefInfo, path: &str) -> String {
    percent_encode(&format!("blob/{}/{path}.html", git_ref.dir))
}

pub fn path_href(git_ref: &RefInfo, repo_root: &str, path: &str) -> Option<String> {
    let path = follow_links(git_ref, path)?;
    if git_ref.dirs.contains_key(&path) {
        let href = Tab::Tree.href(&join_path(&git_ref.dir, &path));
        return Some(format!("{repo_root}{href}"));
    }
    let entry = find_entry(git_ref, &path)?;
    if entry.blob.is_some() {
        return Some(format!("{repo_root}{}", blob_href(git_ref, &path)));
    }
    entry.commit.as_ref()?;
    git_ref
        .submodules
        .iter()
        .find(|(module, url)| *module == path && url.starts_with("https://"))
        .map(|(_, url)| url.clone())
}

pub fn follow_links(git_ref: &RefInfo, path: &str) -> Option<String> {
    let mut path = path.to_string();
    let mut dir_only = false;
    for _ in 0..=MAX_LINKS {
        let dir = path.rsplit_once('/').map_or("", |(dir, _)| dir);
        let link = find_entry(git_ref, &path).and_then(|entry| entry.link.as_deref());
        match link {
            Some(target) if target.is_empty() || target.starts_with('/') => return None,
            Some(target) => {
                dir_only |= target.ends_with('/');
                path = resolve_path(dir, target)?;
            }
            None => {
                let is_dir = git_ref.dirs.contains_key(&path);
                return (is_dir || !dir_only).then_some(path);
            }
        }
    }
    None
}

pub fn resolve_path(base: &str, path: &str) -> Option<String> {
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
    git_ref.dirs.get(dir)?.entry(name)
}

pub fn find_file<'a>(git_ref: &'a RefInfo, path: &str) -> Option<&'a TreeEntry> {
    find_entry(git_ref, path).filter(|entry| entry.blob.is_some())
}

pub fn data_uri(mime: &str, content: &[u8]) -> String {
    format!("data:{mime};base64,{}", base64(content))
}

pub fn image_type(name: &str) -> Option<&'static str> {
    let (_, ext) = name.rsplit_once('.')?;
    IMAGE_TYPES
        .iter()
        .find(|(known, _)| ext.eq_ignore_ascii_case(known))
        .map(|(_, mime)| *mime)
}

fn image_size(mime: &str, content: &[u8]) -> Option<(usize, usize)> {
    if mime != "image/svg+xml" {
        let size = imagesize::blob_size(content).ok()?;
        return Some((size.width, size.height));
    }
    let text = String::from_utf8_lossy(content);
    let start = text.find("<svg")?;
    let tag = &text[start..start + text[start..].find('>')?];
    let number = |value: &str| value.trim().trim_end_matches("px").parse::<f64>().ok();
    if let (Some(width), Some(height)) = (
        attr(tag, "width").and_then(number),
        attr(tag, "height").and_then(number),
    ) {
        return Some((width.round() as usize, height.round() as usize));
    }
    let view_box: Vec<f64> = attr(tag, "viewBox")?
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|part| !part.is_empty())
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    match view_box[..] {
        [_, _, width, height] => Some((width.round() as usize, height.round() as usize)),
        _ => None,
    }
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let mut from = 0;
    while let Some(i) = tag[from..].find(name).map(|i| from + i) {
        from = i + name.len();
        if !tag[..i].ends_with(|c: char| c.is_ascii_whitespace()) {
            continue;
        }
        let Some(value) = tag[from..].strip_prefix('=') else {
            continue;
        };
        let quote = value.chars().next().filter(|c| matches!(c, '"' | '\''))?;
        return value[1..].find(quote).map(|end| &value[1..1 + end]);
    }
    None
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

fn ref_bar(
    repo: &RepoInfo,
    git_ref: &RefInfo,
    path: &str,
    repo_root: &str,
    commits: usize,
    current: &str,
) -> String {
    let mut bar =
        String::from("<div class=\"box sticky flex items-center gap flex-wrap ref-bar\">\n");
    bar.push_str(&ref_menu(
        repo,
        git_ref,
        repo_root,
        Tab::Tree,
        path,
        current,
    ));
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
            bar.push_str(&format!(
                "<span class=\"font-bold\">{}</span>\n",
                escape_str(part)
            ));
        } else {
            bar.push_str(&format!(
                "<a href=\"{repo_root}{}\">{}</a>\n",
                Tab::Tree.href(tree_path),
                escape_str(part)
            ));
        }
    }
    bar.push_str(&format!(
        "<a class=\"ref-bar-commits\" href=\"{repo_root}{}\">{}</a>\n</div>\n",
        Tab::Commits.href(&git_ref.dir),
        commit_count(commits)
    ));
    bar
}

fn commit_count(commits: usize) -> String {
    let s = if commits == 1 { "" } else { "s" };
    format!("<span class=\"font-bold\">{commits}</span> commit{s}")
}

fn menu_target(git_ref: &RefInfo, path: &str) -> (String, bool) {
    if find_file(git_ref, path).is_some() {
        return (blob_href(git_ref, path), true);
    }
    let mut dir = path;
    while !dir.is_empty() && !git_ref.dirs.contains_key(dir) {
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
    current: &str,
) -> String {
    let name = escape_str(&git_ref.name);
    let mut menu = format!(
        "\
<div class=\"ref-menu\">\n\
<button type=\"button\" class=\"ref-btn\" popovertarget=\"ref-menu\" aria-label=\"{name}, switch branch or tag\">{}<span class=\"truncate\">{name}</span>{}{}</button>\n\
<div class=\"ref-menu-list\" id=\"ref-menu\" popover>\n\
",
        ref_icon(git_ref),
        icon("chevron-down"),
        icon("chevron-up")
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
                    format!(" aria-current=\"{current}\"")
                } else {
                    String::new()
                };
                let (href, found) = match tab {
                    Tab::Tree => menu_target(r, path),
                    _ => (tab.href(&r.dir), true),
                };
                let kind = if r.is_tag() {
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
                    "<a class=\"flex items-center gap{missing}\"{title}{active} href=\"{repo_root}{href}\">{}{}</a>\n",
                    ref_icon(r),
                    escape_str(&r.name)
                )
            })
            .collect();
        format!("<div class=\"{class} buttons{long}\">\n{entries}</div>\n")
    };
    if !repo.tags.is_empty() {
        let (branches, tags) = if git_ref.is_tag() {
            ("", " checked")
        } else {
            (" checked", "")
        };
        menu.push_str(&format!(
            "\
<input type=\"radio\" class=\"vh\" name=\"ref-kind\" id=\"ref-branches\"{branches}>\n\
<input type=\"radio\" class=\"vh\" name=\"ref-kind\" id=\"ref-tags\"{tags}>\n\
<div class=\"ref-kinds buttons flex\"><label for=\"ref-branches\">branches</label><label for=\"ref-tags\">tags</label></div>\n\
{}\
",
            list("ref-tags", &repo.tags)
        ));
    }
    menu.push_str(&list("ref-branches", &repo.branches));
    menu.push_str("</div>\n</div>\n");
    menu
}

fn write_code(body: &mut String, content: &[u8]) {
    let text = escape_str(&String::from_utf8_lossy(content));
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let width = lines.len().to_string().len();
    let _ = write!(
        body,
        "<pre class=\"chroma\" style=\"--ln: {width}ch\"><code>"
    );
    for (i, line) in lines.iter().enumerate() {
        let n = i + 1;
        let (text, newline) = match line.strip_suffix('\n') {
            Some(text) => (text, "\n"),
            None => (*line, ""),
        };
        let _ = write!(
            body,
            "<a class=\"ln\" id=\"L{n}\" href=\"#L{n}\" tabindex=\"-1\">{n:>width$}</a><span class=\"cl\">"
        );
        push_line(body, text);
        body.push_str("</span>");
        body.push_str(newline);
    }
    body.push_str("</code></pre>\n");
}

fn push_line(body: &mut String, text: &str) {
    for (i, piece) in text.split('\r').enumerate() {
        if i > 0 && !piece.starts_with('\n') {
            body.push('\n');
        }
        body.push_str(piece);
    }
}

fn write_listing(
    body: &mut String,
    git_ref: &RefInfo,
    dir: &Dir,
    repo_root: &str,
    tree_dir: &str,
    last: &HashMap<String, &CommitInfo>,
) {
    body.push_str("<div class=\"box p-0 joined\">\n");
    let head = if dir.path.is_empty() {
        commit_cells(repo_root, &git_ref.id, &git_ref.summary, &git_ref.date)
    } else {
        last_details(repo_root, last, &dir.path)
    };
    if !head.is_empty() {
        body.push_str(&format!(
            "<div class=\"flex items-center gap px border-b last-commit\">\n{head}</div>\n"
        ));
    }
    if !dir.path.is_empty() {
        body.push_str(&format!(
            "\
<div class=\"flex items-center gap-2 px row tree-row border-b up-row\">\n\
<div class=\"flex-1 tree-path flex items-center gap\">\n\
{}\n\
<a class=\"truncate row-link\" href=\"../index.html\" title=\"parent folder\">..</a>\n\
</div>\n\
</div>\n\
",
            icon("file-directory")
        ));
    }
    for entry in &dir.entries {
        let name = &entry.name;
        let path = join_path(&dir.path, name);
        let link = |href: String, text: String| {
            format!(
                "<a class=\"truncate row-link\" href=\"{}\">{text}</a>",
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
            None => {
                "<div class=\"flex flex-2 tree-commit\"></div>\n<div class=\"tree-date\"></div>\n"
                    .to_string()
            }
        };
        let (icon_name, name) = if entry.is_dir {
            let href = percent_encode(&format!("{tree_dir}{name}/index.html"));
            ("file-directory", link(href, escape_str(name)))
        } else if let Some(target) = &entry.link {
            let text = format!("{} &rarr; {}", escape_str(name), escape_str(target));
            ("file-symlink-file", leads(text))
        } else if let Some(commit) = &entry.commit {
            let text = format!("{} @ {}", escape_str(name), &commit[..7]);
            ("file-submodule", leads(text))
        } else {
            ("file", leads(escape_str(name)))
        };
        body.push_str(&format!(
            "\
<div class=\"flex items-center gap-2 px row tree-row border-b\">\n\
<div class=\"flex-1 tree-path flex items-center gap\">\n\
{}\n\
{name}\n\
</div>\n\
{last_change}\
</div>\n\
",
            icon(icon_name)
        ));
    }
    body.push_str("</div>\n");
}

fn ref_labels(repo: &RepoInfo, id: &str, repo_root: &str, menus: &mut usize) -> String {
    let mut refs: Vec<&RefInfo> = repo
        .branches
        .iter()
        .chain(&repo.tags)
        .filter(|git_ref| git_ref.id == id)
        .collect();
    if refs.is_empty() {
        return String::new();
    }
    refs.sort_by(|a, b| RefInfo::newest_first(a, b));
    let link = |git_ref: &&RefInfo| {
        (
            format!("{repo_root}{}", Tab::Tree.href(&git_ref.dir)),
            format!("{}{}", ref_icon(git_ref), escape_str(&git_ref.name)),
        )
    };
    let chips: String = refs
        .iter()
        .take(REF_CHIPS)
        .map(|git_ref| {
            let (href, text) = link(git_ref);
            format!("<a class=\"chip flex\" href=\"{href}\"><code>{text}</code></a>\n")
        })
        .collect();
    let entries: String = refs
        .iter()
        .map(|git_ref| {
            let (href, text) = link(git_ref);
            format!("<a class=\"flex items-center gap\" href=\"{href}\">{text}</a>\n")
        })
        .collect();
    *menus += 1;
    let n = *menus;
    let s = if refs.len() == 1 { "" } else { "s" };
    format!(
        "\
<div class=\"chips text-xs\">\n{chips}</div>\n\
<div class=\"ref-menu ref-more text-xs\">\n\
<button type=\"button\" class=\"ref-btn\" popovertarget=\"ref-more-{n}\">{} ref{s}{}{}</button>\n\
<div class=\"ref-menu-list buttons\" id=\"ref-more-{n}\" popover>\n{entries}</div>\n\
</div>\n\
",
        refs.len(),
        icon("chevron-down"),
        icon("chevron-up")
    )
}

fn write_commit_row(
    body: &mut String,
    commit: &CommitInfo,
    stat: Option<&str>,
    labels: &str,
    repo_root: &str,
    joined: &str,
    date: &str,
) {
    let href = format!("{repo_root}commits/{}.html", commit.id);
    let stat = stat
        .map(|stat| format!("<div class=\"commit-stat flex-1\"><div>{stat}</div></div>\n"))
        .unwrap_or_default();
    body.push_str(&format!(
        "\
<div class=\"box m-0 row commit-row{joined}\">\n\
<div class=\"flex flex-wrap items-center gap text-xs\">\n\
<a href=\"{href}\" class=\"commit-id row-link\">{}{}</a>\n\
<div class=\"flex items-center commit-by\">\n\
<span class=\"truncate\">{}</span>\n\
<span>&nbsp;&centerdot;&nbsp;</span>\n\
<span>{date}</span>\n\
</div>\n\
{stat}\
</div>\n\
<div class=\"flex items-center commit-line\">\n\
<div class=\"flex commit-summary\"><a class=\"truncate row-link\" href=\"{href}\">{}</a></div>\n\
{labels}\
</div>\n\
</div>\n\
",
        icon("git-commit"),
        &commit.id[..7],
        escape_str(&commit.author_name),
        escape_str(&commit.summary)
    ));
}

fn write_briefs(body: &mut String, repo: &RepoInfo) {
    body.push_str("<div>\n<header class=\"subtitle\">\n<h2>commits</h2>\n</header>\n");
    let mut menus = 1;
    for (n, commit) in repo.commit_list.iter().take(BRIEF_COUNT).enumerate() {
        let joined = if n == 0 { "" } else { " joined" };
        let labels = ref_labels(repo, &commit.id, "", &mut menus);
        let date = commit.committer_date.format(DATE).to_string();
        write_commit_row(body, commit, None, &labels, "", joined, &date);
    }
    body.push_str(&format!(
        "\
<div class=\"box m-0 joined row side-all px\">\n\
<a class=\"row-link\" href=\"{}\">all commits</a>\n\
</div>\n\
</div>\n\
",
        Tab::Commits.href(&repo.head_dir)
    ));
}
