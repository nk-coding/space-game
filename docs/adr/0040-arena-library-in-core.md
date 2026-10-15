# ADR-0040: The `arena` library moves to `crates/core/arena`; its CLI becomes the package `arena-cli` in `crates/tools/`

- Status: Accepted (Niklas: the evolution lab's design, [GDR-0092](../gdr/0092-evolution-lab-for-your-own-ships.md),
  "move rather than duplicate"). Built 2026-10-15.
- Date: 2026-10-15
- Decided by: Niklas (GDR-0092's options, 15 Oct 2026)
- Amends: [ADR-0036](0036-arena-crate.md) (where the crate and its CLI live), [ADR-0038](0038-workspace-structure-feature-folders-and-client-split.md)
  (`tools/` held `arena`)
- Related: [ADR-0003](0003-crate-architecture.md), [ADR-0010](0010-cross-platform-determinism.md),
  [ADR-0031](0031-player-storage-and-design-sharing.md), [GDR-0083](../gdr/0083-evolution-lab.md),
  [GDR-0092](../gdr/0092-evolution-lab-for-your-own-ships.md), `docs/arena.md`

## Context

ADR-0038 put `arena` in `crates/tools/` with `content-tool`, because its first user was its native CLI. GDR-0092
makes the evolution lab a player-facing building, and step 1 of its plan replays arena fights in the flight view.
So the client (`world`) depends on the arena library. ADR-0038 groups the engine-free, deterministic libraries
under `core/`, and the client should not depend on a tool. The library was already engine-free, without I/O or
threads, and built for wasm32 (ADR-0036).

## Options considered

- **The library to `crates/core/arena`, the CLI to its own package `crates/tools/arena-cli`** (chosen): the
  folders say what each is, and the client's dependency points down into `core/`. The binary keeps its name
  `arena` and its commands and output.
- **The client depends on `crates/tools/arena`**: no move, but a client crate on a tool, against ADR-0038's
  grouping.
- **A copy of the fight code in the client**: two implementations that must give the same hashes. Ruled out by
  Niklas ("move rather than duplicate").

## Decision

- `crates/core/arena` (package `arena`): the library with its tests, the `fight_hashes` example, the fight specs of
  `fights/` and the run files of `runs/`. It depends on `data` and `sim` and the format crates the game's share codes
  use (`serde`, `ron`, `base64`, `miniz_oxide`). No Bevy, no I/O, no threads, no wall clock; builds for wasm32.
- `crates/tools/arena-cli` (package `arena-cli`, binary `arena`): only arguments, its thread pool, Markdown
  tables and file writing. What the game and the CLI share (fights, fight codes, the rules' fingerprint, a
  generation's best child) is in the library. `cargo run --release -p arena-cli -- fight|matrix|evolve …`.
- `world` depends on `arena` (the fight viewer, GDR-0092 step 1; later the lab's executor).
- Moved with `git mv`, so the files keep their history. `scripts/determinism-check.sh --fight` still builds the
  package `arena`.

## Consequences

- Paths in the docs change from `crates/tools/arena/…` to `crates/core/arena/…`, and the CLI's cargo command
  from `-p arena` to `-p arena-cli`. Older records keep the old paths as they were written.
- Every rule of ADR-0010 for `sim` holds for `arena` too (its `clippy.toml` bans std's transcendental float
  functions); `arena` is linted natively with all targets, like `sim` and `data`.
- The CLI's output was unchanged by the move: a fight, a matrix and an evolution run printed the same rows and
  wrote the same elites before and after (apart from the timing columns).
