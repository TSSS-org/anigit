use anyhow::Result;
use std::env;

use crate::repo::Repo;

/// `anigit show <commit_id>` — print full details of a single commit.
pub fn run(commit_id: &str) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::discover(&cwd)?;
    let commit = repo.read_commit(commit_id)?;

    println!("commit {}", commit.id);
    println!("Branch: {}", commit.branch);
    println!("Date:   {}", commit.timestamp);
    println!("Parents: {:?}", commit.parent_ids);
    println!("Catalog ref: {}#{}", commit.catalog_ref.source, commit.catalog_ref.id);
    println!();
    println!("    {}", commit.message);
    println!();
    println!("Changes: {:#?}", commit.changes);

    Ok(())
}
