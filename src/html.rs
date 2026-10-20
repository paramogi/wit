use crate::error::WitError;
use crate::repo_info::RepoInfo;

use std::fmt::Write;

pub fn gen_page(repo: &RepoInfo) -> Result<String, WitError> {
    let mut page = String::new();

    write!(
        &mut page,
        "\
<!DOCTYPE html>\n\
<html>\n\
\t<head>\n\
\t\t<title>hello</title>\n\
\t</head>\n\
\t<body>\n\
\t\t<p>{}</p>\n\
\t</body>\n\
</html>\
",
        repo.description
    )?;

    Ok(page)
}
