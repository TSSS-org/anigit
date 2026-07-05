//! The `.anigit/` on-disk repository format.
//!
//! Modeled directly on git's own `.git/` folder: a directory of many small
//! files rather than one growing file, so a single corrupted or deleted file
//! only loses that one entry rather than the whole history
//! (brainstorm.md 1.3a).
//!
//! ```text
//! .anigit/
//!   HEAD                  → which branch you're on
//!   config                → repo_kind, visibility, owner info, remotes
//!   STAGED                → the one entry staged for commit (transient)
//!   refs/
//!     branches/<name>       → commit ID the branch currently points at
//!   objects/
//!     commits/<id>.json      → individual commit records, one file per commit
//! ```

pub mod commit;
pub mod config;
pub mod staging;

use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use commit::{Commit, CommitAction};
use config::RepoConfig;

pub const ANIGIT_DIR: &str = ".anigit";
const DEFAULT_BRANCH: &str = "main";

/// One line of `.anigit/logs/HEAD` — a record of a branch ref moving.
///
/// The reflog is deliberately a single appended-to JSONL file, unlike
/// `objects/commits/` (one file per commit, brainstorm.md 1.3a): it's an
/// operational audit trail, not permanent content-addressed history — the
/// same category reasoning as `.anigit/STAGED` being a single file (see
/// `staging.rs`). Losing a reflog line loses debugging info, never history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflogEntry {
    pub branch: String,
    /// Where the ref pointed before, or `None` for a branch's first commit.
    pub old_id: Option<String>,
    pub new_id: String,
    pub timestamp: DateTime<Utc>,
    /// Why the ref moved: "commit", "amend", "merge", etc.
    pub reason: String,
}

/// Ref names become file names directly under `.anigit/refs/`, so reject
/// anything that could escape that directory or produce an unusable file.
fn validate_ref_name(name: &str, kind: &str) -> Result<()> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('/')
        || name.contains('\\')
        || name.contains(char::is_whitespace)
    {
        bail!("invalid {kind} name: '{name}'");
    }
    Ok(())
}

pub struct Repo {
    /// Path to the `.anigit` directory itself (not the working directory).
    root: PathBuf,
}

impl Repo {
    /// `anigit init` — create a brand new, empty repo in `dir`.
    ///
    /// Per brainstorm.md 1.4, new repos start blank: no catalog data is
    /// copied in, entries only ever reference catalog IDs.
    pub fn init(dir: &Path) -> Result<Self> {
        let root = dir.join(ANIGIT_DIR);
        if root.exists() {
            bail!(
                "anigit repo already exists at {}",
                root.display()
            );
        }

        fs::create_dir_all(root.join("refs").join("branches"))?;
        fs::create_dir_all(root.join("objects").join("commits"))?;

        fs::write(root.join("HEAD"), DEFAULT_BRANCH)?;
        fs::write(
            root.join("config"),
            serde_json::to_string_pretty(&RepoConfig::default())?,
        )?;

        // Empty branch ref file — no commits yet.
        fs::write(root.join("refs").join("branches").join(DEFAULT_BRANCH), "")?;

        Ok(Self { root })
    }

    /// Open an existing repo, walking upward from `start_dir` the way git
    /// does, so anigit commands work from any subdirectory of a repo.
    pub fn discover(start_dir: &Path) -> Result<Self> {
        let mut current = start_dir.to_path_buf();
        loop {
            let candidate = current.join(ANIGIT_DIR);
            if candidate.is_dir() {
                return Ok(Self { root: candidate });
            }
            if !current.pop() {
                bail!("not an anigit repo (or any parent directory)");
            }
        }
    }

    pub fn config(&self) -> Result<RepoConfig> {
        let raw = fs::read_to_string(self.root.join("config"))
            .context("failed to read .anigit/config")?;
        Ok(serde_json::from_str(&raw)?)
    }

    pub fn write_config(&self, config: &RepoConfig) -> Result<()> {
        fs::write(
            self.root.join("config"),
            serde_json::to_string_pretty(config)?,
        )?;
        Ok(())
    }

    pub fn current_branch(&self) -> Result<String> {
        Ok(fs::read_to_string(self.root.join("HEAD"))?.trim().to_string())
    }

    /// The commit ID a branch currently points at, or `None` if the branch
    /// has no commits yet.
    pub fn branch_head(&self, branch: &str) -> Result<Option<String>> {
        let path = self.root.join("refs").join("branches").join(branch);
        if !path.exists() {
            bail!("no such branch: {branch}");
        }
        let contents = fs::read_to_string(path)?;
        let trimmed = contents.trim();
        if trimmed.is_empty() {
            Ok(None)
        } else {
            Ok(Some(trimmed.to_string()))
        }
    }

    /// Move a branch ref and record the move in the reflog. Every ref update
    /// goes through here, so the reflog is a complete audit trail of where
    /// each branch head has pointed.
    fn set_branch_head(&self, branch: &str, commit_id: &str, reason: &str) -> Result<()> {
        let path = self.root.join("refs").join("branches").join(branch);
        // Read the old target defensively (the ref file may not exist yet
        // when a new branch is being created).
        let old_id = fs::read_to_string(&path)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        fs::write(path, commit_id)?;
        self.append_reflog(&ReflogEntry {
            branch: branch.to_string(),
            old_id,
            new_id: commit_id.to_string(),
            timestamp: Utc::now(),
            reason: reason.to_string(),
        })
    }

    fn append_reflog(&self, entry: &ReflogEntry) -> Result<()> {
        let logs_dir = self.root.join("logs");
        fs::create_dir_all(&logs_dir)?;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(logs_dir.join("HEAD"))
            .context("failed to open .anigit/logs/HEAD")?;
        writeln!(file, "{}", serde_json::to_string(entry)?)?;
        Ok(())
    }

    /// All reflog entries, oldest first (on-disk order). Empty if no ref has
    /// ever moved (including repos created before the reflog existed).
    pub fn read_reflog(&self) -> Result<Vec<ReflogEntry>> {
        let path = self.root.join("logs").join("HEAD");
        if !path.exists() {
            return Ok(Vec::new());
        }
        let raw = fs::read_to_string(&path).context("failed to read .anigit/logs/HEAD")?;
        raw.lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| serde_json::from_str(line).context("malformed .anigit/logs/HEAD entry"))
            .collect()
    }

    /// Every branch that exists under `.anigit/refs/branches/`, sorted by
    /// name. This is the branch-enumeration piece `log --graph` was missing
    /// in part 2 of the build.
    pub fn list_branches(&self) -> Result<Vec<String>> {
        let dir = self.root.join("refs").join("branches");
        let mut branches = Vec::new();
        for entry in fs::read_dir(&dir).context("failed to read .anigit/refs/branches")? {
            if let Some(name) = entry?.file_name().to_str() {
                branches.push(name.to_string());
            }
        }
        branches.sort();
        Ok(branches)
    }

    pub fn branch_exists(&self, name: &str) -> bool {
        self.root.join("refs").join("branches").join(name).is_file()
    }

    /// Create a new branch pointing at the current branch's head commit —
    /// same as real git, the new branch starts as a copy of wherever you
    /// are now.
    pub fn create_branch(&self, name: &str) -> Result<()> {
        validate_ref_name(name, "branch")?;
        if self.branch_exists(name) {
            bail!("branch '{name}' already exists");
        }
        match self.branch_head(&self.current_branch()?)? {
            // Goes through set_branch_head so branch creation lands in the
            // reflog like any other ref update — "where did this branch
            // start" is exactly the kind of question the reflog answers.
            Some(head) => self.set_branch_head(name, &head, "branch created")?,
            // No commits yet: the new branch starts with an empty ref file,
            // the same way `init` creates the default branch. No reflog
            // entry — the ref isn't pointing at anything, so nothing moved.
            None => fs::write(self.root.join("refs").join("branches").join(name), "")?,
        }
        Ok(())
    }

    /// Switch which branch HEAD points at. This only rewrites `.anigit/HEAD`
    /// (which branch you're on) — it never moves any branch's commit
    /// pointer; that's `set_branch_head`'s job.
    pub fn set_current_branch(&self, name: &str) -> Result<()> {
        if !self.branch_exists(name) {
            bail!("no such branch: {name}");
        }
        fs::write(self.root.join("HEAD"), name)?;
        Ok(())
    }

    /// Create a tag ref at `.anigit/refs/tags/<name>` pointing at `commit_id`.
    /// Tags mark a specific point permanently (brainstorm.md section 2,
    /// milestones), so unlike branches they are never silently overwritten.
    pub fn create_tag(&self, name: &str, commit_id: &str) -> Result<()> {
        validate_ref_name(name, "tag")?;
        let dir = self.root.join("refs").join("tags");
        fs::create_dir_all(&dir)?;
        let path = dir.join(name);
        if path.exists() {
            bail!("tag '{name}' already exists — tags are permanent markers and can't be overwritten");
        }
        fs::write(path, commit_id)?;
        Ok(())
    }

    /// Write a new commit object to disk and advance the current branch to
    /// point at it. This is the only way commits get written — history is
    /// append-only (brainstorm.md 1.3): nothing here ever overwrites an
    /// existing commit file.
    pub fn write_commit(&self, commit: &Commit) -> Result<()> {
        let path = self
            .root
            .join("objects")
            .join("commits")
            .join(format!("{}.json", commit.id));
        if path.exists() {
            bail!("commit {} already exists (id collision?)", commit.id);
        }
        fs::write(path, serde_json::to_string_pretty(commit)?)?;

        // Reflog reason, derived rather than passed in: a merge commit says
        // so itself; a normal commit chains onto the old head; if the old
        // head is NOT among the new commit's parents, the branch tip is
        // being replaced sideways — that's an amend (see commands/commit.rs).
        let old_head = self.branch_head(&commit.branch).ok().flatten();
        let reason = match commit.action {
            CommitAction::Merge => "merge",
            CommitAction::Commit => match &old_head {
                Some(h) if !commit.parent_ids.contains(h) => "amend",
                _ => "commit",
            },
        };
        self.set_branch_head(&commit.branch, &commit.id, reason)?;
        Ok(())
    }

    pub fn read_commit(&self, id: &str) -> Result<Commit> {
        let path = self
            .root
            .join("objects")
            .join("commits")
            .join(format!("{id}.json"));
        let raw = fs::read_to_string(&path)
            .with_context(|| format!("no such commit: {id}"))?;
        let commit: Commit = serde_json::from_str(&raw)?;
        if commit.schema_version > commit::CURRENT_SCHEMA_VERSION {
            bail!(
                "commit {id} was written by a newer version of anigit \
                 (schema_version {}); please upgrade anigit",
                commit.schema_version
            );
        }
        Ok(commit)
    }

    /// Walk a branch's history from its tip back to the root, following
    /// `parent_ids[0]` (the first-parent chain — same convention as `git log`
    /// on a merge-heavy history).
    pub fn history(&self, branch: &str) -> Result<Vec<Commit>> {
        let mut history = Vec::new();
        let mut current = self.branch_head(branch)?;
        while let Some(id) = current {
            let commit = self.read_commit(&id)?;
            current = commit.parent_ids.first().cloned();
            history.push(commit);
        }
        Ok(history)
    }
}
