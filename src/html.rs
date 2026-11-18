use crate::repo_info::{Folder, RefInfo, RepoInfo, TreeEntry};

use pulldown_cmark::{Event, LinkType, Options, Parser, Tag, html::push_html};
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

const IMAGE_TYPES: [(&str, &str); 6] = [
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("gif", "image/gif"),
    ("webp", "image/webp"),
    ("svg", "image/svg+xml"),
];

const BASE64_CHARS: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

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

fn root_folder<'a>(repo: &'a RepoInfo, dir: &str) -> Option<&'a Folder> {
    repo.branches
        .iter()
        .chain(&repo.tags)
        .find(|git_ref| git_ref.dir == dir)
        .and_then(|git_ref| git_ref.folders.iter().find(|f| f.path.is_empty()))
}

fn file_links(nav: &RepoNav, repo_root: &str) -> Vec<String> {
    let Some(root) = root_folder(nav.repo, nav.git_ref) else {
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
            let summary_href = (nav.tab != Tab::Summary).then(|| format!("{repo_root}index.html"));
            page.push_str(&format!(
                "\
<nav id=\"site_path\"><div id=\"site_link\">{}</div></nav>\n\
<div class=\"navs_wrapper flex justify-between\"><nav class=\"navs\">\n\
",
                crumb(&nav.repo.name, summary_href)
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
            .filter(|&tab| tab != Tab::Summary)
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
    if let Some(root) = root_folder(repo, &repo.head_dir) {
        body.push_str(
            "\
<div class=\"summary-tree\">\n\
<header class=\"subtitle\">\n\
<h2>tree</h2>\n\
</header>\n\
<div id=\"tree_content\">\n\
",
        );
        let tree_dir = format!("tree/{}/", repo.head_dir);
        let blob_dir = format!("blob/{}/", repo.head_dir);
        write_listing(&mut body, root, &tree_dir, &blob_dir);
        body.push_str("</div>\n</div>\n");
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
    let tree_path = join_path(&git_ref.dir, &folder.path);
    let depth = 1 + tree_path.split('/').count();
    let nav = RepoNav {
        repo,
        git_ref: &git_ref.dir,
        tab: Tab::Tree,
        depth,
    };
    let repo_root = "../".repeat(depth);
    let blob_dir = format!("{repo_root}blob/{tree_path}/");

    let mut body = ref_bar(git_ref, &folder.path, &repo_root, "");
    write_listing(&mut body, folder, "", &blob_dir);

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
) -> String {
    let depth = 1 + join_path(&git_ref.dir, &folder.path).split('/').count();
    let nav = RepoNav {
        repo,
        git_ref: &git_ref.dir,
        tab: Tab::Tree,
        depth,
    };
    let repo_root = "../".repeat(depth);
    let path = join_path(&folder.path, &entry.name);
    let mut details = String::from("<div class=\"ml-auto flex items-center gap\">\n");
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
        details.push_str(&format!(
            "<nav class=\"navs\">{} |\n{}</nav>\n",
            crumb("source", source_href),
            crumb("rendered", rendered_href)
        ));
    }
    details.push_str(&size_cells(entry));
    details.push_str("</div>\n");

    let mut body = ref_bar(git_ref, &path, &repo_root, &details);
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

pub fn has_source_page(entry: &TreeEntry) -> bool {
    entry.lines.is_some() && (image_type(&entry.name).is_some() || is_markdown(&entry.name))
}

fn is_markdown(name: &str) -> bool {
    name.rsplit_once('.')
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
        | Options::ENABLE_FOOTNOTES;
    let events = Parser::new_ext(&text, options).map(|event| match event {
        Event::FootnoteReference(label) => Event::FootnoteReference(format!("fn-{label}").into()),
        Event::Start(Tag::FootnoteDefinition(label)) => {
            Event::Start(Tag::FootnoteDefinition(format!("fn-{label}").into()))
        }
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
        _ => event,
    });
    body.push_str("<div class=\"box markdown-body\">");
    push_html(body, events);
    body.push_str("</div>\n");
}

fn link_target(git_ref: &RefInfo, base: &str, repo_root: &str, url: &str) -> Option<String> {
    let path = repo_path(base, url.split(['?', '#']).next()?)?;
    let href = if git_ref.folders.iter().any(|folder| folder.path == path) {
        Tab::Tree.href(&join_path(&git_ref.dir, &path))
    } else {
        find_file(git_ref, &path)?;
        percent_encode(&format!("blob/{}/{path}.html", git_ref.dir))
    };
    let fragment = url
        .split_once('#')
        .map(|(_, fragment)| format!("#{fragment}"))
        .unwrap_or_default();
    Some(format!("{repo_root}{href}{fragment}"))
}

fn image_data(repo: &RepoInfo, git_ref: &RefInfo, base: &str, url: &str) -> Option<String> {
    let path = repo_path(base, url.split(['?', '#']).next()?)?;
    let mime = image_type(&path)?;
    let blob = repo.git.find_blob(find_file(git_ref, &path)?.blob?).ok()?;
    Some(data_uri(mime, blob.content()))
}

fn repo_path(base: &str, url: &str) -> Option<String> {
    if url.is_empty() || url.starts_with("//") || url.split('/').next()?.contains(':') {
        return None;
    }
    let url = percent_decode(url)?;
    let mut parts: Vec<&str> = if url.starts_with('/') {
        Vec::new()
    } else {
        base.split('/').filter(|part| !part.is_empty()).collect()
    };
    for part in url.split('/') {
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

fn find_file<'a>(git_ref: &'a RefInfo, path: &str) -> Option<&'a TreeEntry> {
    let (dir, name) = path.rsplit_once('/').unwrap_or(("", path));
    git_ref
        .folders
        .iter()
        .find(|folder| folder.path == dir)?
        .entries
        .iter()
        .find(|entry| entry.name == name && entry.blob.is_some())
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

fn ref_bar(git_ref: &RefInfo, path: &str, repo_root: &str, details: &str) -> String {
    let mut tree_path = git_ref.dir.clone();
    let mut levels = vec![(git_ref.name.clone(), Tab::Tree.href(&tree_path))];
    for part in path.split('/').filter(|p| !p.is_empty()) {
        tree_path = format!("{tree_path}/{part}");
        levels.push((part.to_string(), Tab::Tree.href(&tree_path)));
    }

    let mut bar =
        String::from("<div class=\"box sticky flex items-center gap border-visited flex-wrap\">\n");
    let last = levels.len() - 1;
    for (i, (text, href)) in levels.iter().enumerate() {
        let link = crumb(text, (i != last).then(|| format!("{repo_root}{href}")));
        match i {
            0 => bar.push_str(&format!("<code>{link}</code>\n")),
            1 => bar.push_str(&format!("{link}\n")),
            _ => bar.push_str(&format!("<span>/</span>\n{link}\n")),
        }
    }
    bar.push_str(details);
    bar.push_str("</div>\n");
    bar
}

fn size_cells(entry: &TreeEntry) -> String {
    let lines = entry.lines.map(|n| format!("{n} L")).unwrap_or_default();
    let bytes = entry.bytes.map(format_bytes).unwrap_or_default();
    format!("<div class=\"tree-size\">{lines}</div>\n<div class=\"tree-size\">{bytes}</div>\n")
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

fn write_listing(body: &mut String, folder: &Folder, tree_dir: &str, blob_dir: &str) {
    body.push_str("<div class=\"box p-0\">\n");
    for entry in &folder.entries {
        let name = &entry.name;
        let (icon, href) = if entry.is_dir {
            (
                FOLDER_ICON,
                percent_encode(&format!("{tree_dir}{name}/index.html")),
            )
        } else {
            (FILE_ICON, percent_encode(&format!("{blob_dir}{name}.html")))
        };
        body.push_str(&format!(
            "\
<div class=\"flex justify-between items-center gap-2 py px tree-row border-b\">\n\
<div class=\"flex-1 tree-path flex items-center gap\">\n\
{icon}\n\
<a href=\"{href}\">{}</a>\n\
</div>\n\
<div class=\"flex items-center gap\">\n\
{}\
</div>\n\
</div>\n\
",
            escape_str(name),
            size_cells(entry)
        ));
    }
    body.push_str("</div>\n");
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
