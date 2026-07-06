use anyhow::Result;
use std::env;

use super::compare::net_changes;
use crate::catalog::{catalog_path, Catalog};
use crate::repo::commit::CatalogRef;
use crate::repo::staging::StagedEntry;
use crate::repo::Repo;
use crate::tui;

/// `anigit add` — the ONLY anigit command with a TUI (brainstorm.md 1.7a).
/// Two sequential phases (1.7a, updated 2026-07-06):
///   1. An interactive incremental-search screen picks the anime — replaces
///      the old `<anime name>` argument, whose ambiguous-first-match
///      resolution picked wrong entries in practice.
///   2. The edit menu (status/episode/score/rewatch) — unchanged from
///      part 6, pre-populated from this anime's current state in history.
///
/// Cancelling EITHER phase stages nothing. On confirm, the result is
/// written to `.anigit/STAGED` for `anigit commit` to consume.
pub fn run() -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::discover(&cwd)?;

    let catalog = Catalog::open(&catalog_path()?)?;
    let Some(entry) = tui::search::run_search_screen(&catalog)? else {
        println!("Cancelled — nothing staged.");
        return Ok(());
    };

    // Pre-populate from prior commits touching this anime, if any — same
    // last-set-wins replay compare/merge use (brainstorm.md 1.3).
    let branch = repo.current_branch()?;
    let existing = net_changes(&repo.history(&branch)?)
        .remove(&("anilist".to_string(), entry.id));

    // entry.episodes caps the episode spinner at the show's real length
    // (None for shows without a confirmed count, e.g. currently airing).
    let Some(menu) = tui::run_add_menu(entry.display_title(), existing, entry.episodes)? else {
        println!("Cancelled — nothing staged.");
        return Ok(());
    };

    let staged = StagedEntry {
        catalog_ref: CatalogRef {
            source: "anilist".to_string(),
            id: entry.id,
        },
        anime_title: entry.display_title().to_string(),
        changes: menu.changes,
    };
    repo.write_staged(&staged)?;

    println!(
        "Staged changes for '{}' (anilist/{}).",
        staged.anime_title, staged.catalog_ref.id
    );
    println!("Use `anigit commit -m \"<message>\"` to record them.");
    Ok(())
}
