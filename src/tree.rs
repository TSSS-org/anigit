//! Generated working-directory folder-tree view (brainstorm.md 1.16).
//!
//! A READ-ONLY, fully-disposable VIEW of current watch state, written
//! alongside `.anigit/` so an anigit repo has real, browsable files the way
//! a git repo does:
//!
//! ```text
//! watching/
//!   README                       ← "do not edit" warning
//!   Cowboy Bebop/
//!     Season 1/
//!       1  2  ...  12            ← plain-text leaf files, one per episode
//! completed/ dropped/ planning/  ← same shape
//! ```
//!
//! Never a second data store: derived entirely by replaying commit history
//! (`compare::net_changes`), never read by any anigit command, and rebuilt
//! FROM SCRATCH on every regeneration — full wipe-and-rewrite was
//! deliberately chosen over incremental diffing (1.16, "Regeneration
//! strategy"); it's idempotent and personal-scale repos make churn a
//! non-issue.
//!
//! Leaf files are `1..=episode_progress`: anigit tracks a single
//! progress number, not per-episode watch events, so the files represent
//! "episodes progressed through up to the current point" — the only
//! derivation the data model supports. `Season N` is always `Season 1`
//! (consistent depth beats special-casing; anigit has no per-season
//! episode numbering).

use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::catalog::{Catalog, CatalogEntry};
use crate::commands::compare::{net_changes, open_catalog, status_label, EntryKey};
use crate::repo::commit::WatchStatus;
use crate::repo::Repo;

/// The four top-level folders. These ARE the generated tree in its
/// entirety — wiping exactly these before a rebuild is safe without
/// guessing about anything else living in the working directory.
const STATUSES: [WatchStatus; 4] = [
    WatchStatus::Watching,
    WatchStatus::Completed,
    WatchStatus::Dropped,
    WatchStatus::Planning,
];

const README_NAME: &str = "README";
const README_TEXT: &str = "\
ANIGIT GENERATED VIEW — DO NOT EDIT ANYTHING IN THESE FOLDERS.

The watching/, completed/, dropped/, and planning/ folders are a read-only
VIEW that anigit generates from your commit history (.anigit/objects/).
They are fully DELETED AND REBUILT after every `anigit commit` — any manual
change here will be silently wiped on your next commit.

The append-only event log inside .anigit/ is the only source of truth.
To change what you see here, use `anigit add` + `anigit commit`.
";

/// Rebuild the whole tree from the current branch's replayed history.
pub fn regenerate(repo: &Repo) -> Result<()> {
    let branch = repo.current_branch()?;
    let state = net_changes(&repo.history(&branch)?);
    let work_dir = repo
        .work_dir()
        .context("cannot determine the repo's working directory")?;
    // Best-effort catalog for titles/episode totals — a missing catalog
    // must not break commits; entries fall back to bare catalog refs.
    let catalog = open_catalog();

    // Full rebuild: wipe the four status dirs (clearing our own read-only
    // flags first — required on Windows, harmless on Unix).
    for status in STATUSES {
        let dir = work_dir.join(status_label(status));
        if dir.exists() {
            make_writable(&dir)?;
            fs::remove_dir_all(&dir)
                .with_context(|| format!("failed to remove old {}", dir.display()))?;
        }
    }

    let mut keys: Vec<&EntryKey> = state.keys().collect();
    keys.sort();

    // Distinct sanitized names per status dir — two titles that sanitize to
    // the same folder name would otherwise silently merge.
    let mut used: HashSet<(&'static str, String)> = HashSet::new();
    let mut files: Vec<PathBuf> = Vec::new();

    for key in keys {
        let changes = &state[key];
        // An anime whose commits never set `status` has no folder to live
        // under — skipped entirely rather than invented into some default
        // status it was never given.
        let Some(status) = changes.status else {
            continue;
        };
        let label = status_label(status);

        let entry = catalog_entry(catalog.as_ref(), key);
        let title = entry
            .as_ref()
            .map(|e| e.display_title().to_string())
            .unwrap_or_else(|| format!("{}-{}", key.0, key.1));
        let mut folder = sanitize_component(&title);
        if !used.insert((label, folder.clone())) {
            folder = format!("{folder} (anilist-{})", key.1);
            used.insert((label, folder.clone()));
        }

        // Always `Season 1` — see module doc comment.
        let season_dir = work_dir.join(label).join(&folder).join("Season 1");
        fs::create_dir_all(&season_dir)?;

        // Files 1..=progress; none yet for unset/0 progress, but the anime
        // folder above still exists so it's browsable under its status.
        let total = entry.as_ref().and_then(|e| e.episodes);
        for episode in 1..=changes.episode_progress.unwrap_or(0) {
            let path = season_dir.join(episode.to_string());
            fs::write(&path, leaf_content(&title, episode, total, key))?;
            files.push(path);
        }
    }

    // A warning README in every status folder that got generated.
    for status in STATUSES {
        let dir = work_dir.join(status_label(status));
        if dir.is_dir() {
            let path = dir.join(README_NAME);
            fs::write(&path, README_TEXT)?;
            files.push(path);
        }
    }

    // Best-effort read-only (1.16: a deterrent, not a guarantee — users can
    // chmod back, and Windows read-only attributes behave differently from
    // Unix mode bits). Files only: read-only DIRECTORIES are far less
    // consistent across OSes and would complicate our own next wipe.
    for file in &files {
        set_readonly(file).ok();
    }

    Ok(())
}

fn catalog_entry(catalog: Option<&Catalog>, key: &EntryKey) -> Option<CatalogEntry> {
    let catalog = catalog?;
    if key.0 != "anilist" {
        return None;
    }
    catalog.find_by_id(key.1).ok().flatten()
}

fn leaf_content(title: &str, episode: u32, total: Option<u32>, key: &EntryKey) -> String {
    format!(
        "# auto-generated by anigit — do not edit; deleted and rebuilt on every commit\n\
         anime: {title}\n\
         episode: {episode}{}\n\
         catalog: {}/{}\n",
        total.map(|t| format!(" of {t}")).unwrap_or_default(),
        key.0,
        key.1
    )
}

/// Make a title safe as a single folder-name component, cross-platform:
/// path separators and Windows-reserved punctuation become `-` (a bare `/`
/// would literally create an unintended subdirectory), control characters
/// too, and trailing dots/spaces are trimmed (Windows rejects them).
fn sanitize_component(name: &str) -> String {
    let replaced: String = name
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '-',
            c if c.is_control() => '-',
            c => c,
        })
        .collect();
    let trimmed = replaced.trim().trim_end_matches(['.', ' ']).trim();
    if trimmed.is_empty() {
        "untitled".to_string()
    } else {
        trimmed.to_string()
    }
}

fn set_readonly(path: &Path) -> std::io::Result<()> {
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_readonly(true);
    fs::set_permissions(path, perms)
}

/// Recursively clear the read-only flags we set ourselves, so the next full
/// wipe can delete the tree (Windows refuses to delete read-only files;
/// Unix doesn't care, so this is cheap insurance either way).
fn make_writable(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    let mut perms = metadata.permissions();
    if perms.readonly() {
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        fs::set_permissions(path, perms)?;
    }
    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            make_writable(&entry?.path())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo::commit::{CatalogRef, Changes, Commit};

    #[test]
    fn sanitizes_risky_title_characters() {
        assert_eq!(
            sanitize_component("Kaguya-sama: Love is War"),
            "Kaguya-sama- Love is War"
        );
        assert_eq!(sanitize_component("Fate/stay night"), "Fate-stay night");
        assert_eq!(sanitize_component("Which Witch?"), "Which Witch-");
        assert_eq!(sanitize_component("a<b>c\"d|e\\f*g"), "a-b-c-d-e-f-g");
        // Windows-hostile trailing dots/spaces, and nothing-left cases.
        assert_eq!(sanitize_component("Trailing... "), "Trailing");
        assert_eq!(sanitize_component("..."), "untitled");
        assert_eq!(sanitize_component("  "), "untitled");
    }

    fn commit_for(
        parents: Vec<String>,
        id: i64,
        status: WatchStatus,
        progress: Option<u32>,
        msg: &str,
    ) -> Commit {
        Commit::new(
            parents,
            "main",
            CatalogRef {
                source: "anilist".to_string(),
                id,
            },
            Changes {
                status: Some(status),
                episode_progress: progress,
                ..Default::default()
            },
            msg,
        )
    }

    /// Integration test against a real temp repo + filesystem: build,
    /// assert structure, commit again, assert the FULL rebuild left no
    /// stale state behind.
    #[test]
    fn regenerate_builds_and_fully_rebuilds() {
        let dir = std::env::temp_dir().join(format!(
            "anigit-tree-test-{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&dir).unwrap();
        let repo = Repo::init(&dir).unwrap();

        // Watching, 3 episodes in; plus a planning entry with no progress.
        let c1 = commit_for(Vec::new(), 30, WatchStatus::Watching, Some(3), "start");
        repo.write_commit(&c1).unwrap();
        let c2 = commit_for(
            vec![c1.id.clone()],
            21519,
            WatchStatus::Planning,
            None,
            "plan",
        );
        repo.write_commit(&c2).unwrap();
        regenerate(&repo).unwrap();

        let watching = dir.join("watching");
        assert!(watching.join(README_NAME).is_file());
        let anime_dir = only_subdir(&watching);
        let season = anime_dir.join("Season 1");
        for episode in 1..=3 {
            let file = season.join(episode.to_string());
            assert!(file.is_file(), "missing episode file {episode}");
            assert!(fs::metadata(&file).unwrap().permissions().readonly());
            let content = fs::read_to_string(&file).unwrap();
            assert!(content.contains("auto-generated"));
            assert!(content.contains("anilist/30"));
        }
        assert!(!season.join("4").exists());
        // Planning entry: folder exists (browsable) but zero episode files.
        let planning_season = only_subdir(&dir.join("planning")).join("Season 1");
        assert!(planning_season.is_dir());
        assert_eq!(fs::read_dir(&planning_season).unwrap().count(), 0);
        assert!(!dir.join("completed").exists()); // no completed entries

        // Move the anime to completed at 5 episodes: full rebuild must drop
        // the watching/ dir entirely, not leave stale files behind.
        let c3 = commit_for(
            vec![c2.id.clone()],
            30,
            WatchStatus::Completed,
            Some(5),
            "done",
        );
        repo.write_commit(&c3).unwrap();
        regenerate(&repo).unwrap();

        assert!(!dir.join("watching").exists(), "stale watching/ left behind");
        let completed_season = only_subdir(&dir.join("completed")).join("Season 1");
        assert_eq!(fs::read_dir(&completed_season).unwrap().count(), 5);

        // Cleanup (clear our own read-only flags first).
        make_writable(&dir).unwrap();
        fs::remove_dir_all(&dir).unwrap();
    }

    fn only_subdir(dir: &Path) -> PathBuf {
        let dirs: Vec<PathBuf> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.is_dir())
            .collect();
        assert_eq!(dirs.len(), 1, "expected exactly one anime dir in {dir:?}");
        dirs.into_iter().next().unwrap()
    }
}
