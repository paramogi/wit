use crate::html::{
    data_uri, escape_str, find_file, follow_links, icon, image_type, path_href, percent_encode,
    resolve_path,
};
use crate::repo_info::{RefInfo, RepoInfo};

use pulldown_cmark::{
    BlockQuoteKind, Event, HeadingLevel, LinkType, Options, Parser, Tag, TagEnd, html::push_html,
};
use std::collections::{HashMap, HashSet};

const PAGE_IDS: [&str; 7] = [
    "header",
    "site_path",
    "ref-menu",
    "ref-branches",
    "ref-tags",
    "view-source",
    "view-rendered",
];

const NUMBERED_PAGE_IDS: [&str; 2] = ["ref-more-", "release-"];

struct RawHeading {
    start: usize,
    level: u8,
    open_end: usize,
    close: usize,
    close_end: usize,
    has_id: bool,
    text: String,
    label: String,
}

pub fn is_markdown(name: &str) -> bool {
    name == "README"
        || name
            .rsplit_once('.')
            .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("md"))
}

pub fn write_markdown(
    body: &mut String,
    class: &str,
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
    let parsed = Parser::new_ext(&text, options).map(|mut event| {
        match &mut event {
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                ..
            }) if *link_type != LinkType::Email => {
                if let Some(url) = link_target(git_ref, base, repo_root, dest_url) {
                    *dest_url = url.into();
                }
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                if let Some(url) = image_data(repo, git_ref, base, dest_url) {
                    *dest_url = url.into();
                }
            }
            Event::Html(html) | Event::InlineHtml(html) => {
                *html = rewrite_html(html, repo, git_ref, base, repo_root).into();
            }
            _ => {}
        }
        event
    });
    let mut events: Vec<Event> = Vec::new();
    let mut html: Option<String> = None;
    for event in parsed {
        if let Event::Html(line) = &event {
            html.get_or_insert_default().push_str(line);
            continue;
        }
        events.extend(html.take().map(|block| Event::Html(block.into())));
        events.push(event);
    }
    events.extend(html.map(|block| Event::Html(block.into())));
    let (events, footnote_ids) = gather_footnotes(events);
    let mut ids = heading_ids(&events, footnote_ids).into_iter();
    let mut html_events = Vec::with_capacity(events.len());
    let mut heading_end: Option<String> = None;
    for event in events {
        match event {
            Event::Start(Tag::Heading {
                level,
                classes,
                attrs,
                ..
            }) => {
                let (id, label) = ids.next().unwrap_or_default();
                let level = lower_level(level);
                if id.is_empty() {
                    html_events.push(Event::Start(Tag::Heading {
                        level,
                        id: None,
                        classes,
                        attrs,
                    }));
                } else {
                    let n = level as usize;
                    html_events.push(Event::Html(
                        format!("<div class=\"markdown-heading\"><h{n} id=\"{id}\">").into(),
                    ));
                    heading_end = Some(format!("</h{n}>{}</div>\n", anchor_link(&id, &label)));
                }
            }
            Event::End(TagEnd::Heading(level)) => html_events.push(match heading_end.take() {
                Some(end) => Event::Html(end.into()),
                None => Event::End(TagEnd::Heading(lower_level(level))),
            }),
            Event::Start(Tag::BlockQuote(Some(kind))) => {
                let (name, label, icon_name) = match kind {
                    BlockQuoteKind::Note => ("note", "Note", "info"),
                    BlockQuoteKind::Tip => ("tip", "Tip", "light-bulb"),
                    BlockQuoteKind::Important => ("important", "Important", "report"),
                    BlockQuoteKind::Warning => ("warning", "Warning", "alert"),
                    BlockQuoteKind::Caution => ("caution", "Caution", "stop"),
                };
                html_events.push(Event::Html(
                    format!(
                        "<blockquote class=\"markdown-alert markdown-alert-{name}\">\n<p class=\"markdown-alert-title\">{}{label}</p>\n",
                        icon(icon_name)
                    )
                    .into(),
                ));
            }
            Event::Html(html) => {
                let lower = html.to_ascii_lowercase();
                let mut linked = String::with_capacity(html.len());
                let mut copied = 0;
                for heading in raw_headings(&html) {
                    let (id, label) = if heading.has_id {
                        Default::default()
                    } else {
                        ids.next().unwrap_or_default()
                    };
                    let in_summary = lower[..heading.start].rfind("<summary")
                        > lower[..heading.start].rfind("</summary");
                    let wrap = !id.is_empty() && !in_summary;
                    let n = (heading.level + 1).min(6);
                    linked.push_str(&html[copied..heading.start]);
                    if wrap {
                        linked.push_str("<div class=\"markdown-heading\">");
                    }
                    linked.push_str(&format!(
                        "<h{n}{}",
                        &html[heading.start + 3..heading.open_end]
                    ));
                    if !id.is_empty() {
                        linked.push_str(&format!(" id=\"{id}\""));
                    }
                    linked.push_str(&format!(
                        ">{}</h{n}>",
                        &html[heading.open_end + 1..heading.close]
                    ));
                    if !id.is_empty() {
                        linked.push_str(&anchor_link(&id, &label));
                    }
                    if wrap {
                        linked.push_str("</div>");
                    }
                    copied = heading.close_end;
                }
                linked.push_str(&html[copied..]);
                html_events.push(Event::Html(linked.into()));
            }
            _ => html_events.push(event),
        }
    }
    body.push_str(&format!("<div class=\"{class}\">"));
    push_html(body, html_events.into_iter());
    body.push_str("</div>\n");
}

fn gather_footnotes(events: Vec<Event<'_>>) -> (Vec<Event<'_>>, Vec<String>) {
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
                        "<sup class=\"footnote-reference\" id=\"{}\"><a href=\"#{}\">[{number}]</a></sup>",
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
    let mut ids = Vec::new();
    if referenced.is_empty() {
        return (main, ids);
    }
    main.push(Event::Html("<section class=\"footnotes\">\n<ol>\n".into()));
    for (label, count) in referenced {
        ids.push(format!("fn-{label}"));
        ids.extend((1..=count).map(|k| footnote_ref_id(&label, k)));
        let mut body = definitions.remove(&label).unwrap_or_default();
        let arrows: String = (1..=count)
            .map(|k| {
                let number = if k == 1 {
                    String::new()
                } else {
                    format!("<sup>{k}</sup>")
                };
                format!(
                    " <a href=\"#{}\" class=\"footnote-backref\">↩\u{FE0E}{number}</a>",
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
    (main, ids)
}

fn footnote_ref_id(label: &str, k: usize) -> String {
    if k == 1 {
        format!("fnref-{label}")
    } else {
        format!("fnref-{label}-{k}")
    }
}

fn heading_ids(events: &[Event], footnote_ids: Vec<String>) -> Vec<(String, String)> {
    let mut used: HashSet<String> = PAGE_IDS
        .iter()
        .map(|id| id.to_string())
        .chain(footnote_ids)
        .collect();
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
            Event::End(TagEnd::Heading(_)) => texts.extend(heading.take().map(|text| {
                let label = escape_str(&text);
                (text, label)
            })),
            Event::Html(html) => texts.extend(
                raw_headings(html)
                    .into_iter()
                    .filter(|heading| !heading.has_id)
                    .map(|heading| (heading.text, heading.label)),
            ),
            _ => {}
        }
    }
    texts
        .into_iter()
        .map(|(text, label)| {
            let slug: String = text
                .to_lowercase()
                .chars()
                .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'))
                .map(|c| if c == ' ' { '-' } else { c })
                .collect();
            let mut id = slug.clone();
            let mut n = 0;
            while !slug.is_empty() && (used.contains(&id) || numbered_page_id(&id)) {
                n += 1;
                id = format!("{slug}-{n}");
            }
            used.insert(id.clone());
            (id, label)
        })
        .collect()
}

fn numbered_page_id(id: &str) -> bool {
    NUMBERED_PAGE_IDS.iter().any(|prefix| {
        id.strip_prefix(prefix)
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
    })
}

fn is_entity(rest: &str) -> bool {
    rest.split_once(';').is_some_and(|(name, _)| {
        !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'#')
    })
}

fn lower_level(level: HeadingLevel) -> HeadingLevel {
    HeadingLevel::try_from(level as usize + 1).unwrap_or(HeadingLevel::H6)
}

fn raw_headings(html: &str) -> Vec<RawHeading> {
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
        let Some(close_end) = lower[close..].find('>').map(|i| close + i + 1) else {
            break;
        };
        from = close;
        let has_id = lower[start..open_end]
            .split_whitespace()
            .any(|attr| attr.starts_with("id="));
        let inner = &html[open_end + 1..close];
        let mut label = String::new();
        let mut in_tag = false;
        for (at, c) in inner.char_indices() {
            if in_tag {
                in_tag = c != '>';
            } else if c == '<' {
                in_tag = true;
            } else if c == '&' && is_entity(&inner[at + 1..]) {
                label.push('&');
            } else {
                label.push_str(&escape_str(&c.to_string()));
            }
        }
        let mut text = String::new();
        let mut until = None;
        for c in inner.chars() {
            match until {
                Some(end) if c == end => until = None,
                Some(';') if !c.is_ascii_alphanumeric() && c != '#' => {
                    until = None;
                    text.push(c);
                }
                Some(_) => {}
                None if c == '<' => until = Some('>'),
                None if c == '&' => until = Some(';'),
                None => text.push(c),
            }
        }
        headings.push(RawHeading {
            start,
            level: level - b'0',
            open_end,
            close,
            close_end,
            has_id,
            text: text.trim().to_string(),
            label: label.trim().to_string(),
        });
    }
    headings
}

fn anchor_link(id: &str, label: &str) -> String {
    format!(
        "<a class=\"anchor\" href=\"#{id}\" aria-label=\"{label}, link to this section\">{}</a>",
        icon("link")
    )
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
