use crate::repo_info::{Folder, RefInfo, RepoInfo, TreeEntry};

use std::cmp::Reverse;

const BRAND: &str = "wit";

const CLONE_URL_BASE: &str = "https://git.paramogi.world/";

const BRIEF_COUNT: usize = 12;

const SUMMARY_REF_COUNT: usize = 3;

const BYTE_UNITS: [&str; 5] = ["B", "kB", "MB", "GB", "TB"];

const REPO_FILES: [(&str, [&str; 2]); 2] = [
    ("README", ["README.md", "README"]),
    ("LICENSE", ["LICENSE", "COPYING"]),
];

pub const STYLE: &str = include_str!("style.css");

const FILE_ICON: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" fill=\"currentColor\" height=\"16\" width=\"16\" viewBox=\"0 0 384 512\"><path d=\"M320 464c8.8 0 16-7.2 16-16V160H256c-17.7 0-32-14.3-32-32V48H64c-8.8 0-16 7.2-16 16V448c0 8.8 7.2 16 16 16H320zM0 64C0 28.7 28.7 0 64 0H229.5c17 0 33.3 6.7 45.3 18.7l90.5 90.5c12 12 18.7 28.3 18.7 45.3V448c0 35.3-28.7 64-64 64H64c-35.3 0-64-28.7-64-64V64z\"/></svg>";

const FOLDER_ICON: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" fill=\"currentColor\" height=\"16\" width=\"16\" viewBox=\"0 0 512 512\"><path d=\"M0 96C0 60.7 28.7 32 64 32H196.1c19.1 0 37.4 7.6 50.9 21.1L289.9 96H448c35.3 0 64 28.7 64 64V416c0 35.3-28.7 64-64 64H64c-35.3 0-64-28.7-64-64V96zM64 80c-8.8 0-16 7.2-16 16V416c0 8.8 7.2 16 16 16H448c8.8 0 16-7.2 16-16V160c0-8.8-7.2-16-16-16H286.6c-10.6 0-20.8-4.2-28.3-11.7L213.1 87c-4.5-4.5-10.6-7-17-7H64z\"/></svg>";

#[derive(Clone, Copy, PartialEq)]
enum Tab {
    Summary,
    Tree,
    Log,
    Branches,
    Tags,
}

const TABS: [Tab; 5] = [Tab::Summary, Tab::Tree, Tab::Log, Tab::Branches, Tab::Tags];

impl Tab {
    fn label(self) -> &'static str {
        match self {
            Tab::Summary => "summary",
            Tab::Tree => "tree",
            Tab::Log => "log",
            Tab::Branches => "branches",
            Tab::Tags => "tags",
        }
    }

    fn main_class(self) -> &'static str {
        match self {
            Tab::Summary => "action-readme",
            Tab::Tree => "action-code",
            Tab::Log => "action-commits",
            Tab::Branches => "action-branches",
            Tab::Tags => "action-tags",
        }
    }

    fn href(self, git_ref: &str) -> String {
        match self {
            Tab::Summary => "index.html".to_string(),
            Tab::Tree => format!("tree/{}/index.html", percent_encode(git_ref)),
            Tab::Log => format!("logs/{}/index.html", percent_encode(git_ref)),
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
    trail: Vec<(String, String)>,
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
        return format!("{bytes} B");
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

fn file_links(nav: &RepoNav, repo_root: &str) -> Vec<String> {
    let Some(root) = nav
        .repo
        .branches
        .iter()
        .chain(&nav.repo.tags)
        .find(|git_ref| git_ref.dir == nav.git_ref)
        .and_then(|git_ref| git_ref.folders.iter().find(|f| f.path.is_empty()))
    else {
        return Vec::new();
    };
    REPO_FILES
        .iter()
        .filter_map(|(label, names)| {
            let name = names
                .iter()
                .find(|name| root.entries.iter().any(|e| !e.is_dir && e.name == **name))?;
            let href = percent_encode(&format!("blob/{}/{name}.html", nav.git_ref));
            Some(format!("<a href=\"{repo_root}{href}\">{label}</a>"))
        })
        .collect()
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
            let mut crumbs = vec![
                (nav.repo.name.clone(), "index.html".to_string()),
                (nav.tab.label().to_string(), nav.tab.href(nav.git_ref)),
            ];
            crumbs.extend(nav.trail.iter().cloned());
            let last = crumbs.len() - 1;
            let crumbs: Vec<String> = crumbs
                .iter()
                .enumerate()
                .map(|(i, (text, href))| {
                    if i == last {
                        format!("<a class=\"btn-active\" href=\"\">{}</a>", escape_str(text))
                    } else {
                        format!("<a href=\"{repo_root}{href}\">{}</a>", escape_str(text))
                    }
                })
                .collect();
            page.push_str(&format!(
                "\
<nav id=\"site_path\"><div id=\"site_link\">{}</div></nav>\n\
<div class=\"navs_wrapper flex justify-between\"><nav class=\"navs\">\n\
",
                crumbs.join(" / ")
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
            page.push_str(&format!("{}\n</nav>\n", links.join(" |\n")));
            let files = file_links(nav, &repo_root);
            if !files.is_empty() {
                page.push_str(&format!(
                    "<nav class=\"navs flex gap-2\">\n{}\n</nav>\n",
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
                let label = if tab == Tab::Log {
                    "commits"
                } else {
                    tab.label()
                };
                format!("<a href=\"{dir}/{}\">{label}</a>", tab.href(&repo.head_dir))
            })
            .collect();
        body.push_str(&format!(
            "\
<div class=\"box home-card\">\n\
<div>\n\
<p class=\"repo-name\"><a href=\"{dir}/index.html\">{name}</a></p>\n\
<div class=\"repo-desc\">{}</div>\n\
</div>\n\
<nav class=\"repo-links\">{}</nav>\n\
</div>\n\
",
            escape_str(&repo.description),
            links.join(" | ")
        ));
    }
    body.push_str("</div>\n");

    wrap_page("repositories", None, &body)
}

pub fn gen_summary(repo: &RepoInfo) -> String {
    let nav = RepoNav {
        repo,
        git_ref: &repo.head_dir,
        tab: Tab::Summary,
        depth: 0,
        trail: Vec::new(),
    };
    let last_change = repo
        .commit_list
        .first()
        .map(|commit| commit.committer_date.format("%Y-%m-%d").to_string());
    let clone_url = format!("{CLONE_URL_BASE}{}.git", repo.name);
    let mut rows: Vec<(&str, &str)> =
        vec![("name", &repo.name), ("description", &repo.description)];
    if !repo.owner.is_empty() {
        rows.push(("owner", &repo.owner));
    }
    if let Some(date) = &last_change {
        rows.push(("last change", date));
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
    write_briefs(&mut body, repo);
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
        trail: Vec::new(),
    };

    let mut body = String::from("<div class=\"box mt-0 p-0\">\n");
    for git_ref in refs {
        body.push_str(&format!(
            "\
<div class=\"flex justify-between items-center gap-2 py px tree-row border-b\">\n\
<div><a href=\"{}\">{}</a></div>\n\
</div>\n\
",
            Tab::Tree.href(&git_ref.dir),
            escape_str(&git_ref.name)
        ));
    }
    body.push_str("</div>\n");

    wrap_page(&repo.name, Some(&nav), &body)
}

pub fn gen_tree(repo: &RepoInfo, git_ref: &RefInfo, folder: &Folder) -> String {
    let mut tree_path = git_ref.dir.clone();
    let mut trail = vec![(git_ref.name.clone(), Tab::Tree.href(&tree_path))];
    for part in folder.path.split('/').filter(|p| !p.is_empty()) {
        tree_path = format!("{tree_path}/{part}");
        trail.push((part.to_string(), Tab::Tree.href(&tree_path)));
    }
    let depth = 1 + tree_path.split('/').count();
    let nav = RepoNav {
        repo,
        git_ref: &git_ref.dir,
        tab: Tab::Tree,
        depth,
        trail,
    };
    let blob_dir = format!("{}blob/{tree_path}/", "../".repeat(depth));

    let mut body = String::from("<div class=\"box p-0\">\n");
    for TreeEntry {
        name,
        is_dir,
        lines,
        bytes,
    } in &folder.entries
    {
        let (icon, href) = if *is_dir {
            (FOLDER_ICON, percent_encode(&format!("{name}/index.html")))
        } else {
            (FILE_ICON, percent_encode(&format!("{blob_dir}{name}.html")))
        };
        let lines = lines.map(|n| format!("{n} L")).unwrap_or_default();
        let bytes = bytes.map(format_bytes).unwrap_or_default();
        body.push_str(&format!(
            "\
<div class=\"flex justify-between items-center gap-2 py px tree-row border-b\">\n\
<div class=\"flex-1 tree-path flex items-center gap\">\n\
{icon}\n\
<a href=\"{href}\">{}</a>\n\
</div>\n\
<div class=\"flex items-center gap\">\n\
<div class=\"tree-size\">{lines}</div>\n\
<div class=\"tree-size\">{bytes}</div>\n\
</div>\n\
</div>\n\
",
            escape_str(name)
        ));
    }
    body.push_str("</div>\n");

    let title = format!("{}/{}", repo.name, git_ref.name);
    wrap_page(&title, Some(&nav), &body)
}

fn write_briefs(body: &mut String, repo: &RepoInfo) {
    body.push_str(
        "\
<div class=\"summary-briefs\">\n\
<header class=\"subtitle\">\n\
<h2>commit briefs</h2>\n\
</header>\n\
<div id=\"briefs_content\">\n\
<div class=\"box summary-commit-box\">\n\
",
    );
    for commit in repo.commit_list.iter().take(BRIEF_COUNT) {
        let id = &commit.id;
        let labels: String = repo
            .branches
            .iter()
            .chain(&repo.tags)
            .filter(|git_ref| git_ref.id == *id)
            .map(|git_ref| {
                format!(
                    "<code><a href=\"{}\">{}</a></code>\n",
                    Tab::Tree.href(&git_ref.dir),
                    escape_str(&git_ref.name)
                )
            })
            .collect();
        body.push_str(&format!(
            "\
<div class=\"summary-commit-row border-b\">\n\
<div class=\"flex items-center gap mb text-sm\">\n\
<a href=\"commits/{id}.html\" class=\"mono\">{}</a>\n\
<div class=\"flex items-center gap-xs\">\n\
<span>{}</span>\n\
<span>&nbsp;&centerdot;&nbsp;</span>\n\
<span>{}</span>\n\
</div>\n\
{labels}\
</div>\n\
<div class=\"summary-commit-msg\">\n\
<a href=\"commits/{id}.html\">{}</a>\n\
</div>\n\
</div>\n\
",
            &id[..7],
            escape_str(&commit.author_name),
            commit.author_date.format("%Y-%m-%d %H:%M:%S %z"),
            escape_str(&commit.summary)
        ));
    }
    body.push_str("</div>\n</div>\n</div>\n");
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
<section class=\"{wrapper} border-b py px\">\n\
<div class=\"{inner}\"><a href=\"{}\">{}</a></div>\n\
</section>\n\
",
            Tab::Tree.href(&git_ref.dir),
            escape_str(&git_ref.name)
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
