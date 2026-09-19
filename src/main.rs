use git2::Repository;
use std::env;
use std::process;

fn main() {
    let repo_path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("provide the repo path");
        process::exit(1);
    });

    let repo = Repository::open_bare(repo_path).unwrap_or_else(|_| {
        eprintln!("repo must be bare");
        process::exit(1);
    });
}
