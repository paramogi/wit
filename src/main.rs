mod error;
mod html;
mod repo_info;

use html::gen_page;
use repo_info::RepoInfo;
use std::{env, process};

use crate::error::WitError;

fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        process::exit(1);
    }
}

fn run() -> Result<(), WitError> {
    let arg = env::args().nth(1).ok_or(WitError::MissingPath)?;
    let repo = RepoInfo::open(&arg)?;
    //println!("{repo:#?}");
    let page = gen_page(&repo)?;
    println!("{page}");
    Ok(())
}
