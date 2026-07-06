use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::Path;

use crate::catalog::{catalog_path_for_sync, Catalog};

/// `anigit refresh` — manually sync the local anime metadata catalog cache
/// against the animetaScraper VM. This is the ONE v1 command that requires
/// network access (a deliberate, documented exception to "v1 is fully
/// offline" — it syncs shared catalog metadata, not personal repo data).
///
/// Implements the delta-pull design from brainstorm.md 1.13/1.13a:
///   1. Fetch manifest.json from the source (always fetched first, tiny).
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
///
/// The source is `ANIGIT_CATALOG_SOURCE` — an `http(s)://` URL or a local
/// directory path laid out per 1.13a (`manifest.json`, `snapshots/`,
/// `deltas/`). There's no real VM yet, so there is deliberately no default
/// to fall back to; once one exists, its URL becomes the default and the
/// env var stays as an override, mirroring `ANIGIT_CATALOG`.
pub fn run() -> Result<()> {
    let Some(source) = env::var_os("ANIGIT_CATALOG_SOURCE") else {
        bail!(
            "no catalog source configured — set ANIGIT_CATALOG_SOURCE to the \
             animetaScraper served directory (an http(s):// URL or a local \
             path containing manifest.json). There is no default VM yet \
             (brainstorm.md 1.13a)."
        );
    };
    let source = source.to_string_lossy().into_owned();

    let manifest: Manifest = serde_json::from_slice(&fetch(&source, "manifest.json")?)
        .context("failed to parse manifest.json")?;
    if manifest.schema_version > 1 {
        bail!(
            "manifest.json has schema_version {} — produced by a newer \
             animetaScraper; please upgrade anigit",
            manifest.schema_version
        );
    }

    // Open (or bootstrap) the local catalog. init_schema is CREATE IF NOT
    // EXISTS, so this is a no-op on an existing catalog and lets refresh
    // populate a brand-new one from scratch.
    let catalog = Catalog::open(&catalog_path_for_sync()?)?;
    catalog.init_schema()?;

    let last_synced = catalog.last_synced_run()?.unwrap_or(0);
    if last_synced >= manifest.latest_run {
        println!(
            "Already up to date (synced to run {last_synced}, latest is run {}).",
            manifest.latest_run
        );
        return Ok(());
    }

    // Checkpoint decision (1.13a): the newest checkpoint the client hasn't
    // synced past, if any, beats replaying the delta chain from last_synced.
    let checkpoint = manifest
        .checkpoints
        .iter()
        .filter(|c| c.run > last_synced)
        .max_by_key(|c| c.run);

    let mut from_run = last_synced;
    if let Some(checkpoint) = checkpoint {
        let bytes = fetch(&source, &checkpoint.file)?;
        // The checkpoint may come over HTTP, and SQLite needs a file to
        // attach — stage it in a temp file either way.
        let staged = env::temp_dir().join(format!("anigit-checkpoint-{}.sqlite", checkpoint.run));
        fs::write(&staged, bytes)?;
        let rows = catalog.replace_from_checkpoint(&staged)?;
        fs::remove_file(&staged).ok();
        println!(
            "Loaded checkpoint for run {} (generated {}): catalog replaced, {rows} entries.",
            checkpoint.run, checkpoint.generated_at
        );
        from_run = checkpoint.run;
    }

    // Deltas strictly after the starting point, oldest first. A gap in the
    // chain means the manifest is broken — refuse rather than skip runs.
    let mut deltas: Vec<&DeltaRef> = manifest
        .deltas
        .iter()
        .filter(|d| d.run > from_run && d.run <= manifest.latest_run)
        .collect();
    deltas.sort_by_key(|d| d.run);
    for (expected, delta) in (from_run + 1..=manifest.latest_run).zip(&deltas) {
        if delta.run != expected {
            bail!(
                "manifest is missing the delta for run {expected} (found run \
                 {} instead) — cannot sync a broken chain",
                delta.run
            );
        }
    }
    if deltas.len() as u64 != manifest.latest_run - from_run {
        bail!(
            "manifest lists {} delta(s) after run {from_run} but latest_run \
             is {} — cannot sync a broken chain",
            deltas.len(),
            manifest.latest_run
        );
    }

    let (mut inserted, mut updated, mut missing) = (0u64, 0u64, 0u64);
    for delta_ref in &deltas {
        let delta: DeltaFile = serde_json::from_slice(&fetch(&source, &delta_ref.file)?)
            .with_context(|| format!("failed to parse {}", delta_ref.file))?;
        if delta.schema_version > 1 {
            bail!(
                "{} has schema_version {} — produced by a newer \
                 animetaScraper; please upgrade anigit",
                delta_ref.file,
                delta.schema_version
            );
        }
        for change in &delta.changes {
            match change.op.as_str() {
                "insert" => {
                    catalog.insert_from_delta(change.id, &change.fields, &delta.generated_at)?;
                    inserted += 1;
                }
                "update" => {
                    if catalog.update_from_delta(change.id, &change.fields, &delta.generated_at)? {
                        updated += 1;
                    } else {
                        println!(
                            "warning: delta run {} updates id {} which is not \
                             in the local catalog — skipped",
                            delta.run, change.id
                        );
                        missing += 1;
                    }
                }
                // "delete" is reserved for future use in 1.13a; anything
                // unrecognized means a newer producer.
                other => bail!(
                    "unsupported op '{other}' in delta run {} — produced by a \
                     newer animetaScraper; please upgrade anigit",
                    delta.run
                ),
            }
        }
    }

    catalog.set_last_synced_run(manifest.latest_run)?;

    println!(
        "Synced to run {} ({}): {inserted} inserted, {updated} updated across {} delta(s){}{}.",
        manifest.latest_run,
        if checkpoint.is_some() {
            "via checkpoint"
        } else {
            "deltas only"
        },
        deltas.len(),
        if checkpoint.is_some() { ", after a full checkpoint load" } else { "" },
        if missing > 0 {
            format!(", {missing} skipped (missing rows)")
        } else {
            String::new()
        }
    );
    Ok(())
}

/// Fetch one file from the served directory — `http(s)://` via reqwest, or
/// plain filesystem for a local path / `file://` URL. Same code path either
/// way, so pointing at a real VM later is only a source-string change.
fn fetch(source: &str, relative: &str) -> Result<Vec<u8>> {
    if source.starts_with("http://") || source.starts_with("https://") {
        let url = format!("{}/{relative}", source.trim_end_matches('/'));
        let response = reqwest::blocking::get(&url)
            .and_then(|r| r.error_for_status())
            .with_context(|| format!("failed to fetch {url}"))?;
        Ok(response.bytes()?.to_vec())
    } else {
        let base = source.strip_prefix("file://").unwrap_or(source);
        fs::read(Path::new(base).join(relative))
            .with_context(|| format!("failed to read {base}/{relative}"))
    }
}

/// `manifest.json`, exactly as specified in brainstorm.md 1.13a and produced
/// by animetaScraper.
#[derive(Deserialize)]
struct Manifest {
    schema_version: u32,
    latest_run: u64,
    /// Cadence hint only (a checkpoint every Nth run) — the sync logic works
    /// purely off the checkpoint list itself.
    #[allow(dead_code)]
    checkpoint_interval: u64,
    checkpoints: Vec<CheckpointRef>,
    deltas: Vec<DeltaRef>,
}

#[derive(Deserialize)]
struct CheckpointRef {
    run: u64,
    file: String,
    generated_at: String,
}

#[derive(Deserialize)]
struct DeltaRef {
    run: u64,
    file: String,
    #[allow(dead_code)]
    generated_at: String,
    #[allow(dead_code)]
    changed_count: u64,
}

/// A single delta file (e.g. `deltas/delta-011.json`), per 1.13a.
#[derive(Deserialize)]
struct DeltaFile {
    schema_version: u32,
    run: u64,
    generated_at: String,
    changes: Vec<DeltaChange>,
}

#[derive(Deserialize)]
struct DeltaChange {
    op: String,
    id: i64,
    fields: serde_json::Map<String, serde_json::Value>,
}
