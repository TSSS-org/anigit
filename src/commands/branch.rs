use anyhow::{bail, Result};

/// `anigit branch [name]` — list branches, or create a new one from the
/// current HEAD if `name` is given.
pub fn run(name: Option<&str>) -> Result<()> {
    // TODO:
    // - No name: list all branches under .anigit/refs/branches/, marking
    //   the current one.
    // - With name: create a new ref file pointing at the current branch's
    //   head commit.
    let _ = name;
    bail!("anigit branch: not yet implemented")
}
