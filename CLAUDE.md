# Space Game

A 2D physical ship-builder in Rust + Bevy. You are an awakened ship-mind in an abandoned precursor shipyard at the rim of a procedural galaxy. You build ships block by block on a grid (no wiring: shared power pool per piece, heat and radiators, adjacency bonuses), decide how they think (behavior trees on top of built-in maneuvers and an always-on flight computer), fly them yourself (stick + target lock) and salvage wrecks for materials and research. Ships are fully physical (mass, centre of mass, thrust torque) and break apart.

Owner: Niklas (hobby developer, works with AI help). This file is the brief for every Claude session and subagent working in this repo. Read it fully before you start.

## Where things are

| Path | What it is |
|---|---|
| `docs/adr/`, `docs/gdr/` | **Decision records** (authoritative): ADRs for architecture/tech, GDRs for game design. Each has a `README.md` index (including open questions). |
| `docs/concept-notes.md` | The overview of the game concept and all decisions, with dates and links to the records. Update it together with the records. |
| `docs/spike-plan.md` | The original first-work plan (physics benchmark → core sim → web/feel → load test → cross-device hash). |
| `docs/style-guide.md` | Ship style 2a and the UI look, exact colours, rendering suggestions. |
| `docs/roadmap.md` | Current phase plan and status (kept up to date by the lead session). |
| `docs/content-model.md` | How game content is organised so both the game and the future wiki use it. |
| `docs/sim.md`, `docs/client.md`, `docs/behavior-format.md` | How the simulation, the Bevy client and the behavior rule format work. |
| `docs/results/` | Benchmark and spike results (the physics benchmark harness itself lives in the separate local repo `../space-game-physics-bench`). |
| `docs/concept/` | The archived concept bundle: `boards/` (concept board HTML + screenshots, UI mockups PNG + source), `reference/` (`drydock-model.js` = drydock rules in plain JS, `concept-board-script.js`, `data-draft/*.ron`, Python generators), `CLAUDE.original.md`. Paths inside the old docs that say `reference/…` or `boards/…` now live under `docs/concept/`; `assets/…` now lives under `art/`. |
| `art/` | Art sources: `icons/*.svg` (each part as a style-2a tile, `*.icon-only.svg` = glyph without tile), `ships/` (Halvex Runner SVG + `halvex-runner.layout.json`, automaton raider SVG), `tokens.json` (all colours/style constants), `overview.png`. |
| `content/base/` | The base game content pack (RON). Loaded exactly like a mod. |
| `crates/` | Cargo workspace members (every dir with a `Cargo.toml` is a member). |

## Crates (planned architecture)

- `data`: content schema (serde types), pack loader (from in-memory files, so it works on wasm/Android), validation, shared blueprint geometry. No Bevy. Used by sim, game and tools (wiki).
- `sim`: engine-free simulation: ships, parts, physics (Rapier2D with `enhanced-determinism`, ADR-0014), heat, power, damage, weapons, flight computer, behaviors + Rhai. Fixed 30 Hz, deterministic. No Bevy, no graphics.
- `game`: the Bevy client (web first, later desktop and Android). Rendering at 60 fps with interpolation, UI, input.
- Experiments and benchmarks don't live here (ADR-0025): the physics benchmark is the separate local repo `/home/devbox/workspace/space-game-physics-bench`.
- Later: `net` (host-authoritative co-op, join codes, a tiny relay; browser transports are no longer required, ADR-0006/ADR-0023), `arena` (headless fights, evolution, balance), a wiki generator built on `data`.

## Hard rules

- **No remote repositories.** Local git only. Never create, push to or link a GitHub/GitLab repo.
- **All Rust.** No C++ anywhere. C (Box2D v3.1) only as a reference number in the physics benchmark, never in the game. (Node is fine for dev tooling such as the headless screenshot helper, never for game code.)
- **No Godot, Unity, Unreal or JavaScript game engines.** Bevy is the client. The web build comes from the same Rust code (WebAssembly).
- **Scripting is Rhai** (not Lua).
- **Every game crate must build for wasm32** (the dev platform) as well as Android + desktop. No threads or filesystem required inside `sim`/`data`; data comes in as bytes/strings.
- **Cross-platform determinism in `sim`:** fixed 30 Hz step; seeded RNG streams per system; ordered containers only (`BTreeMap`/`Vec`, never iterate a `HashMap`/`HashSet` in simulation logic); software math for trig (`libm` or own impl, never `f32::sin` etc. from std which may differ per platform); no wall-clock time; physics engine in its deterministic mode; a state hash every tick in debug builds. No FMA / fast-math.
- **Performance is the top priority** when choosing between options.
- **Placeholder numbers stay placeholders**: concept-board numbers are for feel, not a spec.
- **Don't silently change settled decisions** (see `docs/adr/`, `docs/gdr/`). If something doesn't work, report it with numbers.
- **The main repo holds only chosen code** (ADR-0025). No dead code, no abandoned alternatives, no experiment harnesses; those go to separate local repos next to this one. Reports about them may stay in `docs/results/`.
- **No fixed raster graphics** (Niklas, 11 Oct 2026). All art is vector: SVG, meshes, procedural shapes, vector fonts. Raster textures (icon atlases, glow sprites) are generated in memory at runtime at the resolution the display needs (zoom × DPI), and regenerated when that changes. Never commit PNGs for the game to use (screenshots in `docs/` are fine).
- **Content lives in data, not code.** Names, descriptions, stats, ship layouts: all in `content/` RON files, never hard-coded in Rust. See `docs/content-model.md`.

## Current priorities (decided by Niklas, 11 Oct 2026)

- **Web first, for development.** The web build (wasm in the host browser) is how Niklas develops and looks at the game; it is **not an end-user platform** (clarified 11 Oct 2026). End users get Android and desktop. So: wasm size doesn't matter (don't optimise for it); browser performance matters only for comfortable development; native performance weighs most. Android comes later; phone measurements from the spike plan are deferred until then. Keep everything Android-compatible (no desktop-only APIs).
- **Minimal content** at the start: a few materials, the ~16 drydock parts, a couple of ships. Build the systems, not the content.
- **Wiki-ready content:** a wiki will come later. All content must be organised so the same files feed the game and the wiki (stable ids, text in data, stat labels/units in data, optional long-form wiki prose next to the data). See `docs/content-model.md`.

## Development environment (the VM)

- Linux VM without GPU, 15 cores, ~11 GB RAM + 7 GB swap. **No sudo** (apt is unavailable). Native Bevy windows are pointless here (software rendering); look at the game through the web build.
- Rust via rustup (`source ~/.cargo/env` if `cargo` isn't on PATH): stable 1.99, targets `x86_64-unknown-linux-gnu` and `wasm32-unknown-unknown`. `trunk` 0.21.14 and `cargo-binstall` are in `~/.cargo/bin`.
- crates.io and GitHub are reachable (crates.io API needs a User-Agent header: use a generic one like `space-game-dev-tools`; never put personal data such as names or email addresses into requests).
- Heavy builds (Bevy, physics engines): several agents may build at the same time, so pass `-j 6` (or `CARGO_BUILD_JOBS=6`) to keep RAM in check.
- **Disk is tight** (48 GB total, ~10 GB free on 12 Oct 2026; it ran full twice). Every extra target dir costs 1–5 GB. Prefer the shared `target/`; if you need your own `CARGO_TARGET_DIR` to avoid lock contention, use `target/<your-name>` and **delete it when you finish**. Check `df -h /home` before big builds.
- **Headless browser check:** `node /home/devbox/workspace/tools/headless/shot.mjs <url> <out.png> [waitMs] [w] [h]` opens a page in headless Chromium (WebGL2 via SwiftShader, slow but correct), prints console messages and page errors (wasm panics show up here), and saves a screenshot. Use it to verify any web work, then look at the PNG with the Read tool. SwiftShader timings say nothing about real performance.
- The VM's address on the LAN side is `10.77.6.2`. Serve web builds with `--address 0.0.0.0` so Niklas can open them from the host browser (e.g. `http://10.77.6.2:8080`).
- Timing measured in the VM is noisy, more so when other agents compile at the same time. Repeat runs, report median and p95, record the load average (`uptime`) next to the numbers.

## Git conventions

- Work on branch `dev` (Niklas merges into `main`). Local commits only.
- Several agents may work in this tree at once. **Commit only your own paths** (`git add <paths>`; never `git add -A` / `git commit -a`). Leave `Cargo.lock` to the lead session unless told otherwise. If `git` reports `index.lock`, wait a moment and retry.
- Small, focused commits with a clear message. End every commit message with:
  `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`
- Pin exact dependency versions (`bevy = "=0.20.0"` style) for engine-level crates; Bevy breaks APIs every 4–5 months.

## Decision records

Two numbered series (decided by Niklas, 11 Oct 2026):
- `docs/adr/NNNN-<slug>.md`: **Architecture Decision Records**, for technical decisions (crates, determinism, content format, rendering, physics, tooling, platforms).
- `docs/gdr/NNNN-<slug>.md`: **Game Design Records**, for gameplay and design decisions (ship rules, combat, controls, progression, world, look).

Each series has a `README.md` index and a `0000-template.md`. Records are short: status, date, who decided, context, options considered, decision, consequences, links. Records are never rewritten to change a decision: a new record **supersedes** the old one, and the old one's status says so. `docs/concept-notes.md` is the overview and links to the records.

When work produces a decision (or a provisional default), write or propose the record in the same change. Agents may add records with status `Proposed`; a decision becomes `Accepted` when Niklas decides it, or is `Accepted (provisional, lead)` when the lead session sets a default for him to review.

## Working with Niklas

- Explain trade-offs briefly and recommend one option. He wants design questions asked **interactively** (the question tool, multiple choice, recommended option first), not as lists in chat. Visual comparisons beat text when the question is about looks.
- When a decision is made or changed, write the ADR/GDR, update `docs/concept-notes.md` (and the roadmap), with the date.

## Glossary

| Term | Meaning |
|---|---|
| Ship-mind | The player: a precursor mind that controls ships through logic cores. |
| Bonded matter | Programmable matter that a part consumes once; destroyed ships re-form at home for free. |
| Piece | A connected group of parts; one rigid body. Each piece has its own power pool. |
| Vessel | A piece with at least one working logic core: it acts on its own (behavior) or is piloted. Pieces without a working core are salvage (GDR-0013). |
| Focus | The one vessel the player pilots directly; switching focus is instant. |
| Mind capacity | How many logic cores the ship-mind can run at once; grows with progression. |
| Cluster | One simulation space, about 10+ minutes across. A voyage usually covers one. |
| Band | Rim, outer, middle, inner, core. A guardian sits at every band gate. |
| Style 2a | The chosen ship look: flat tiles, one flat colour per machine with a glowing icon, soft teal hull glow (coral for enemies). |
