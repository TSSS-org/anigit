use anyhow::Result;
use std::env;

use crate::repo::Repo;

pub fn run(oneline: bool, graph: bool) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::discover(&cwd)?;
    let branch = repo.current_branch()?;
    let history = repo.history(&branch)?;

    if history.is_empty() {
        println!("No commits yet.");
        return Ok(());
    }

    // TODO: real --graph rendering (branch/merge lines). For now, --graph
    // falls back to the same linear output as default; --oneline gives a
    // condensed single-line-per-commit view like real `git log --oneline`.
    let _ = graph;

    for commit in history {
        if oneline {
            println!(
                "{} {}",
                &commit.id[..commit.id.len().min(11)],
                commit.message
            );
        } else {
            println!("commit {}", commit.id);
            println!("Date:   {}", commit.timestamp);
            println!();
            println!("    {}", commit.message);
            println!();
        }
    }

    Ok(())
}
