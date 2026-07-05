use anyhow::{bail, Result};

/// `anigit diff [from] [to]` — show changes between two commits, or between
/// a commit and current working/staged state if only one arg (or none) is
/// given.
pub fn run(from: Option<&str>, to: Option<&str>) -> Result<()> {
    // TODO: replay `changes` fields between the two commit points and print
    // a human-readable diff (per-field before/after).
    let _ = (from, to);
    bail!("anigit diff: not yet implemented")
}
