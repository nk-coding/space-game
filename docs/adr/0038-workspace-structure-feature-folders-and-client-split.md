# ADR-0038: Workspace structure: grouped crate folders, the client split into three crates, feature folders inside

- Status: Accepted
- Date: 2026-10-14
- Decided by: Niklas (approved the lead's proposal as written, 14 Oct 2026; to run right after the chain-reaction work)
- Related: [ADR-0003](0003-crate-architecture.md), [ADR-0034](0034-fast-everyday-web-builds-and-shared-compile-cache.md), [ADR-0010](0010-cross-platform-determinism.md), [ADR-0036](0036-arena-crate.md), `CLAUDE.md` "Crates"

## Context

Niklas, 14 Oct 2026: "all these rust crates seem to lack some structure, in particular if this grows more in the
future / or is this really best practice how it is currently done?" He chose "folders + split game", after the
running work.

The state on 14 Oct (77,500 lines of Rust):

- The engine-free split of ADR-0003 holds and is the part worth keeping: `data` (11.7k lines) and `sim` (17.1k) have
  no Bevy, and `arena` and `content-tool` build on them.
- `game` is 38.3k lines in 75 files, 30 top-level modules side by side in `src/`. They reach into each other
  freely: of 30 modules, 22 use five or more others, and there are cycles through almost every pair of layers
  (the toolkit uses the part icons, which use the ship renderer, which uses the scene, which uses the drydock).
  Most of these edges are one type that sits in the wrong module: `PlayerDesign` and `PlayerBehaviors` in
  `drydock` (used by the scene, voyages and the career), `PilotFocus` in `scene` (used by the camera, the ship
  renderer and the bridge), `CameraRig` read by `atlas`, the `Rng` of `backdrop` used by `fx`.
- `sim` and `data` are flat too (31 and 20 files), and 14 files are over 1,100 lines (`sim/ship.rs` 1,858,
  `data/edit.rs` 1,734, `data/validate.rs` 1,644, `drydock/panels.rs` 1,546, `drydock/blueprints.rs` 1,515,
  `ui_kit.rs` 1,375, `content-tool/wiki/pages.rs` 2,171).
- A change anywhere in `game` recompiles all 38k lines (about 30 s for the web-dev build, ADR-0034).

Is the current layout normal? The engine-free core is good practice. A flat `game` crate is common in young Bevy
projects; the usual advice once it grows is one plugin per feature in its own folder, and separate crates when
compile times or boundaries start to hurt. At 38k lines with cycles everywhere, both now apply.

## Options considered

- **Feature folders only, `game` stays one crate**: cheapest; boundaries stay conventions and the whole client
  still recompiles on every change. Not chosen (Niklas chose the split).
- **Many small client crates (one per screen)**: strongest boundaries, but Bevy plugins share many resources, so
  most screens would need a common crate anyway, and each crate adds link and metadata overhead. Not chosen.
- **Three client crates by layer, feature folders inside every crate, crates grouped in folders** (chosen).

## Decision

### Workspace folders

```
crates/
  core/     data, sim                    engine-free, deterministic (ADR-0003, ADR-0010)
  client/   art, kit, world, game        Bevy client (art stays Bevy-free)
  tools/    arena, content-tool          native CLIs and headless tools
```

Package names stay (`cargo test -p sim` keeps working); only paths move. `members = ["crates/*/*"]`.

### The client in three crates, each layer only using the ones below

- **`kit`** (toolkit, about 4k lines): `atlas` (+ its shader), `meshbuf`, `style`, `ui_kit` (transition,
  scrollbar), the icon component and raster cache of `ui_icons`, `clipboard`, `params`. Knows nothing about
  ships or the sim. The camera's zoom reaches it through a small `ViewScale` resource that `world` writes.
- **`world`** (the running game, about 10k lines): `sim_bridge`, `scene`, `ship_view`, `ship_model`, `fx`,
  `shield`, `backdrop`, `camera`, `controls`, `hud`, `flight_hud`, `fleet_bar`, `test_range`, `tuning`, `mode`,
  `voyage`, `storage`, `career`, plus a `player` module for what the player owns (`PlayerDesign`,
  `PlayerBehaviors`, `DesignDoc`, moved out of `drydock`). Part icons (rendered from ship visuals) register
  here into `kit`'s icon cache.
- **`game`** (screens and the app, about 24k lines): `drydock`, `behavior_ui`, `research`, `fleet`, `map`,
  `shipyard`, `home`, `debrief`, and `main.rs`. Screens don't use each other's internals; what two screens share
  moves down into `world`.

### Inside every crate: feature folders, one plugin per feature

- `game`/`world`/`kit`: each feature is a folder with `mod.rs` holding its plugin and its public surface
  (as `drydock/` and `map/` already do); single-file features stay files.
- `sim`: `ship/` (ship, vessel, design, mass, collider, breakage, power, alloc, control), `flight/` (flight,
  maneuver, physics), `combat/` (weapons, plume, chain reactions, salvage), `behavior/` (exists), `voyage/`
  (voyage, voyage_runner, scenario); at the root what everything uses: world, events, ids, hash, rng, math,
  config, report.
- `data`: `content/` (model, load, validate, kind, problem, table, text, art_keys, svg, limits), `design/`
  (geometry, edit, blueprint, stats), `behavior/` (behavior, behavior_edit); career, fs and hash at the root.
- Files over about 800 lines are split by responsibility (not by line count alone), starting with the 14 above.
- Call sites move to the new paths; no compatibility re-exports (no dead code).

### How it is done

- In stages, one commit each, with `git mv` so history follows: (1) `data` and `sim` folders, (2) the `game`
  cycles broken inside the one crate (types moved down), (3) the three crates and the workspace folders,
  (4) big files split.
- Behaviour must not change: every test passes after every stage, every determinism hash is unchanged (native and
  wasm), the fight hashes of `docs/arena.md` are unchanged, and headless screenshots of each screen match before
  and after.
- Build times are measured before and after, the same way, and recorded in `docs/results/`: a clean native
  build, an incremental native build after touching `drydock/panels.rs`, `sim/ship/…` and `kit/ui_kit`, and the
  same three for `scripts/web-build.sh`.

## Consequences

- A change in a screen recompiles `game` only (about 24k lines instead of 38k); a change in `kit` recompiles the
  whole client. Whether the web build gets faster is measured, not assumed (the final wasm link may dominate).
- The layering is enforced by the compiler: a screen that needs another screen's type is the sign it belongs in
  `world`.
- Every path in `CLAUDE.md`, `docs/` and the scripts (`web-build.sh`, trunk's `index.html` location, the
  content-tool and arena paths) changes once; the docs are updated in the same stage.
- Supersedes nothing: ADR-0003's engine-free split stands. Its crate list gets a dated status line pointing here
  once this is accepted.
