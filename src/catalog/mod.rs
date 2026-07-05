//! Access layer for the local anime metadata catalog.
//!
//! Per brainstorm.md 1.5: this SQLite file is bundled with the anigit
//! package install itself (not generated at `anigit init`), text-only (no
//! images/video), and kept fresh via `anigit refresh` pulling deltas from
//! the animetaScraper VM (1.11-1.13a). User repos never store this data
//! directly — they only ever store a `CatalogRef` pointing into this file,
//! which is what makes catalog updates never conflict with repo history.

use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

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
        let rows = stmt.query_map([pattern], |row| {
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
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
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
