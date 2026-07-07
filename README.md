# anigit

Git, but for your anime-watching history.

`anigit` is a local-first CLI tool that reimplements git's full command
surface — `init`, `add`, `commit`, `log`, `diff`, `branch`, `merge`, and more
— renamed and repurposed for tracking anime instead of code. Written in
Rust. Full design rationale lives in [`brainstorm.md`](./brainstorm.md).

## Status

v1 complete and released. Every command in v1's scope (everything git can
do fully offline, plus `anigit refresh` as a documented network exception)
has a real implementation. See `brainstorm.md` section 4 for the full
v1/v2/v3 boundaries, and section 1 for the complete build history.

## Installation

### Homebrew (macOS/Linux)

```
brew tap TSSS-org/anigit
brew trust --formula tsss-org/anigit/anigit
brew install anigit
```

The `brew trust` step is required by Homebrew's tap trust check for
non-official taps (Homebrew 6.0.0+) — without it, `brew install` will
refuse with an "untrusted tap" error. This is expected, current Homebrew
behavior, not a problem with this formula.

To uninstall a Homebrew install, use Homebrew's own command:
```
brew uninstall anigit
```
**Do not use `anigit uninstall` for a Homebrew install** — it will detect
the Homebrew-managed install path and refuse, pointing you back at
`brew uninstall anigit`. This is intentional: deleting the binary directly
would leave Homebrew's own installation records out of sync with reality.

### Scoop (Windows) — ⚠️ untested

```
scoop bucket add anigit https://github.com/TSSS-org/scoop-anigit
scoop install anigit
```

The manifest is written to Scoop's documented schema and the download
URLs/checksums are real and verified against the actual release assets,
but this has not yet been run on a real Windows machine — no Windows
environment was available to test end-to-end at the time this was written.
If you try it and it works (or doesn't), that's worth confirming/fixing
before relying on it.

To uninstall a Scoop install, use Scoop's own command (`scoop uninstall
anigit`) rather than `anigit uninstall`, for the same bookkeeping-mismatch
reason as Homebrew above — `anigit uninstall`'s package-manager detection
currently only recognizes Homebrew's install path pattern; Scoop detection
is a planned follow-up, not yet implemented.

### Manual (any platform)

Download the prebuilt binary for your OS/architecture from the
[latest release](https://github.com/TSSS-org/anigit/releases/latest),
unzip it, and place the `anigit` binary somewhere on your PATH.

To uninstall a manual install:
```
anigit uninstall            # prompts for confirmation
anigit uninstall --confirm  # skips the prompt (for scripting/testing)
```
This removes the `anigit` binary and its bundled/synced anime catalog file
only — it never touches any `.anigit` repos or their generated folder-tree
views, regardless of where they are.

### From source

```
cargo install --path .
```

(Requires a Rust toolchain — install via [rustup](https://rustup.rs) if you
don't have one yet.)

## Project layout

```
src/
  main.rs           → CLI entry point, dispatches to commands/
  lib.rs            → module wiring
  cli.rs            → clap subcommand definitions (the full v1 command list)
  repo/
    mod.rs            → .anigit/ folder handling (init, HEAD, refs, commits)
    commit.rs          → Commit/Changes/CatalogRef data model (schema_version'd)
    config.rs          → RepoKind (Shared/SingleUser) + Visibility toggle
  commands/          → one file per subcommand
  catalog/           → SQLite access layer for the local anime metadata cache
  tui/               → interactive screens: anigit add's edit form, and the
                        search screens used by both `add` and `blame`
  tree.rs            → generates the read-only watching/completed/dropped/
                        planning/ folder view after every commit
scripts/
  update-packages.sh  → regenerates BOTH the Homebrew formula and the
                        Scoop manifest for a new release in one run
                        (downloads + verifies + hashes all six binaries,
                        writes homebrew-anigit/Formula/anigit.rb and
                        scoop-anigit/bucket/anigit.json)
```

## Releasing a new version

1. Bump the version in `Cargo.toml`.
2. Commit, push, then tag and push the tag: `git tag vX.Y.Z && git push origin vX.Y.Z`.
3. Wait for the GitHub Actions release workflow to finish building all 6
   platform targets (check the Actions tab).
4. Run `./scripts/update-packages.sh X.Y.Z` to regenerate BOTH the
   Homebrew formula and the Scoop manifest with fresh, verified checksums.
5. Commit and push the updated formula in `homebrew-anigit/` and the
   updated manifest in `scoop-anigit/`.
6. Test Homebrew: `brew uninstall anigit || true`, `brew untap TSSS-org/anigit || true`,
   then `brew tap TSSS-org/anigit`, `brew trust --formula tsss-org/anigit/anigit`,
   `brew install anigit`, `anigit --version`.
7. Test Scoop (on a real Windows machine — see the untested caveat above):
   `scoop bucket add anigit https://github.com/TSSS-org/scoop-anigit`,
   `scoop install anigit`, `anigit --version`.

## Related projects

- **`animetaScraper`** (separate repo, Python) — scrapes AniList's public
  GraphQL API to build/maintain the anime metadata catalog this tool reads
  from. Runs centrally on a scheduled VM job; `anigit refresh` pulls deltas
  from it.
- **`homebrew-anigit`** (separate repo) — the Homebrew tap; see
  Installation above.
- **`scoop-anigit`** (separate repo) — the Scoop bucket; see Installation
  above (untested on real Windows as of this writing).
- **AniHub** (not started) — a GitHub-style companion website for `anigit`
  repos. Planned for after v1 of this CLI is complete.