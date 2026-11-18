use crate::repo_info::{RefInfo, RepoInfo};

use std::cmp::Reverse;

const BRAND: &str = "wit";

const CLONE_URL_BASE: &str = "https://git.paramogi.world/";

const BRIEF_COUNT: usize = 12;

const SUMMARY_REF_COUNT: usize = 3;

pub const STYLE: &str = include_str!("style.css");

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
            Tab::Tree => format!("tree/{git_ref}/index.html"),
            Tab::Log => format!("logs/{git_ref}/index.html"),
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

fn wrap_page(title: &str, nav: Option<&RepoNav>, body: &str) -> String {
    let root = match nav {
        Some(nav) => "../".repeat(nav.depth + 1),
        None => String::new(),
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
        escape_str(title)
    );

    let main_class = match nav {
        Some(nav) => {
            let repo_root = "../".repeat(nav.depth);
            page.push_str(&format!(
                "\
<nav id=\"site_path\"><div id=\"site_link\"><a href=\"{repo_root}index.html\">{}</a> / {}</div></nav>\n\
<div class=\"navs_wrapper\"><nav class=\"navs\">\n\
",
                escape_str(&nav.repo.name),
                nav.tab.label()
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
                        escape_str(&tab.href(nav.git_ref)),
                        tab.label()
                    )
                })
                .collect();
            page.push_str(&format!("{}\n</nav></div>\n", links.join(" |\n")));
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
    sorted.sort_by_key(|repo| Reverse(repo.commit_list.first().map(|c| c.author_date)));

    let mut body = String::from("<div class=\"home-grid\">\n");
    for repo in sorted {
        let name = escape_str(&repo.name);
        let links: Vec<String> = repo_tabs(repo)
            .map(|tab| {
                let label = if tab == Tab::Log {
                    "commits"
                } else {
                    tab.label()
                };
                format!(
                    "<a href=\"{name}/{}\">{label}</a>",
                    escape_str(&tab.href(&repo.head))
                )
            })
            .collect();
        body.push_str(&format!(
            "\
<div class=\"box home-card\">\n\
<div>\n\
<p class=\"repo-name\"><a href=\"{name}/index.html\">{name}</a></p>\n\
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

    wrap_page("index", None, &body)
}

pub fn gen_summary(repo: &RepoInfo) -> String {
    let nav = RepoNav {
        repo,
        git_ref: &repo.head,
        tab: Tab::Summary,
        depth: 0,
    };
    let last_change = repo
        .commit_list
        .first()
        .map(|commit| commit.author_date.format("%Y-%m-%d").to_string());
    let mut rows: Vec<(&str, &str)> =
        vec![("name", &repo.name), ("description", &repo.description)];
    if !repo.owner.is_empty() {
        rows.push(("owner", &repo.owner));
    }
    if let Some(date) = &last_change {
        rows.push(("last change", date));
    }

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
    body.push_str(&format!(
        "\
<div class=\"summary-meta-row\">\n\
<span class=\"summary-meta-key\">clone url</span>\n\
<div class=\"summary-meta-clone m-0\">{CLONE_URL_BASE}{}.git</div>\n\
</div>\n\
</div>\n\
",
        escape_str(&repo.name)
    ));
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
        git_ref: &repo.head,
        tab,
        depth: 0,
    };

    let mut body = String::from("<div class=\"box mt-0 p-0\">\n");
    for git_ref in refs {
        body.push_str(&format!(
            "\
<div class=\"flex justify-between items-center gap-2 py px tree-row border-b\">\n\
<div><a href=\"{}\">{}</a></div>\n\
</div>\n\
",
            escape_str(&Tab::Tree.href(&git_ref.name)),
            escape_str(&git_ref.name)
        ));
    }
    body.push_str("</div>\n");

    let title = format!("{} - {}", tab.label(), repo.name);
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
                    escape_str(&Tab::Tree.href(&git_ref.name)),
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
            escape_str(&Tab::Tree.href(&git_ref.name)),
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
