use anyhow::{bail, Result};

/// `anigit blame <anime name>` — show which commit last changed each field
/// for a given anime entry. Low-priority per brainstorm.md section 2, but
/// straightforward once `history()` exists — walk commits touching this
/// catalog_ref and report the most recent commit per field.
pub fn run(anime_name: &str) -> Result<()> {
    let _ = anime_name;
    bail!("anigit blame: not yet implemented")
}
