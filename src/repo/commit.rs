//! Core data types for anigit commits.
//!
//! Mirrors the draft schema in `brainstorm.md` section 1.3a. `schema_version`
//! is checked before parsing any commit so future format changes never break
//! old commits — this is the field that makes the format "stay forever."

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Bump this only when making a breaking change to the commit format.
/// Non-breaking additions should go in `metadata` instead of bumping this.
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// A reference to an entry in the shared, read-only anime catalog.
/// Deliberately a struct (not a bare ID) so a second metadata source can be
/// added later without migrating old commits — see brainstorm.md 1.4 / 1.5.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CatalogRef {
    /// Which catalog this ID belongs to, e.g. "anilist" or a future
    /// self-hosted fallback source.
    pub source: String,
    /// The catalog's own ID for this anime.
    pub id: i64,
}

/// Fields a single commit can change. All fields are optional because a
/// commit only records what actually changed — current state is derived by
/// replaying every commit in order (append-only event log, brainstorm.md 1.3).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Changes {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<WatchStatus>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub episode_progress: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rewatch_count: Option<u32>,
}

/// Watch status. Order matters for merge auto-resolution of "how far along"
/// comparisons — see brainstorm.md 1.7 (Option B: objective fields resolve
/// via a deterministic rule).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum WatchStatus {
    Dropped,
    Planning,
    Watching,
    Completed,
}

/// A single commit — the atomic unit of anigit history. One file per commit
/// under `.anigit/objects/commits/<id>.json` (brainstorm.md 1.3a).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Commit {
    pub schema_version: u32,
    /// Unique commit ID, e.g. "c_9f2a1b7e".
    pub id: String,
    pub timestamp: DateTime<Utc>,
    /// Parent commit ID(s). A normal commit has one parent; a merge commit
    /// has two. The very first commit in a repo has an empty list.
    pub parent_ids: Vec<String>,
    pub branch: String,
    pub author: String,
    pub action: CommitAction,
    pub catalog_ref: CatalogRef,
    pub changes: Changes,
    pub message: String,
    /// Open-ended catch-all for future fields that don't need a schema
    /// version bump. Keep this as the pressure-release valve, not a dumping
    /// ground for things that should be first-class fields.
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommitAction {
    Commit,
    Merge,
}

impl Commit {
    /// Construct a new commit with a freshly generated ID and current
    /// timestamp. Callers fill in the rest.
    pub fn new(
        parent_ids: Vec<String>,
        branch: impl Into<String>,
        catalog_ref: CatalogRef,
        changes: Changes,
        message: impl Into<String>,
    ) -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            id: format!("c_{}", uuid::Uuid::new_v4().simple()),
            timestamp: Utc::now(),
            parent_ids,
            branch: branch.into(),
            // TODO: replace with real local-user identity once that's decided.
            author: "local-user".to_string(),
            action: CommitAction::Commit,
            catalog_ref,
            changes,
            message: message.into(),
            metadata: HashMap::new(),
        }
    }
}
