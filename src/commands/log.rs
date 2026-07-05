use anyhow::Result;
use std::env;

use crate::repo::Repo;

/// `anigit log [--oneline] [--graph]` — commit history for the current
/// branch, newest first.
///
/// `--graph` scope note: `Repo::history()` follows only the first-parent
/// chain, and there is no way to enumerate all branches on disk yet, so a
/// real multi-branch DAG can't be drawn. Instead, merge commits (two
/// parents) get a `|\` branching marker showing WHERE a merge landed,
/// without rendering the second parent's separate line of history.
///
/// Update (part 3): `Repo::list_branches()` now exists, so enumerating all
/// branch tips IS possible — full multi-branch DAG rendering is now
/// unblocked but not yet implemented here. Still TODO.
pub fn run(oneline: bool, graph: bool) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::discover(&cwd)?;
    let branch = repo.current_branch()?;
    let history = repo.history(&branch)?;

    if history.is_empty() {
        println!("No commits yet.");
        return Ok(());
    }

    for commit in history {
        let is_merge = commit.parent_ids.len() >= 2;
        let short = &commit.id[..commit.id.len().min(11)];

        if oneline {
            if graph {
                print!("* ");
            }
            println!("{short} {}", commit.message);
            if graph && is_merge {
                println!("|\\");
            }
        } else {
            // In graph mode, continuation lines get a `|` rail so the
            // commit markers line up in a column like real `git log --graph`.
            let rail = if graph { "| " } else { "" };
            if graph {
                print!("* ");
            }
            println!(
                "commit {}{}",
                commit.id,
                if is_merge { " (merge)" } else { "" }
            );
            if graph && is_merge {
                println!("|\\");
            }
            println!("{rail}Date:   {}", commit.timestamp);
            println!("{rail}");
            println!("{rail}    {}", commit.message);
            println!("{rail}");
        }
    }

    Ok(())
}
