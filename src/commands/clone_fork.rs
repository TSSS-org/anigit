use anyhow::{bail, Result};

/// `anigit clone <url> [destination]` — copy an existing repo.
///
/// v1 note: only local-path "URLs" are meaningful until AniHub/remote sync
/// exists (v2+) — see brainstorm.md 1.8. A `<url.anigit>` scheme for
/// AniHub-hosted repos is anticipated but not implemented yet.
pub fn run_clone(url: &str, destination: Option<&str>) -> Result<()> {
    // TODO: copy the entire .anigit/ directory tree from `url` (a local path
    // for now) into `destination` (or a directory derived from the source
    // name if not given).
    let _ = (url, destination);
    bail!("anigit clone: not yet implemented")
}

/// `anigit fork <url.anigit>` — anigit's own invented command (no real git
/// equivalent; "fork" is a GitHub concept). Functions like `clone` but tags
/// the resulting repo with fork status/provenance. Planned to integrate with
/// AniHub once that exists so forked repos show their lineage on the
/// website too. See brainstorm.md 1.7a.
pub fn run_fork(url: &str, destination: Option<&str>) -> Result<()> {
    // TODO:
    // 1. Same copy mechanism as run_clone.
    // 2. Additionally write fork provenance into .anigit/config: source URL
    //    and timestamp of the fork, so this repo can later show "forked
    //    from X" once AniHub exists.
    let _ = (url, destination);
    bail!("anigit fork: not yet implemented")
}
