use git2::Repository;
use std::env;
use std::process;

fn main() {
    let Some(arg) = env::args().nth(1) else {
        eprintln!("provide the repo path");
        process::exit(1);
    };

    let repo = Repository::open_bare(arg).unwrap_or_else(|e| {
        eprintln!("{}", e.message());
        process::exit(1);
    });
}
