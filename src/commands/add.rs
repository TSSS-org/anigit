use anyhow::Result;
use std::env;

use crate::catalog::{catalog_path, resolve_by_name, Catalog};
use crate::repo::commit::CatalogRef;
use crate::repo::staging::StagedEntry;
use crate::repo::Repo;
use crate::tui;

/// `anigit add <anime name>` — the ONLY anigit command with an interactive
/// TUI (brainstorm.md 1.7a). Opens a menu for editing status/episode/score/
/// comments before a subsequent `anigit commit -m "..."` finalizes it.
///
/// Flow: look up the name in the local catalog cache, launch the add menu
/// (`crate::tui::run_add_menu` — currently a stub returning default
/// `Changes`; the real ratatui menu is a separate build part), then write
/// the result to `.anigit/STAGED` for `anigit commit` to consume.
pub fn run(anime_name: &str) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::discover(&cwd)?;

    let catalog = Catalog::open(&catalog_path()?)?;
    let entry = resolve_by_name(&catalog, anime_name)?;

    let menu = tui::run_add_menu(&entry.title, None)?;

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
