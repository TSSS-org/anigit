use anyhow::Result;
use std::env;

use super::compare::net_changes;
use crate::catalog::{catalog_path, resolve_by_name, Catalog};
use crate::repo::commit::CatalogRef;
use crate::repo::staging::StagedEntry;
use crate::repo::Repo;
use crate::tui;

/// `anigit add <anime name>` — the ONLY anigit command with an interactive
/// TUI (brainstorm.md 1.7a). Opens a menu for editing status/episode/score
/// before a subsequent `anigit commit -m "..."` finalizes it.
///
/// Flow: look up the name in the local catalog cache, derive this anime's
/// current state from the branch history so the menu opens pre-populated,
/// launch the add menu, then write the result to `.anigit/STAGED` for
/// `anigit commit` to consume — unless the user cancelled, in which case
/// nothing is staged.
pub fn run(anime_name: &str) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::discover(&cwd)?;

    let catalog = Catalog::open(&catalog_path()?)?;
    let entry = resolve_by_name(&catalog, anime_name)?;

    // Pre-populate from prior commits touching this anime, if any — same
    // last-set-wins replay compare/merge use (brainstorm.md 1.3).
    let branch = repo.current_branch()?;
    let existing = net_changes(&repo.history(&branch)?)
        .remove(&("anilist".to_string(), entry.id));

    let Some(menu) = tui::run_add_menu(&entry.title, existing)? else {
        println!("Cancelled — nothing staged.");
        return Ok(());
    };

    let staged = StagedEntry {
        catalog_ref: CatalogRef {
            source: "anilist".to_string(),
            id: entry.id,
        },
        anime_title: entry.title,
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
