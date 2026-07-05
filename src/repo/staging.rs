//! The staging area: `.anigit/STAGED`.
//!
//! Placed under `repo/` (rather than inline in the command files) because the
//! staging file is part of the on-disk `.anigit/` format, exactly like HEAD,
//! config, and refs — three separate commands (add, commit, status, diff)
//! all read it, so it belongs next to the rest of the repo format code
//! instead of being owned by any one command.
//!
//! Unlike commits (one immutable file each, brainstorm.md 1.3a), the staging
//! area is a single mutable JSON file. That's deliberate: it's transient
//! working state, not permanent history — it holds at most ONE anime entry's
//! pending changes at a time (per brainstorm.md 1.7a, `anigit add <name>`
//! stages one entry, `anigit commit -m "..."` consumes it), and it's deleted
//! the moment a commit lands. The append-only guarantees in 1.3 only apply
//! to history, and this never is history.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use super::commit::{CatalogRef, Changes};
use super::Repo;

const STAGED_FILE: &str = "STAGED";

/// The one entry currently staged for commit, if any.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagedEntry {
    /// Which catalog entry these changes apply to — this is what ends up in
    /// the commit's `catalog_ref`.
    pub catalog_ref: CatalogRef,
    /// Title snapshot at stage time, kept ONLY for display in `status`/`diff`
    /// output. Never authoritative — the catalog owns metadata (1.4).
    pub anime_title: String,
    /// The changes collected from the `anigit add` TUI.
    pub changes: Changes,
}

impl Repo {
    fn staged_path(&self) -> PathBuf {
        self.root.join(STAGED_FILE)
    }

    /// Read the staged entry, or `None` if nothing is staged.
    pub fn read_staged(&self) -> Result<Option<StagedEntry>> {
        let path = self.staged_path();
        if !path.exists() {
            return Ok(None);
        }
        let raw = fs::read_to_string(&path).context("failed to read .anigit/STAGED")?;
        let entry = serde_json::from_str(&raw).context("failed to parse .anigit/STAGED")?;
        Ok(Some(entry))
    }

    /// Stage an entry, replacing anything previously staged (only one anime
    /// entry can be staged at a time — brainstorm.md 1.7a).
    pub fn write_staged(&self, entry: &StagedEntry) -> Result<()> {
        fs::write(self.staged_path(), serde_json::to_string_pretty(entry)?)
            .context("failed to write .anigit/STAGED")?;
        Ok(())
    }

    /// Delete the staging file after its contents have been committed.
    pub fn clear_staged(&self) -> Result<()> {
        let path = self.staged_path();
        if path.exists() {
            fs::remove_file(&path).context("failed to remove .anigit/STAGED")?;
        }
        Ok(())
    }
}
