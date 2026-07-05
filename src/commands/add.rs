use anyhow::{bail, Result};
use std::env;
use std::path::PathBuf;

use crate::catalog::Catalog;
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
    let mut matches = catalog.find_by_name(anime_name)?;
    if matches.is_empty() {
        bail!(
            "no catalog entry found matching '{anime_name}'.\n\
             The local catalog may be stale — try `anigit refresh`, or check \
             the spelling."
        );
    }
    if matches.len() > 1 {
        // TODO: proper ambiguity picker UI (a later build part). For now,
        // take the first match and be up front about it.
        println!(
            "note: {} catalog entries match '{anime_name}'; using the first \
             ('{}'). Ambiguous matching will be improved later.",
            matches.len(),
            matches[0].title
        );
    }
    let entry = matches.remove(0);

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

/// Where the bundled/synced catalog SQLite file lives. Per brainstorm.md 1.5
/// it ships alongside the binary; `ANIGIT_CATALOG` overrides it for
/// development and testing.
fn catalog_path() -> Result<PathBuf> {
    let path = match env::var_os("ANIGIT_CATALOG") {
        Some(p) => PathBuf::from(p),
        None => {
            let exe = env::current_exe()?;
            match exe.parent() {
                Some(dir) => dir.join("animeta.sqlite"),
                None => bail!("could not determine the anigit install directory"),
            }
        }
    };
    if !path.is_file() {
        bail!(
            "anime catalog not found at {} — the catalog SQLite file should \
             ship with the anigit install (brainstorm.md 1.5). Set \
             ANIGIT_CATALOG to point at a catalog file, or run \
             `anigit refresh` once catalog sync is available.",
            path.display()
        );
    }
    Ok(path)
}
