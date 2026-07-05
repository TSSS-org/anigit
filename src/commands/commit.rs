use anyhow::{bail, Result};

/// `anigit commit -m "message"` — plain, flag-based, no TUI (brainstorm.md
/// 1.7a). Finalizes whatever was staged by a prior `anigit add` call.
pub fn run(message: &str, amend: bool) -> Result<()> {
    // TODO:
    // 1. Load the staged change (format/location TBD — needs a staging-area
    //    design decision that mirrors git's index, scoped to a single
    //    anime entry at a time per brainstorm.md 1.7a).
    // 2. Construct a `repo::commit::Commit` from the staged Changes +
    //    CatalogRef, with `message` and the correct `parent_ids` (current
    //    branch head).
    // 3. If `amend`, replace metadata on the most recent commit instead of
    //    creating a new one (still respecting append-only history — likely
    //    means writing a new commit object and updating the branch ref,
    //    not literally mutating the old file).
    // 4. repo.write_commit(&commit)?
    let _ = (message, amend);
    bail!("anigit commit: not yet implemented — see TODOs in commands/commit.rs")
}
