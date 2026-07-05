use anyhow::Result;
use std::env;

use crate::repo::Repo;

/// `anigit branch [name]` — list branches, or create a new one from the
/// current HEAD if `name` is given. Mirrors real `git branch`: creation
/// does NOT switch to the new branch (that's `checkout`/`switch -b`).
pub fn run(name: Option<&str>) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::discover(&cwd)?;

    match name {
        None => {
            let current = repo.current_branch()?;
            for branch in repo.list_branches()? {
                if branch == current {
                    println!("* {branch}");
                } else {
                    println!("  {branch}");
                }
            }
        }
        Some(name) => {
            repo.create_branch(name)?;
            println!("Created branch '{name}'.");
        }
    }

    Ok(())
}
