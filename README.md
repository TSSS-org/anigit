# anigit

Git, but for your anime-watching history.

`anigit` is a local-first CLI tool that reimplements git's full command
surface — `init`, `add`, `commit`, `log`, `diff`, `branch`, `merge`, and more
— renamed and repurposed for tracking anime instead of code. Written in
Rust. Full design rationale lives in [`brainstorm.md`](./brainstorm.md).

## Status

Early scaffold. v1 scope = everything git can do fully offline. See
`brainstorm.md` section 4 for the full v1/v2/v3 boundaries.

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
  tui/               → the anigit-add interactive menu (the only TUI command)
```

## Related projects

- **`animetaScraper`** (separate repo, Python) — scrapes AniList's public
  GraphQL API to build/maintain the anime metadata catalog this tool reads
  from. Runs centrally on a scheduled VM job; `anigit refresh` pulls deltas
  from it.
- **AniHub** (not started) — a GitHub-style companion website for `anigit`
  repos. Planned for after v1 of this CLI is complete.

## Building

```
cargo build
cargo run -- init
```

(Requires a Rust toolchain — install via [rustup](https://rustup.rs) if you
don't have one yet.)