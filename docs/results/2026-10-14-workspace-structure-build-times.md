# Build times before and after the workspace restructure (ADR-0038)

Measured 14 Oct 2026. **Before** = commit `5768128` (one `game` crate, flat `data` and `sim`). **After** = the
restructure (`crates/{core,client,tools}`, the client in `kit`, `world` and `game`): **pending**. The VM shrank to
3.8 GB of RAM (plus 8 GB of swap) halfway through the refactor; numbers taken in swap would not compare with the
baseline, so the after column is measured once the VM has its memory back, the same way (see "Method" and "To
measure the after column").

## The box

- **4 cores** (the ADR's earlier figures, such as "about 30 s for the web-dev build", came from the 16-core VM), 13 GB
  RAM + 8 GB swap during the baseline. Niklas used the other cores at the same time: the load average (1, 5,
  15 min) is next to every number.
- Every build with `-j 3` (`CARGO_BUILD_JOBS=3`), one build at a time, sccache on (`~/.cargo/config.toml`).

## Method

How Niklas builds: the `dev` profile (native) and the `web-dev` profile (`scripts/web-build.sh`) are both
incremental (cargo's default for `dev`, `incremental = true` for `web-dev`), so `CARGO_INCREMENTAL` was left unset, in
a target dir of its own (`target/timing`; sccache does not cache incremental crates, so the workspace's own crates
really compile). Dependencies were warm (a first build filled the target dir from sccache). Each measurement ran twice.

- **(a) workspace crates from scratch**: `cargo clean -p` for every workspace package (before: `data sim game art
  arena content-tool`; after: also `kit world`), then `cargo build --workspace`.
- **(b) native incremental**, `cargo build --workspace` after a change in `drydock/panels.rs` (a screen), `sim`'s ship
  code (before `src/ship.rs`, after `src/ship/mod.rs`) and the UI kit (before `game/src/ui_kit.rs`, after
  `kit/src/ui_kit/mod.rs`):
  - **touch**: the file's mtime only (what the ADR names). With incremental compilation nothing is recompiled, so
    this is mostly cargo's checks and the link.
  - **edit**: a real code change. Run 1 adds a line `const _TIMING_EDIT: u32 = 1;` under the file's doc comment
    (every item below it moves, as most edits do); run 2 changes its value (only that item changes). Removed
    afterwards.
- **(c) web incremental**, `scripts/web-build.sh` (profile `web-dev`, no wasm-opt) after the same touches and edits.

## Results

| Measurement | Before, run 1 | Before, run 2 | Load (1 min) | After, run 1 | After, run 2 |
|---|---|---|---|---|---|
| (a) workspace crates from scratch, deps warm | 71.7 s | 63.9 s | 3.3 / 3.6 | pending | pending |
| (b) native, touch `drydock/panels.rs` | 3.8 s | 3.2 s | 3.6 / 3.4 | pending | pending |
| (b) native, touch `sim` ship | 6.2 s | 3.6 s | 3.4 / 3.2 | pending | pending |
| (b) native, touch `ui_kit` | 3.0 s | 3.1 s | 3.4 / 3.4 | pending | pending |
| (b) native, edit `drydock/panels.rs` (new line / value) | 44.2 s | 5.7 s | 3.2 / 3.1 | pending | pending |
| (b) native, edit `sim` ship | 55.9 s | 9.2 s | 3.5 / 3.2 | pending | pending |
| (b) native, edit `ui_kit` | 59.8 s | 6.3 s | 3.3 / 3.2 | pending | pending |
| (c) web, touch `drydock/panels.rs` | 29.1 s | 26.3 s | 3.0 / 2.5 | pending | pending |
| (c) web, touch `sim` ship | 27.0 s | 27.6 s | 2.1 / 2.2 | pending | pending |
| (c) web, touch `ui_kit` | 27.9 s | 28.2 s | 2.0 / 1.9 | pending | pending |
| (c) web, edit `drydock/panels.rs` (new line / value) | 63.4 s | 24.4 s | 3.0 / 3.0 | pending | pending |
| (c) web, edit `sim` ship | 48.9 s | 18.0 s | 2.9 / 2.5 | pending | pending |
| (c) web, edit `ui_kit` | 49.5 s | 17.7 s | 2.7 / 2.6 | pending | pending |

The first web build into the empty target dir (the wasm dependencies from sccache and from scratch) took 406 s.

What the baseline already says: on 4 cores a touch costs about 3 s natively and 27 s on the web (the web build's floor:
wasm-bindgen over the 110 MB wasm, trunk). An edit that moves a file's items costs 44–60 s natively and 49–63 s on
the web, a small edit 6–9 s and 18–24 s. Whether the split helps is what the after column has to show: an edit in a
screen now recompiles only `game` (24k lines instead of 38k), an edit in `ui_kit` recompiles `kit`, `world` and
`game`, and the web build's link and wasm-bindgen stay.

## To measure the after column

On the restructured tree, with 13 GB of RAM, the same target dir and jobs: `cargo build --workspace` once (warm
up), then the same steps with `cargo clean -p data -p sim -p art -p kit -p world -p game -p arena -p content-tool`,
the files `crates/client/game/src/drydock/panels/mod.rs`, `crates/core/sim/src/ship/mod.rs` and
`crates/client/kit/src/ui_kit/mod.rs`, and `scripts/web-build.sh` with `CARGO_TARGET_DIR` set to that dir.
