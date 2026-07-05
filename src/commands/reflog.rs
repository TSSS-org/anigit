use anyhow::{bail, Result};

/// `anigit reflog` — log of all ref (branch head) updates, as a recovery
/// safety net. Real git keeps this separate from the commit log since it
/// survives even history rewrites; for anigit's append-only model this is
/// simpler (refs only ever move forward), but still useful for "what did
/// HEAD point at before" debugging.
pub fn run() -> Result<()> {
    bail!("anigit reflog: not yet implemented")
}
