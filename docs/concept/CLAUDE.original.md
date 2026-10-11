# Space Game: context for Claude Code

This folder is the complete concept-phase context for Niklas's space game, exported on 11 Oct 2026. Read this file first, then the docs it points to. Nothing here is code yet: the next step is the **spike** (`docs/spike-plan.md`).

## The game in one paragraph

A 2D ship-builder for Android, Windows, Linux and the web. You are an awakened ship-mind in an abandoned precursor shipyard at the rim of a procedural galaxy. You build ships block by block on a grid (no wiring: shared power pool, heat and radiators, adjacency bonuses), decide how they think (behavior trees on top of built-in maneuvers and an always-on flight computer), fly them yourself (stick + target lock) and salvage wrecks for materials and research. Ships are fully physical: mass, centre of mass, thrust torque, and they break apart. Story stays light, the fleet is secondary, there is no economy and no generic scrap.

## Hard rules

- **No remote repositories.** Work in a local folder with local git only. Do not create, push to or link any GitHub/GitLab repo.
- **All Rust.** No C++ anywhere. C (Box2D v3.1) is allowed only as a reference measurement in the physics benchmark, never in the game.
- **No Godot, no Unity, no Unreal, no JavaScript game engines.** Bevy is the client. The web build comes from the same Rust code (WebAssembly).
- **Scripting is Rhai.** Niklas dislikes Lua's `end` syntax; don't propose Lua.
- **Every crate must build for wasm32, Android and desktop.** The web is a full platform (single player, co-op, mods, saves), not just a dev build.
- **Cross-platform determinism** in the `sim` crate:
  - fixed 30 Hz step and seeded RNG streams per system;
  - ordered containers only (no HashMap iteration in the simulation);
  - software math for trigonometry, no wall-clock time;
  - physics engine in its deterministic mode, and a state hash every tick in debug builds.
- **Performance is the top priority** when choosing between options (e.g. the physics engine is picked by benchmark).
- Placeholder numbers stay placeholders: the concept board's numbers are for feel, not a spec.

## Development setup

- You run in a **Linux VM without GPU passthrough**. A native Bevy window here would be software-rendered and slow.
  - Run the simulation, tests, benchmarks and headless tools natively in the VM.
  - Look at the game through the **web build**: serve it from the VM (for example `trunk serve --address 0.0.0.0`) and open it in the host browser. Port forwarding or bridged networking may be needed.
- **Android**: build the APK in the VM and install it on Niklas's flagship phone with **adb over Wi-Fi** (Android "Wireless debugging": `adb pair`, then `adb connect`). The VM's network must be able to reach the phone's IP.
- Timing measured inside the VM is noisy. Repeat runs, report the median and p95, and treat the phone and host-browser numbers as the ones that count.

## Where things are

| Path | What it is |
|---|---|
| `docs/concept-notes.md` | All decisions so far, with dates. The source of truth for design and tech decisions. |
| `docs/spike-plan.md` | What to build first, step by step, with pass/fail criteria. Start here for work. |
| `docs/style-guide.md` | Ship style 2a and the UI look, with exact colours and how to render them in Bevy. |
| `boards/concept-board/concept-board.html` | The interactive concept board (open in a browser). Has a working drydock with the real rules, damage sandbox, galaxy generator, recipe explorer, tech tree, evolution lab. |
| `boards/concept-board/*.png` | Screenshots of each board section, for a quick read. |
| `boards/ui-mockups/*.png` | The chosen UI mockups: phone drydock, flight HUD, home ring menu, behavior editor, galaxy map, desktop drydock, ship parts style sheet. `source/` holds their HTML/SVG. |
| `boards/ship-style/chosen-2a.png` | The chosen ship look, before/after and in flight. |
| `assets/icons/*.svg` | Every part as a style 2a tile (and `*.icon-only.svg` without the tile). Source for the texture atlas. `overview.png` shows them all. |
| `assets/ships/` | The sample ship "Halvex Runner" (12×7 cells) and an automaton raider as SVG, plus the runner's tile layout as JSON. |
| `assets/tokens.json` | Colours and style constants for ships and UI. |
| `reference/drydock-model.js` | The drydock rules from the concept board as plain JS: footprints, rotation, slope connectivity, power, heat, bonuses, thrust/torque, guns and engines needing open space, splitting into pieces, build points. Port the ideas, not the numbers. |
| `reference/concept-board-script.js` | The board's full script, including drawing, damage sandbox, galaxy generator and recipe data. |
| `reference/data-draft/` | Draft RON for parts and two sample ships, exported from the board. A starting point for the `data` crate, not a fixed schema. |
| `reference/generators/` | The Python scripts that generated the mockups and the style 2a SVGs. Reference only. |

## Planned architecture

- `sim`: engine-free. Ships, parts, physics, heat, power, damage, behavior trees, Rhai. Fixed 30 Hz, seeded, no graphics.
- `game`: the Bevy client. Rendering at 60 fps with interpolation, UI, touch, mouse and keyboard input, audio.
- `net`: host-authoritative sessions, join codes, a tiny relay. The transport must work in browsers: WebRTC, with a WebSocket fallback, and the relay doubles as the signalling server.
- `arena`: headless fights, evolution, balance reports, determinism checks.
- `data`: RON files for parts, trees, materials, races, set pieces and strings. The base game loads like a mod.

The spike only needs `sim`, a small `game` and a `bench` crate.

## Working with Niklas

- He is a hobby developer working with AI help. Explain trade-offs briefly and recommend one option.
- For decisions, he prefers short multiple-choice questions (a recommended option first) over long prose. Visual comparisons beat text when the question is about looks.
- When a decision is made or changed, update `docs/concept-notes.md` (and the spike plan if it's affected), with the date.
- Don't silently change settled decisions. If the spike shows one doesn't work, report it with numbers and ask.

## Glossary

| Term | Meaning |
|---|---|
| Ship-mind | The player: a precursor mind that controls ships through logic cores. |
| Bonded matter | Programmable matter that a part consumes once; destroyed ships re-form at home for free. |
| Piece | A connected group of parts. Each piece has its own power pool, and cut-off pieces become salvage. |
| Cluster | One simulation space, about 10+ minutes across. A voyage usually covers one. |
| Band | Rim, outer, middle, inner, core. A guardian sits at every band gate. |
| Style 2a | The chosen ship look: flat tiles, one flat colour per machine with a glowing icon, soft teal hull glow. |
