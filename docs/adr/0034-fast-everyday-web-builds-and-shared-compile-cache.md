# ADR-0034: Fast everyday web builds and a shared compile cache

- Status: Accepted
- Date: 2026-10-14
- Decided by: Niklas (interactively, after asking why full builds take so long)
- Related: [ADR-0025](0025-main-repo-holds-only-chosen-code.md), `CLAUDE.md` (development environment), `scripts/web-build.sh`

## Context

Measured 14 Oct 2026 on the VM (16 cores, load ≈ 11 with three agents building): after a one-file change in
`game`, `cargo check` took 7 s and the native test build 8 s, but `scripts/web-build.sh` took 4 min 44 s: 2 min 44 s
of cargo (profile `release`: one codegen unit and thin LTO, so the whole `game` crate is optimised single-threaded),
the rest `wasm-bindgen` and `wasm-opt -O3` on a 49 MB wasm. Worktree agents build in their own target dirs with
incremental compilation off, so each one compiled Bevy and ~400 dependencies from scratch (native, tests, web).
The web build is for development only (not shipped), so its runtime speed matters only for comfortable work.

## Options considered

- **An everyday web profile** (no LTO, 16 codegen units, incremental, no wasm-opt) with the full release kept for
  performance checks (chosen).
- **A shared compile cache for every cargo build on the VM** (sccache; chosen with the profile).
- Splitting `game` into several crates (later: a refactor, would cut rebuilds further).
- Keep everything as it is.

## Decision

- `Cargo.toml` gets `[profile.web-dev]` (inherits `release`; `codegen-units = 16`, `lto = false`,
  `incremental = true`). `scripts/web-build.sh` builds with it by default, without wasm-opt; `--full` is the old
  build (profile `release` + wasm-opt -O3), for performance numbers.
- sccache 0.18 (`~/.cargo/bin/sccache`) wraps rustc for every cargo build on this VM through
  `~/.cargo/config.toml` (`build.rustc-wrapper`), cache in `~/.cache/sccache`, at most 5 GiB
  (`~/.config/sccache/config`). It caches non-incremental crates (all dependencies, and agents' builds with
  `CARGO_INCREMENTAL=0`); incremental workspace crates pass through. Nothing in the repo depends on it.

## Consequences

- Browser performance numbers come only from `scripts/web-build.sh --full`.
- The first `web-dev` build compiles every dependency once more (its own profile dir, ~2 GB).
- On another machine without sccache, delete the wrapper line from `~/.cargo/config.toml` (it is not in the repo).
- If a stale-cache problem ever shows, `sccache --zero-stats` / deleting `~/.cache/sccache` resets it.
