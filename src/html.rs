use crate::error::WitError;
use crate::repo_info::RepoInfo;

use std::{cmp::Reverse, fmt::Write};

const BRAND: &str = "wit";

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

struct RepoNav<'a> {
    name: &'a str,
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

fn wrap_page(title: &str, nav: Option<&RepoNav>, body: &str) -> Result<String, WitError> {
    let root = match nav {
        Some(nav) => "../".repeat(nav.depth + 1),
        None => String::new(),
    };
    let mut page = String::new();

    write!(
        &mut page,
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
    )?;

    let main_class = match nav {
        Some(nav) => {
            let repo_root = "../".repeat(nav.depth);
            write!(
                &mut page,
                "\
<nav id=\"site_path\"><div id=\"site_link\"><a href=\"{repo_root}index.html\">{}</a> / {}</div></nav>\n\
<div class=\"navs_wrapper\"><nav class=\"navs\">\n\
",
                escape_str(nav.name),
                nav.tab.label()
            )?;
            let links: Vec<String> = TABS
                .iter()
                .map(|&tab| {
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
            write!(&mut page, "{}\n</nav></div>\n", links.join(" |\n"))?;
            nav.tab.main_class()
        }
        None => "action-index",
    };

    write!(
        &mut page,
        "\
<main class=\"{main_class}\">\n\
{body}\
</main>\n\
</body>\n\
</html>\n\
"
    )?;

    Ok(page)
}

pub fn gen_index(repos: &[RepoInfo]) -> Result<String, WitError> {
    let mut sorted: Vec<&RepoInfo> = repos.iter().collect();
    sorted.sort_by_key(|repo| Reverse(repo.commit_list.first().map(|c| c.author_date)));

    let mut body = String::from("<div class=\"home-grid\">\n");
    for repo in sorted {
        let name = escape_str(&repo.name);
        let links: Vec<String> = TABS
            .iter()
            .map(|&tab| {
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
        write!(
            &mut body,
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
        )?;
    }
    body.push_str("</div>\n");

    wrap_page("index", None, &body)
}

pub fn gen_page(repo: &RepoInfo) -> Result<String, WitError> {
    let nav = RepoNav {
        name: &repo.name,
        git_ref: &repo.head,
        tab: Tab::Summary,
        depth: 0,
    };
    let body = format!("<p>{}</p>\n", escape_str(&repo.description));
    wrap_page(&repo.name, Some(&nav), &body)
}
