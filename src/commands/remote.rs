use anyhow::{bail, Result};

/// `anigit remote` — manage remote repo URLs. v2+ plumbing (needs
/// push/pull/AniHub to be meaningful), stubbed now since it's cheap to
/// scaffold alongside RepoConfig.remotes.
pub fn add(name: &str, url: &str) -> Result<()> {
    let _ = (name, url);
    bail!("anigit remote add: not yet implemented")
}

pub fn remove(name: &str) -> Result<()> {
    let _ = name;
    bail!("anigit remote remove: not yet implemented")
}

pub fn list() -> Result<()> {
    bail!("anigit remote list: not yet implemented")
}
