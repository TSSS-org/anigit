use anyhow::Result;
use std::env;

use crate::catalog::{catalog_path, Catalog, CatalogEntry};
use crate::commands::compare::net_changes;
use crate::repo::commit::{Changes, Commit, WatchStatus};
use crate::repo::Repo;
use crate::tui::blame_search::run_blame_search_screen;

/// `anigit blame` — for each `Changes` field, show which commit most
/// recently set it for one anime, picked via an interactive search screen
/// (brainstorm.md 1.15; same treatment `add` got in uxFix1, replacing the
/// old ambiguous-picks-first `resolve_by_name` `<anime name>` argument).
/// Unlike `add`'s screen, the search is scoped to anime this repo's own
/// history has actually touched — blame is about "what have I said about
/// things I've already tracked," not catalog browsing — so the candidate
/// list is built once up front from history keys and filtered in memory.
///
/// The blame walk itself is unchanged: deliberately simple (current
/// branch's first-parent chain only) — blame is a low-priority,
/// nice-to-have feature per brainstorm.md section 2.
pub fn run() -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::discover(&cwd)?;
    let branch = repo.current_branch()?;
    let history = repo.history(&branch)?;

    // net_changes's keys are exactly the set of anime this repo has ever
    // committed something about — the blame search space.
    let tracked = net_changes(&history);
    if tracked.is_empty() {
        println!("No commits yet on branch '{branch}' — nothing to blame.");
        return Ok(());
    }

    let catalog = Catalog::open(&catalog_path()?)?;
    let mut keys: Vec<_> = tracked.into_keys().collect();
    keys.sort(); // stable list order across runs
    let mut candidates: Vec<CatalogEntry> = Vec::new();
    for (source, id) in keys {
        // Skip entries the catalog can't resolve (non-anilist source, or a
        // stale/missing catalog) — blame can't show anything useful for an
        // entry with no metadata, and crashing would be worse.
        if source != "anilist" {
            continue;
        }
        if let Some(entry) = catalog.find_by_id(id)? {
            candidates.push(entry);
        }
    }
    if candidates.is_empty() {
        println!(
            "None of this repo's tracked anime were found in the local catalog.\n\
             Try `anigit refresh` to update it."
        );
        return Ok(());
    }
    candidates.sort_by(|a, b| a.display_title().to_lowercase().cmp(&b.display_title().to_lowercase()));

    let Some(entry) = run_blame_search_screen(candidates)? else {
        println!("Cancelled.");
        return Ok(());
    };

    // history() is newest-first, so the first commit that set a field is the
    // most recent one to touch it.
    let history: Vec<Commit> = history
        .into_iter()
        .filter(|c| c.catalog_ref.source == "anilist" && c.catalog_ref.id == entry.id)
        .collect();

    if history.is_empty() {
        // Can't normally happen (candidates come from this history), but
        // keep the guard rather than assuming.
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
