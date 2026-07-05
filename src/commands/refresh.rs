use anyhow::{bail, Result};

/// `anigit refresh` — manually sync the local anime metadata catalog cache
/// against the animetaScraper VM. This is the ONE v1 command that requires
/// network access (a deliberate, documented exception to "v1 is fully
/// offline" — it syncs shared catalog metadata, not personal repo data).
///
/// Implements the delta-pull design from brainstorm.md 1.13/1.13a:
///   1. Fetch manifest.json from the VM (always fetched first, tiny).
///   2. Compare local `last_synced_run` against `latest_run`.
///   3. If a newer checkpoint exists than what's been synced past, download
///      that checkpoint .sqlite directly (replaces local cache wholesale),
///      then apply only the deltas listed after that checkpoint's run.
///   4. Otherwise, download and apply just the delta files between
///      `last_synced_run` and `latest_run`.
///   5. Update local `last_synced_run` to `latest_run`.
///
/// Checkpoint vs. delta-chain choice matters for speed: past ~2-3 missed
/// cycles, one checkpoint download beats chaining many small deltas.
pub fn run() -> Result<()> {
    // TODO:
    // 1. Load local sync state (last_synced_run) — needs a small local
    //    metadata file alongside the bundled SQLite catalog, not yet
    //    designed.
    // 2. reqwest::blocking::get the VM's manifest.json URL (URL/config TBD —
    //    likely belongs in a global anigit config, not per-repo, since the
    //    catalog is shared across all repos on a machine).
    // 3. Implement the checkpoint-vs-delta-chain decision from 1.13a.
    // 4. Apply deltas: for each { op, id, fields } entry, insert/update the
    //    corresponding row in the local SQLite catalog cache.
    bail!("anigit refresh: not yet implemented — see brainstorm.md 1.13/1.13a for full design")
}
