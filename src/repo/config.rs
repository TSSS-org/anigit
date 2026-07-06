//! Repo-level configuration: `.anigit/config`.
//!
//! Encodes the Shared/SingleUser toggle and visibility levels described in
//! `brainstorm.md` section 1.6. Both are stored as a flag/permission check on
//! top of an identical underlying event-log format — never a different data
//! shape per repo kind — so they stay cheap to flip at any time.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RepoKind {
    /// Multiple people can contribute/merge into this repo.
    Shared,
    /// Personal watch list. Can be forked, but only the owner can commit.
    SingleUser,
}

/// Visibility levels, modeled directly on GitHub's own repo visibility
/// options (brainstorm.md 1.6). Changeable at any time, but only by the
/// repo's owner — never by other contributors.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Visibility {
    PublicContributable,
    PublicViewOnly,
    Private,
    PrivateSharedWithSpecificPeople,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoConfig {
    pub repo_kind: RepoKind,
    pub visibility: Visibility,
    /// Local identifier for the repo owner. Real identity/auth model TBD —
    /// this is a placeholder until AniHub-style accounts exist (v2+).
    pub owner: String,
    /// Remote URLs this repo knows about (for future push/pull, v2+).
    #[serde(default)]
    pub remotes: Vec<RemoteEntry>,
    /// Fork provenance, present only on repos created by `anigit fork`
    /// (brainstorm.md 1.7a — fork is clone + lineage, for AniHub later).
    /// Optional and absent by default, so configs written before this field
    /// existed still parse unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forked_from: Option<ForkProvenance>,
}

/// Where a forked repo came from and when. `source` is a local path in v1
/// (brainstorm.md 1.8); it becomes an AniHub URL once that exists.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForkProvenance {
    pub source: String,
    pub forked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteEntry {
    pub name: String,
    pub url: String,
}

impl Default for RepoConfig {
    fn default() -> Self {
        Self {
            // New repos default to SingleUser/Private — the most conservative
            // starting point; owner can toggle either at any time (1.6).
            repo_kind: RepoKind::SingleUser,
            visibility: Visibility::Private,
            owner: "local-user".to_string(),
            remotes: Vec::new(),
            forked_from: None,
        }
    }
}
