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

    let mut rev = repo.revwalk().unwrap_or_else(|e| {
        eprintln!("{}", e.message());
        process::exit(1);
    });

    let _ = rev.push_head();

    for r in rev {
        let oid = r.unwrap();
        let commit = repo.find_commit(oid).unwrap();
        println!("{}", commit.message().unwrap());
        println!("{}", commit.author());
        println!("{}", commit.time().seconds());
    }
}
