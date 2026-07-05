use anyhow::{bail, Result};

/// `anigit merge <branch>` — merge `branch` into the current branch.
///
/// Gated by `repo_kind` (brainstorm.md 1.6): refuse outright on
/// `RepoKind::SingleUser` repos. On `RepoKind::Shared` repos, follows the
/// accepted conflict model from brainstorm.md 1.7:
///   - Objective/numeric fields (e.g. episode_progress) auto-resolve via
///     max() — watching more is objectively "further along."
///   - Subjective fields (score, status) are real conflicts — do NOT
///     silently auto-pick. Surface via the same comparison UX as
///     `anigit compare` (Option D) and require a manual decision.
pub fn run(branch: &str) -> Result<()> {
    // TODO:
    // 1. Load repo config, check repo_kind == Shared, else bail with a clear
    //    error pointing at `anigit compare` as the SingleUser-safe
    //    alternative.
    // 2. Find common ancestor of current branch and `branch`.
    // 3. Diff both branches' changes since the ancestor.
    // 4. Auto-resolve objective fields via max().
    // 5. For subjective field conflicts, invoke the same comparison UI as
    //    commands::compare rather than duplicating that logic.
    // 6. Write a new commit with action = Merge and parent_ids = [both tips].
    let _ = branch;
    bail!("anigit merge: not yet implemented")
}
