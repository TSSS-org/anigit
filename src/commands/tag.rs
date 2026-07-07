use anyhow::{bail, Result};
use colored::Colorize;
use std::env;

use crate::repo::Repo;

/// `anigit tag <name>` — create a tag at the current commit (e.g.
/// "watched-100-shows", "2025-completed"). Milestone/flavor feature per
/// brainstorm.md section 2 (Under Consideration) — cheap plumbing on top of
/// the commit/ref model. Creation only; listing/deletion aren't part of the
/// v1 CLI surface.
pub fn run(name: &str) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::discover(&cwd)?;
    let branch = repo.current_branch()?;

    let Some(head) = repo.branch_head(&branch)? else {
        bail!("cannot tag: branch '{branch}' has no commits yet");
    };
    repo.create_tag(name, &head)?;

    println!(
        "{}",
        format!("Created tag '{name}' at {}.", &head[..head.len().min(11)]).green()
    );
    Ok(())
}
