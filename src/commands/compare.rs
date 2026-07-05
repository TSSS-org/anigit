use anyhow::{bail, Result};

/// `anigit compare <other_repo>` — anigit's own invented command (no git
/// equivalent). Implements the Option D manual merge-conflict-resolution-
/// by-comparison UX from brainstorm.md 1.7: surfaces a diff/comparison view
/// between two repos/lists ("you said 8, they said 6; you've watched 40
/// they haven't...") rather than silently auto-picking a value. Doubles as
/// a standalone "compare two lists" feature independent of any merge.
///
/// Shared logic with `anigit merge`'s conflict-resolution step — that
/// command should call into this one rather than duplicating the
/// comparison UI.
pub fn run(other_repo: &str) -> Result<()> {
    // TODO:
    // 1. Open `other_repo` as a second Repo.
    // 2. Diff both repos' current state (derived from replaying their
    //    respective histories) per shared catalog_ref.
    // 3. Auto-resolve objective fields (max), print/highlight subjective
    //    field differences for the user to review.
    let _ = other_repo;
    bail!("anigit compare: not yet implemented")
}
