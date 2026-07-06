//! Access layer for the local anime metadata catalog.
//!
//! Per brainstorm.md 1.5: this SQLite file is bundled with the anigit
//! package install itself (not generated at `anigit init`), text-only (no
//! images/video), and kept fresh via `anigit refresh` pulling deltas from
//! the animetaScraper VM (1.11-1.13a). User repos never store this data
//! directly — they only ever store a `CatalogRef` pointing into this file,
//! which is what makes catalog updates never conflict with repo history.

use anyhow::{bail, Result};
use chrono::{DateTime, Duration, Utc};
use rusqlite::{Connection, OptionalExtension};
use std::env;
use std::path::{Path, PathBuf};

/// Where the bundled/synced catalog SQLite file WILL live, without requiring
/// it to exist yet — `anigit refresh` uses this to bootstrap an empty
/// catalog on first sync. Everything else should use `catalog_path()`.
pub fn catalog_path_for_sync() -> Result<PathBuf> {
    match env::var_os("ANIGIT_CATALOG") {
        Some(p) => Ok(PathBuf::from(p)),
        None => {
            let exe = env::current_exe()?;
            match exe.parent() {
                Some(dir) => Ok(dir.join("animeta.sqlite")),
                None => bail!("could not determine the anigit install directory"),
            }
        }
    }
}

/// Where the bundled/synced catalog SQLite file lives. Per brainstorm.md 1.5
/// it ships alongside the binary; `ANIGIT_CATALOG` overrides it for
/// development and testing. (Extracted from commands/add.rs in part 4 —
/// add, blame, compare, and merge all open the catalog.)
pub fn catalog_path() -> Result<PathBuf> {
    let path = catalog_path_for_sync()?;
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

/// One catalog row — the full 1.11-scoped field set: ID, title, format,
/// episode count, description, status, air dates, genres/tags. Explicitly
/// NO characters/staff/studios/social links and no images (finalized
/// scoping, brainstorm.md 1.11).
#[derive(Debug, Clone)]
pub struct CatalogEntry {
    pub id: i64,
    pub title: String,
    pub format: Option<String>,
    pub episodes: Option<u32>,
    pub description: Option<String>,
    pub status: AiringStatus,
    /// ISO date strings ("2026-01-08") — a real date type isn't needed for v1.
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub genres: Vec<String>,
    /// RFC3339 timestamp of when this row was last synced/written — the
    /// input to the staleness check (brainstorm.md 1.12).
    pub last_updated: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiringStatus {
    Releasing,
    Finished,
    NotYetReleased,
}

/// Column list shared by every entry query, in `entry_from_row` order.
const ENTRY_COLUMNS: &str =
    "id, title, format, episodes, description, status, start_date, end_date, \
     genres_json, last_updated";

/// Shared row mapping for the `ENTRY_COLUMNS` column order used by every
/// query above.
fn entry_from_row(row: &rusqlite::Row) -> rusqlite::Result<CatalogEntry> {
    let status_str: String = row.get(5)?;
    let genres_json: Option<String> = row.get(8)?;
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
        start_date: row.get(6)?,
        end_date: row.get(7)?,
        // Genres live in one JSON-encoded text column (SQLite has no array
        // type; a separate genres table would be over-engineering for v1).
        genres: genres_json
            .as_deref()
            .and_then(|j| serde_json::from_str(j).ok())
            .unwrap_or_default(),
        last_updated: row.get(9)?,
    })
}

/// Delta `fields` keys → catalog column names (brainstorm.md 1.13a delta
/// shape). Also the whitelist: anything else in a delta is a schema
/// mismatch, not something to guess about.
const DELTA_FIELDS: &[(&str, &str)] = &[
    ("title", "title"),
    ("format", "format"),
    ("episodes", "episodes"),
    ("description", "description"),
    ("status", "status"),
    ("start_date", "start_date"),
    ("end_date", "end_date"),
    ("genres", "genres_json"),
];

fn column_for_field(field: &str) -> Result<&'static str> {
    match DELTA_FIELDS.iter().find(|(f, _)| *f == field) {
        Some((_, col)) => Ok(col),
        None => bail!(
            "unknown field '{field}' in catalog delta — this delta may come \
             from a newer animetaScraper; please upgrade anigit"
        ),
    }
}

/// JSON delta value → SQLite value. Arrays (genres) are stored JSON-encoded.
fn sql_value(field: &str, value: &serde_json::Value) -> Result<rusqlite::types::Value> {
    use rusqlite::types::Value as Sql;
    Ok(match value {
        serde_json::Value::Null => Sql::Null,
        serde_json::Value::String(s) => Sql::Text(s.clone()),
        serde_json::Value::Number(n) => match n.as_i64() {
            Some(i) => Sql::Integer(i),
            None => Sql::Real(n.as_f64().unwrap_or_default()),
        },
        serde_json::Value::Array(_) => Sql::Text(serde_json::to_string(value)?),
        other => bail!("unsupported value {other} for field '{field}' in catalog delta"),
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
    /// Column list matches brainstorm.md 1.11's finalized scoping exactly:
    /// ID, title, format, episode count, description, status, air dates,
    /// genres/tags — no characters/staff/studios/social links.
    pub fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS anime (
                id INTEGER PRIMARY KEY,
                title TEXT NOT NULL,
                format TEXT,
                episodes INTEGER,
                description TEXT,
                status TEXT NOT NULL,
                start_date TEXT,
                end_date TEXT,
                genres_json TEXT,
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
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {ENTRY_COLUMNS} FROM anime WHERE title LIKE ?1 LIMIT 20"
        ))?;
        let pattern = format!("%{name}%");
        let rows = stmt.query_map([pattern], entry_from_row)?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
    }

    /// Single-entry lookup by catalog ID — lets `compare`/`merge` show a
    /// title next to a bare `CatalogRef` when the catalog is available.
    pub fn find_by_id(&self, id: i64) -> Result<Option<CatalogEntry>> {
        let mut stmt = self.conn.prepare(&format!(
            "SELECT {ENTRY_COLUMNS} FROM anime WHERE id = ?1"
        ))?;
        let mut rows = stmt.query_map([id], entry_from_row)?;
        rows.next().transpose().map_err(Into::into)
    }

    /// The staleness check from brainstorm.md 1.12: only `Releasing` entries
    /// ever need freshness checks against the 6-day threshold — anything
    /// `Finished` (or not yet released) never changes weekly, so the check
    /// stays a cheap local status+timestamp lookup for most of the catalog.
    pub fn is_stale(&self, entry: &CatalogEntry) -> bool {
        if entry.status != AiringStatus::Releasing {
            return false;
        }
        match DateTime::parse_from_rfc3339(&entry.last_updated) {
            Ok(t) => Utc::now() - t.with_timezone(&Utc) > Duration::days(6),
            // An unparseable timestamp can't prove freshness — treat as
            // stale, the safe direction.
            Err(_) => true,
        }
    }

    /// The manifest run number this catalog was last synced to (brainstorm.md
    /// 1.13a step 2), or `None` if it has never synced.
    pub fn last_synced_run(&self) -> Result<Option<u64>> {
        let value: Option<String> = self
            .conn
            .query_row(
                "SELECT value FROM sync_state WHERE key = 'last_synced_run'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        Ok(value.and_then(|v| v.parse().ok()))
    }

    pub fn set_last_synced_run(&self, run: u64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sync_state (key, value) VALUES ('last_synced_run', ?1)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [run.to_string()],
        )?;
        Ok(())
    }

    /// Apply one `op: "insert"` delta change (brainstorm.md 1.13a).
    pub fn insert_from_delta(
        &self,
        id: i64,
        fields: &serde_json::Map<String, serde_json::Value>,
        last_updated: &str,
    ) -> Result<()> {
        for required in ["title", "status"] {
            if !fields.contains_key(required) {
                bail!("malformed delta: insert for id {id} is missing required field '{required}'");
            }
        }
        let mut columns = vec!["id".to_string(), "last_updated".to_string()];
        let mut params: Vec<rusqlite::types::Value> =
            vec![id.into(), last_updated.to_string().into()];
        for (field, value) in fields {
            columns.push(column_for_field(field)?.to_string());
            params.push(sql_value(field, value)?);
        }
        let placeholders: Vec<String> = (1..=params.len()).map(|i| format!("?{i}")).collect();
        self.conn.execute(
            &format!(
                "INSERT INTO anime ({}) VALUES ({})",
                columns.join(", "),
                placeholders.join(", ")
            ),
            rusqlite::params_from_iter(params),
        )?;
        Ok(())
    }

    /// Apply one `op: "update"` delta change (brainstorm.md 1.13a). Returns
    /// false if no row with this id exists to update.
    pub fn update_from_delta(
        &self,
        id: i64,
        fields: &serde_json::Map<String, serde_json::Value>,
        last_updated: &str,
    ) -> Result<bool> {
        let mut assignments = vec!["last_updated = ?1".to_string()];
        let mut params: Vec<rusqlite::types::Value> = vec![last_updated.to_string().into()];
        for (field, value) in fields {
            params.push(sql_value(field, value)?);
            assignments.push(format!("{} = ?{}", column_for_field(field)?, params.len()));
        }
        params.push(id.into());
        let changed = self.conn.execute(
            &format!(
                "UPDATE anime SET {} WHERE id = ?{}",
                assignments.join(", "),
                params.len()
            ),
            rusqlite::params_from_iter(params),
        )?;
        Ok(changed > 0)
    }

    /// Replace the local `anime` table's contents wholesale from a checkpoint
    /// snapshot `.sqlite` (brainstorm.md 1.13a step 3). Returns the number of
    /// rows the catalog now holds. `sync_state` is deliberately untouched —
    /// it's local client state, not catalog content.
    pub fn replace_from_checkpoint(&self, checkpoint: &Path) -> Result<usize> {
        let Some(path) = checkpoint.to_str() else {
            bail!("checkpoint path is not valid UTF-8: {}", checkpoint.display());
        };
        self.conn
            .execute("ATTACH DATABASE ?1 AS checkpoint", [path])?;
        let result = (|| -> Result<usize> {
            self.conn.execute("DELETE FROM anime", [])?;
            let copied = self.conn.execute(
                &format!(
                    "INSERT INTO anime ({ENTRY_COLUMNS}) \
                     SELECT {ENTRY_COLUMNS} FROM checkpoint.anime"
                ),
                [],
            )?;
            Ok(copied)
        })();
        self.conn.execute("DETACH DATABASE checkpoint", [])?;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(status: AiringStatus, last_updated: String) -> CatalogEntry {
        CatalogEntry {
            id: 1,
            title: "test".into(),
            format: None,
            episodes: None,
            description: None,
            status,
            start_date: None,
            end_date: None,
            genres: Vec::new(),
            last_updated,
        }
    }

    #[test]
    fn stale_only_when_releasing_and_older_than_six_days() {
        let catalog = Catalog::open(Path::new(":memory:")).unwrap();
        let old = (Utc::now() - Duration::days(7)).to_rfc3339();
        let fresh = (Utc::now() - Duration::days(1)).to_rfc3339();

        assert!(catalog.is_stale(&entry(AiringStatus::Releasing, old.clone())));
        assert!(!catalog.is_stale(&entry(AiringStatus::Releasing, fresh)));
        // Finished entries are never stale, whatever their timestamp (1.12).
        assert!(!catalog.is_stale(&entry(AiringStatus::Finished, old)));
        // Unparseable timestamps can't prove freshness.
        assert!(catalog.is_stale(&entry(AiringStatus::Releasing, "garbage".into())));
    }
}
