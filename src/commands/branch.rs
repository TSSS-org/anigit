use anyhow::{bail, Result};
use colored::Colorize;
use std::env;

use crate::repo::Repo;

/// `anigit branch [name] [-d|-D]` — list branches, create a new one from
/// the current HEAD if `name` is given, or delete one with `-d` (safe:
/// refuses if the branch's commits would become unreachable) / `-D`
/// (force). Mirrors real `git branch`: creation does NOT switch to the new
/// branch (that's `checkout`/`switch -b`).
pub fn run(name: Option<&str>, delete: bool, force_delete: bool) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::discover(&cwd)?;

    if delete || force_delete {
        // clap's `requires = "name"` guarantees this, but guard anyway so a
        // future dispatch change can't silently turn delete into list.
        let Some(name) = name else {
            bail!("branch deletion requires a branch name");
        };
        repo.delete_branch(name, force_delete)?;
        println!("{}", format!("Deleted branch '{name}'.").green());
        return Ok(());
    }

    match name {
        None => {
            let current = repo.current_branch()?;
            for branch in repo.list_branches()? {
                if branch == current {
                    println!("* {}", branch.green());
                } else {
                    println!("  {branch}");
                }
            }
        }
        Some(name) => {
            repo.create_branch(name)?;
            println!("{}", format!("Created branch '{name}'.").green());
        }
    }

    Ok(())
}
