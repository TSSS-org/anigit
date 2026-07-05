use anyhow::{bail, Result};

/// `anigit tag <name>` — create a tag at the current commit (e.g.
/// "watched-100-shows", "2025-completed"). Milestone/flavor feature per
/// brainstorm.md section 2 (Under Consideration) — not core v1, but stubbed
/// here since it's cheap plumbing on top of the commit/ref model.
pub fn run(name: &str) -> Result<()> {
    // TODO: write a tag ref, similar shape to branch refs but under
    // .anigit/refs/tags/<name> instead of refs/branches/<name>.
    let _ = name;
    bail!("anigit tag: not yet implemented")
}
