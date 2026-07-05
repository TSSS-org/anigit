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
//!   refs/
//!     branches/<name>       → commit ID the branch currently points at
//!   objects/
//!     commits/<id>.json      → individual commit records, one file per commit
//! ```

pub mod commit;
pub mod config;

use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

use commit::Commit;
use config::RepoConfig;

pub const ANIGIT_DIR: &str = ".anigit";
const DEFAULT_BRANCH: &str = "main";

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

    fn set_branch_head(&self, branch: &str, commit_id: &str) -> Result<()> {
        fs::write(
            self.root.join("refs").join("branches").join(branch),
            commit_id,
        )?;
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
        self.set_branch_head(&commit.branch, &commit.id)?;
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
