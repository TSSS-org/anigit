use anyhow::Result;
use std::env;

use crate::repo::Repo;

pub fn run() -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::discover(&cwd)?;
    let branch = repo.current_branch()?;
    let config = repo.config()?;

    println!("On branch {branch}");
    println!("Repo kind: {:?}", config.repo_kind);
    println!("Visibility: {:?}", config.visibility);

    match repo.branch_head(&branch)? {
        Some(head) => println!("HEAD -> {head}"),
        None => println!("No commits yet."),
    }

    // TODO: once `anigit add` staging is implemented, show staged-but-not-
    // committed changes here too (mirrors `git status`'s "Changes to be
    // committed" section).
    println!("(staged changes display not yet implemented)");

    Ok(())
}
