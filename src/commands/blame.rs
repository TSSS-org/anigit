use anyhow::{bail, Result};
use std::env;
use std::path::PathBuf;

use crate::catalog::Catalog;
use crate::repo::commit::{Changes, Commit, WatchStatus};
use crate::repo::Repo;

/// `anigit blame <anime name>` — for each `Changes` field, show which commit
/// most recently set it for the given anime. Deliberately simple (walks the
/// current branch's first-parent chain only) — blame is a low-priority,
/// nice-to-have feature per brainstorm.md section 2.
pub fn run(anime_name: &str) -> Result<()> {
    let cwd = env::current_dir()?;
    let repo = Repo::discover(&cwd)?;

    // Same catalog lookup + ambiguous-match handling as `anigit add`
    // (commands/add.rs) — duplicated here because add.rs is finalized and
    // keeps its helpers private.
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
        println!(
            "note: {} catalog entries match '{anime_name}'; using the first \
             ('{}'). Ambiguous matching will be improved later.",
            matches.len(),
            matches[0].title
        );
    }
    let entry = matches.remove(0);

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
            entry.title, entry.id
        );
        return Ok(());
    }

    println!("Blame for '{}' (anilist/{}) on branch {branch}:", entry.title, entry.id);
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

/// Catalog file resolution — same rule as `anigit add` (commands/add.rs):
/// `ANIGIT_CATALOG` env override, else `animeta.sqlite` next to the binary
/// (brainstorm.md 1.5).
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
            "anime catalog not found at {} — set ANIGIT_CATALOG to point at \
             a catalog file, or run `anigit refresh` once catalog sync is \
             available.",
            path.display()
        );
    }
    Ok(path)
}
