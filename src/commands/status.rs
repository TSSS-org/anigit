use anyhow::Result;
use colored::Colorize;
use std::env;

use super::diff::changes_lines;
use crate::repo::Repo;

/// `anigit status` — current branch, repo kind/visibility, HEAD, and the
/// staging area (mirrors `git status`'s "Changes to be committed" section;
/// staging model per brainstorm.md 1.7a).
pub fn run() -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::discover(&cwd)?;
    let branch = repo.current_branch()?;
    let config = repo.config()?;

    println!("On branch {}", branch.green());
    println!("{} {:?}", "Repo kind:".cyan(), config.repo_kind);
    println!("{} {:?}", "Visibility:".cyan(), config.visibility);

    match repo.branch_head(&branch)? {
        Some(head) => println!("HEAD -> {head}"),
        None => println!("No commits yet."),
    }

    println!();
    match repo.read_staged()? {
        Some(staged) => {
            println!("{}", "Changes staged for commit:".cyan());
            println!(
                "  {} ({}/{})",
                staged.anime_title, staged.catalog_ref.source, staged.catalog_ref.id
            );
            let lines = changes_lines(&staged.changes);
            if lines.is_empty() {
                println!("    (no field changes recorded)");
            } else {
                for line in lines {
                    println!("    {line}");
                }
            }
            println!("\nUse `anigit commit -m \"<message>\"` to record them.");
        }
        None => {
            println!("Nothing staged.");
            println!("Use `anigit add <anime name>` to stage changes.");
        }
    }

    Ok(())
}
