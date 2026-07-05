//! Access layer for the local anime metadata catalog.
//!
//! Per brainstorm.md 1.5: this SQLite file is bundled with the anigit
//! package install itself (not generated at `anigit init`), text-only (no
//! images/video), and kept fresh via `anigit refresh` pulling deltas from
//! the animetaScraper VM (1.11-1.13a). User repos never store this data
//! directly — they only ever store a `CatalogRef` pointing into this file,
//! which is what makes catalog updates never conflict with repo history.

use anyhow::{bail, Result};
use rusqlite::Connection;
use std::env;
use std::path::{Path, PathBuf};

/// Where the bundled/synced catalog SQLite file lives. Per brainstorm.md 1.5
/// it ships alongside the binary; `ANIGIT_CATALOG` overrides it for
/// development and testing. (Extracted from commands/add.rs in part 4 —
/// add, blame, compare, and merge all open the catalog.)
pub fn catalog_path() -> Result<PathBuf> {
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

/// Resolve a user-typed name to exactly one catalog entry: clear error on
/// zero matches; on multiple matches, print a note and take the first.
/// (Extracted from commands/add.rs in part 4 — shared by add and blame so
/// the zero/one/many handling stays identical everywhere.)
pub fn resolve_by_name(catalog: &Catalog, name: &str) -> Result<CatalogEntry> {
    let mut matches = catalog.find_by_name(name)?;
    if matches.is_empty() {
        bail!(
            "no catalog entry found matching '{name}'.\n\
             The local catalog may be stale — try `anigit refresh`, or check \
             the spelling."
        );
    }
    if matches.len() > 1 {
        // TODO: proper ambiguity picker UI (a later build part). For now,
        // take the first match and be up front about it.
        println!(
            "note: {} catalog entries match '{name}'; using the first \
             ('{}'). Ambiguous matching will be improved later.",
            matches.len(),
            matches[0].title
        );
    }
    Ok(matches.remove(0))
}

#[derive(Debug, Clone)]
pub struct CatalogEntry {
    pub id: i64,
    pub title: String,
    pub format: Option<String>,
    pub episodes: Option<u32>,
    pub description: Option<String>,
    pub status: AiringStatus,
    // TODO: air dates, genres/tags — schema fields per brainstorm.md 1.11
    // scoping (fields we actually want from AniList).
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiringStatus {
    Releasing,
    Finished,
    NotYetReleased,
}

/// Shared row mapping for the `SELECT id, title, format, episodes,
/// description, status` column order used by every query above.
fn entry_from_row(row: &rusqlite::Row) -> rusqlite::Result<CatalogEntry> {
    let status_str: String = row.get(5)?;
    Ok(CatalogEntry {
        id: row.get(0)?,
        title: row.get(1)?,
        format: row.get(2)?,
        episodes: row.get(3)?,
        description: row.get(4)?,
        status: match status_str.as_str() {
            "RELEASING" => AiringStatus::Releasing,
            "FINISHED" => AiringStatus::Finished,
            _ => AiringStatus::NotYetReleased,
        },
    })
}

pub struct Catalog {
    conn: Connection,
}

impl Catalog {
    /// Open the bundled/synced catalog SQLite file.
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        Ok(Self { conn })
    }

    /// Create the schema, for use by animetaScraper or first-run setup.
    /// TODO: finalize column list to match brainstorm.md 1.11 scoping
    /// exactly (ID, title, format, episode count, description, status, air
    /// dates, genres/tags — no characters/staff/studios/social links).
    pub fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS anime (
                id INTEGER PRIMARY KEY,
                title TEXT NOT NULL,
                format TEXT,
                episodes INTEGER,
                description TEXT,
                status TEXT NOT NULL,
                last_updated TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS sync_state (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );",
        )?;
        Ok(())
    }

    /// Fuzzy-ish lookup by title for `anigit add <anime name>`.
    /// TODO: real fuzzy matching (currently exact/LIKE substring only).
    pub fn find_by_name(&self, name: &str) -> Result<Vec<CatalogEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, format, episodes, description, status
             FROM anime WHERE title LIKE ?1 LIMIT 20",
        )?;
        let pattern = format!("%{name}%");
        let rows = stmt.query_map([pattern], entry_from_row)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
    }

    /// Single-entry lookup by catalog ID — lets `compare`/`merge` show a
    /// title next to a bare `CatalogRef` when the catalog is available.
    pub fn find_by_id(&self, id: i64) -> Result<Option<CatalogEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, format, episodes, description, status
             FROM anime WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map([id], entry_from_row)?;
        rows.next().transpose().map_err(Into::into)
    }

    /// The staleness check from brainstorm.md 1.12: only `Releasing` entries
    /// ever need freshness checks against the 6-day threshold; `Finished`
    /// entries never need re-fetching.
    pub fn is_stale(&self, _entry: &CatalogEntry) -> bool {
        // TODO: compare last_updated column against now() - 6 days, but
        // only for AiringStatus::Releasing entries.
        false
    }
}
