use anyhow::{bail, Result};
use colored::Colorize;
use std::env;

use crate::repo::Repo;

/// Shared implementation for `anigit checkout` and `anigit switch` — both
/// commands are kept as real, separate commands (not one aliasing the
/// other) per the no-aliases naming policy in brainstorm.md 1.7a, but their
/// behavior is identical: switch HEAD to `branch`, optionally creating it
/// first if `create` (`-b` flag) is set.
pub fn run(branch: &str, create: bool) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::discover(&cwd)?;

    if create {
        repo.create_branch(branch)?;
    } else if !repo.branch_exists(branch) {
        bail!(
            "no such branch: {branch}\n\
             Use `-b` to create and switch to it in one step."
        );
    }

    if repo.current_branch()? == branch {
        println!("Already on branch '{branch}'.");
        return Ok(());
    }

    repo.set_current_branch(branch)?;
    println!("{}", format!("Switched to branch '{branch}'.").green());
    Ok(())
}
