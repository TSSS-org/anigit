use anyhow::{bail, Result};

/// Shared implementation for `anigit checkout` and `anigit switch` — both
/// commands are kept as real, separate commands (not one aliasing the
/// other) per the no-aliases naming policy in brainstorm.md 1.7a, but their
/// behavior is identical: switch HEAD to `branch`, optionally creating it
/// first if `create` (`-b` flag) is set.
pub fn run(branch: &str, create: bool) -> Result<()> {
    // TODO:
    // - If create: same as `anigit branch <branch>` then switch to it.
    // - Update .anigit/HEAD to point at `branch`.
    let _ = (branch, create);
    bail!("anigit checkout/switch: not yet implemented")
}
