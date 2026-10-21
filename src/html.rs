use crate::error::WitError;
use crate::repo_info::RepoInfo;

use std::fmt::Write;

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

pub fn gen_page(repo: &RepoInfo) -> Result<String, WitError> {
    let mut page = String::new();

    write!(
        &mut page,
        "\
<!DOCTYPE html>\n\
<html>\n\
\t<head>\n\
\t\t<meta charset=\"utf-8\">\n\
\t\t<title>{}</title>\n\
\t</head>\n\
\t<body>\n\
\t\t<p>{}</p>\n\
\t</body>\n\
</html>\
",
        escape_str(&repo.name),
        escape_str(&repo.description)
    )?;

    Ok(page)
}
