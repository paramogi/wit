mod error;
mod html;
mod repo_info;

use html::gen_page;
use repo_info::RepoInfo;
use std::{env, process};

fn main() {
    let Some(arg) = env::args().nth(1) else {
        eprintln!("provide a path");
        process::exit(1);
    };

    let repo = match RepoInfo::open(&arg) {
        Ok(repo) => repo,
        Err(e) => {
            eprintln!("{e}");
            process::exit(1);
        }
    };
    //println!("{repo:#?}");

    let page = match gen_page(&repo) {
        Ok(page) => page,
        Err(e) => {
            eprint!("{e}");
            process::exit(1);
        }
    };

    println!("{page}");
}
