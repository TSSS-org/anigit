use anyhow::{bail, Result};

/// `anigit revert <commit_id>` — safe undo: creates a NEW commit that
/// reverses the given commit's changes, rather than deleting/rewriting
/// history. This is the only "undo" mechanism in v1 — `reset` (which would
/// rewrite history) is deferred to v2 and may end up restricted even then,
/// since it conflicts with the append-only philosophy (brainstorm.md 1.3,
/// 1.7a).
pub fn run(commit_id: &str) -> Result<()> {
    // TODO:
    // 1. Read the target commit's `changes`.
    // 2. Compute the inverse (e.g. if it set episode_progress: 12, figure
    //    out what it was before by walking history further back).
    // 3. Write a new commit with those inverse changes and a message like
    //    "Revert <commit_id>".
    let _ = commit_id;
    bail!("anigit revert: not yet implemented")
}
