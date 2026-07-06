use anyhow::Result;
use std::env;

use crate::catalog::{catalog_path, resolve_by_name, Catalog};
use crate::repo::commit::{Changes, Commit, WatchStatus};
use crate::repo::Repo;

/// `anigit blame <anime name>` — for each `Changes` field, show which commit
/// most recently set it for the given anime. Deliberately simple (walks the
/// current branch's first-parent chain only) — blame is a low-priority,
/// nice-to-have feature per brainstorm.md section 2.
pub fn run(anime_name: &str) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::discover(&cwd)?;

    let catalog = Catalog::open(&catalog_path()?)?;
    let entry = resolve_by_name(&catalog, anime_name)?;

    let branch = repo.current_branch()?;
    // history() is newest-first, so the first commit that set a field is the
    // most recent one to touch it.
    let history: Vec<Commit> = repo
        .history(&branch)?
        .into_iter()
        .filter(|c| c.catalog_ref.source == "anilist" && c.catalog_ref.id == entry.id)
        .collect();

    if history.is_empty() {
        println!(
            "'{}' (anilist/{}) has no commits on branch '{branch}'.",
            entry.display_title(),
            entry.id
        );
        return Ok(());
    }

    println!(
        "Blame for '{}' (anilist/{}) on branch {branch}:",
        entry.display_title(),
        entry.id
    );
    field_blame(&history, "status", |c| c.status.map(status_str_owned));
    field_blame(&history, "episode_progress", |c| {
        c.episode_progress.map(|v| v.to_string())
    });
    field_blame(&history, "score", |c| c.score.map(|v| v.to_string()));
    field_blame(&history, "rewatch_count", |c| {
        c.rewatch_count.map(|v| v.to_string())
    });

    Ok(())
}

/// Print one blame line for a single field: the newest commit that set it
/// (with when and to what value), or "never set".
fn field_blame(history: &[Commit], name: &str, value: impl Fn(&Changes) -> Option<String>) {
    let hit = history
        .iter()
        .find_map(|c| value(&c.changes).map(|v| (c, v)));
    match hit {
        Some((commit, value)) => println!(
            "  {name}: {value}  ({} {} \"{}\")",
            &commit.id[..commit.id.len().min(11)],
            commit.timestamp,
            commit.message
        ),
        None => println!("  {name}: never set for this anime"),
    }
}

fn status_str_owned(status: WatchStatus) -> String {
    match status {
        WatchStatus::Dropped => "dropped",
        WatchStatus::Planning => "planning",
        WatchStatus::Watching => "watching",
        WatchStatus::Completed => "completed",
    }
    .to_string()
}
