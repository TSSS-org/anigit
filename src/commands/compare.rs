use anyhow::{Context, Result};
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

use crate::catalog::{catalog_path, Catalog};
use crate::repo::commit::{Changes, Commit, WatchStatus};
use crate::repo::Repo;

/// `anigit compare <other_repo>` — anigit's own invented command (no git
/// equivalent). Implements the Option D manual merge-conflict-resolution-
/// by-comparison UX from brainstorm.md 1.7: surfaces a diff/comparison view
/// between two repos/lists ("you said 8, they said 6...") rather than
/// silently auto-picking a value. Doubles as a standalone "compare two
/// lists" feature independent of any merge.
///
/// This module also owns the pieces `anigit merge` builds on rather than
/// duplicating: `net_changes` (replay commits into per-anime current state)
/// and `print_comparison` (the side-by-side field display used both here
/// and for merge conflicts).
///
/// `other_repo` is a local filesystem path (no network/AniHub in v1,
/// brainstorm.md 1.8). Strictly read-only — never writes to either repo.
pub fn run(other_repo: &str) -> Result<()> {
    let cwd = env::current_dir()?;
    let mine = Repo::discover(&cwd)?;
    let theirs = Repo::discover(&PathBuf::from(other_repo))
        .with_context(|| format!("could not open other repo at '{other_repo}'"))?;

    // Two standalone repos share no commit lineage at all, so they're
    // compared by full current state per catalog_ref: replay each repo's
    // entire history from the beginning (same net_changes logic merge uses
    // from a common ancestor, just anchored at the root).
    let my_state = net_changes(&mine.history(&mine.current_branch()?)?);
    let their_state = net_changes(&theirs.history(&theirs.current_branch()?)?);

    let catalog = open_catalog();

    let mut keys: Vec<EntryKey> = my_state.keys().chain(their_state.keys()).cloned().collect();
    keys.sort();
    keys.dedup();

    if keys.is_empty() {
        println!("Nothing to compare — both repos have no entries.");
        return Ok(());
    }

    for key in keys {
        let name = display_name(catalog.as_ref(), &key);
        match (my_state.get(&key), their_state.get(&key)) {
            (Some(m), Some(t)) if m == t => println!("= {name} — identical"),
            (Some(m), Some(t)) => {
                println!("~ {name}");
                print_comparison("yours", "theirs", m, t);
            }
            (Some(m), None) => {
                println!("< {name} — only in yours");
                print_comparison("yours", "theirs", m, &Changes::default());
            }
            (None, Some(t)) => {
                println!("> {name} — only in theirs");
                print_comparison("yours", "theirs", &Changes::default(), t);
            }
            (None, None) => unreachable!("key came from one of the two maps"),
        }
    }

    Ok(())
}

/// Hashable stand-in for `CatalogRef` (which doesn't derive `Hash`, and the
/// data model is frozen): `(source, id)`.
pub type EntryKey = (String, i64);

/// Replay a commit list (newest-first, as `Repo::history` returns) into net
/// per-anime state: for each field, the most recent commit that set it wins
/// — the same "current state = replay all entries in order" rule as the
/// whole event-log model (brainstorm.md 1.3).
pub fn net_changes(commits_newest_first: &[Commit]) -> HashMap<EntryKey, Changes> {
    let mut state: HashMap<EntryKey, Changes> = HashMap::new();
    for commit in commits_newest_first.iter().rev() {
        let key = (commit.catalog_ref.source.clone(), commit.catalog_ref.id);
        let entry = state.entry(key).or_default();
        let c = &commit.changes;
        if c.status.is_some() {
            entry.status = c.status;
        }
        if c.episode_progress.is_some() {
            entry.episode_progress = c.episode_progress;
        }
        if c.score.is_some() {
            entry.score = c.score;
        }
        if c.rewatch_count.is_some() {
            entry.rewatch_count = c.rewatch_count;
        }
    }
    state
}

/// The side-by-side field display shared between standalone `compare` output
/// and `merge`'s conflict view (brainstorm.md 1.7, Option D — one code path,
/// not two lookalikes).
pub fn print_comparison(left_label: &str, right_label: &str, left: &Changes, right: &Changes) {
    for ((name, lv), (_, rv)) in field_rows(left).into_iter().zip(field_rows(right)) {
        if lv.is_none() && rv.is_none() {
            continue;
        }
        let marker = if lv != rv { "  << differs" } else { "" };
        println!(
            "    {name}: {left_label}={} {right_label}={}{marker}",
            lv.as_deref().unwrap_or("(unset)"),
            rv.as_deref().unwrap_or("(unset)")
        );
    }
}

/// "'Title' (anilist/30)" when the catalog can resolve the ID, else a bare
/// "anilist/30".
pub fn display_name(catalog: Option<&Catalog>, key: &EntryKey) -> String {
    if let Some(catalog) = catalog {
        if key.0 == "anilist" {
            if let Ok(Some(entry)) = catalog.find_by_id(key.1) {
                return format!("'{}' ({}/{})", entry.title, key.0, key.1);
            }
        }
    }
    format!("{}/{}", key.0, key.1)
}

/// Best-effort catalog handle for showing titles — comparison still works
/// without one (bare catalog refs), so a missing catalog is not an error.
pub fn open_catalog() -> Option<Catalog> {
    catalog_path().ok().and_then(|p| Catalog::open(&p).ok())
}

pub fn status_label(status: WatchStatus) -> &'static str {
    match status {
        WatchStatus::Dropped => "dropped",
        WatchStatus::Planning => "planning",
        WatchStatus::Watching => "watching",
        WatchStatus::Completed => "completed",
    }
}

fn field_rows(c: &Changes) -> [(&'static str, Option<String>); 4] {
    [
        ("status", c.status.map(|s| status_label(s).to_string())),
        ("episode_progress", c.episode_progress.map(|v| v.to_string())),
        ("score", c.score.map(|v| v.to_string())),
        ("rewatch_count", c.rewatch_count.map(|v| v.to_string())),
    ]
}
