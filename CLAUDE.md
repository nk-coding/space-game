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
| `docs/plans/` | Proposals for future phases (first playable loop, galaxy, Android, co-op and arena, the evolution lab); not decisions. |
| `docs/content-model.md` | How game content is organised so both the game and the future wiki use it. |
| `docs/sim.md`, `docs/client.md`, `docs/behavior-format.md`, `docs/drydock.md`, `docs/career.md`, `docs/wiki.md` | How the simulation, the Bevy client, the behavior rule format, the drydock (ship editor), the career (progression and its save) and the wiki work. |
| `wiki/` | The wiki (ADR-0039): a VitePress site in the game's look. Hand-written pages (`guide/`, `mechanics/`, `behavior/`, listed in `nav.json`) reference the game's data by id; `content-tool wiki-data` generates a page per content entry into `wiki/gen/` (git-ignored). Vector only. |
| `docs/results/` | Benchmark and spike results (the physics benchmark harness itself lives in the separate local repo `../space-game-physics-bench`: local-only, not published). |
| `art/tokens.json` | All colours and style constants (compiled into the `art` crate and the wiki). Part icons live in the content pack: `content/base/icons/<key>.svg` (ADR-0028). |
| `../space-game-archive/` | A separate local repo (14 Oct 2026; local-only, not published, so public readers can't follow links into it) with what the game no longer uses but older records link to: the concept bundle (`concept/`: boards, UI mockups, the drydock rules in plain JS, data drafts, generators, the original CLAUDE.md; old paths `docs/concept/…`, `reference/…`, `boards/…`), the concept ship art (`ships/`, was `art/ships/`), the phase 7 screen mockups (`mockups/`, was `docs/plans/mockups/`), superseded plans and uncited screenshots. Nothing in this repo may depend on it. |
| `content/base/` | The base game content pack (RON). Loaded exactly like a mod. |
| `README.md`, `LICENSE`, `THIRD_PARTY_NOTICES.md` | The public front page (with the AI disclosure), the licence split (code MIT OR Apache-2.0, content and art CC BY 4.0, ADR-0047) and the dependencies' licences (generated: `scripts/third-party-notices.sh`, `docs/client.md` "Licences and third-party notices"). |
| `.github/workflows/` | GitHub Actions (ADR-0048): `pages.yml` publishes the optimised web build to GitHub Pages on pushes to `dev` (the published main branch), `ci.yml` runs the tests and the wiki check. They run only where the repo is published, never on the VM. |
| `crates/` | Cargo workspace members, grouped (ADR-0038): `core/` (`data`, `sim`, `arena`, `galaxy`: engine-free, deterministic), `client/` (`art`, `kit`, `world`, `game`: the Bevy client; `lab-worker`: the evolution lab's web worker, a second wasm binary of the web build, ADR-0043), `tools/` (`arena-cli`, `content-tool`; ADR-0040). Every `crates/<group>/<crate>` with a `Cargo.toml` is a member. |

## Crates (ADR-0003, folders and the client split ADR-0038)

- `data`: content schema (serde types), pack loader (from in-memory files, so it works on wasm/Android), validation, shared blueprint geometry, the blueprint text format, the pure editing models `data::design::edit` (drydock) and `data::behavior::edit` (behavior editor), the progression model `data::career` and the action map `data::input` (actions, inputs, bindings; defaults in `controls.ron`). No Bevy. Used by sim, game and tools (wiki).
- `sim`: engine-free simulation: ships, parts, physics (Rapier2D with `enhanced-determinism`, ADR-0014), heat, power, damage, weapons, flight computer, behaviors + Rhai. Fixed 30 Hz, deterministic. No Bevy, no graphics.
- The Bevy client (web first, later desktop and Android; rendering at 60 fps with interpolation, UI, input) is three
  crates, each using only the ones below it (ADR-0038): `kit` (the toolkit: runtime atlas and its shader, style
  tokens, mesh building, the bevy_ui widget kit and fonts, UI icons, clipboard, launch parameters; knows nothing about
  ships or the sim), `world` (the running game: the sim bridge, scenes and the fight viewer, ship views, effects, shields, camera,
  controls, the flight HUD and fleet bar, test range, voyages and their transitions, the scenery between a cluster's sites and its sky, storage, the career and what
  the player owns, `world::player`, the player's settings and the action map's input, `world::settings`) and `game` (the screens and `main`: drydock, BEHAVIOR screen, research, FLEET,
  voyage map, shipyard, home, debrief). What two screens share moves down into `world`.
- Inside every crate: feature folders, one plugin per feature (`mod.rs` holds the plugin and the public surface;
  single-file features stay files). `sim`: `ship/`, `flight/`, `combat/`, `behavior/`, `voyage/`; `data`:
  `content/`, `design/`, `behavior/`.
- `galaxy` (`crates/core/galaxy`, ADR-0042): the galaxy generator: a game's skeleton (clusters in bands, home, gates, trails of signals) and a cluster's interior from the seed and the content; integer positions, a generation hash (`scripts/determinism-check.sh --galaxy N`). Depends on `data` only, no Bevy, no I/O; the sim rolls interiors into voyage plans (`VoyagePlan::roll_interior`).
- `art`: style tokens (embedded), effect textures and the runtime SVG rasteriser. No Bevy.
- `content-tool`: CLI over `data` (validate, export-json, list, and `wiki-data`: the wiki's generated pages and data, checking its hand-written pages).
- Experiments and benchmarks don't live here (ADR-0025): the physics benchmark is the separate local repo `/home/devbox/workspace/space-game-physics-bench`.
- `arena` (`crates/core/arena`, ADR-0040): headless, deterministic fights on `sim` (fight specs with a RON text form, results, the arena score, a steppable `Fight`), the voyage fitness (`arena::voyage` over `sim::voyage::runner`), the evolution (`arena::evolve`: run configs in RON with up to four starts and their locks, mutation and crossover through the editing models, a MAP-Elites archive, a steppable `Run` with budgeted breeding; every genome and fight kept, run files `RunSave` with resume; trials with weights and goals, VALIDATE and RE-SCORE, GDR-0092; THE LADDER `arena::evolve::ladder`, voyage codes `SGV1:`, jobs as RON messages for the web workers `arena::worker`, step 9). No Bevy, no I/O or threads; builds for wasm32, used by the game, the `lab-worker` and the CLI (ADR-0036, `docs/arena.md`).
- `arena-cli` (`crates/tools/arena-cli`): the native `arena` binary (`fight`, `matrix`, `evolve`, `trials`, `validate`, `rescore`, `replay`, `lineage`, `check`, `report`) on every core, a thin layer of arguments, threads, tables and files over `arena`: `cargo run --release -p arena-cli -- fight crates/core/arena/fights/runner_vs_raiders.ron --seeds 50`; `-- evolve <run.ron> [--from <design>]... [--lock category:weapons]... [--trial default|<preset>[:weight[:seeds]]]... [--goal unhurt:1]... [--goals win_unhurt] --save run_file.ron [--until N]` (trials and goals: `lab.ron`'s presets, GDR-0092 step 5; `-- trials` lists them), `-- evolve --resume run_file.ron`, `-- validate run_file.ron [--best 8] [--seeds 20] [--save F]`, `-- rescore stale_run.ron --save new_run.ron`, `-- replay run_file.ron <gen> <child> <job>`, `-- lineage run_file.ron [<genome>]`. It prints fight codes (`SGF1:…`, GDR-0092): the game replays one at `?scene=fight&fight=<code>` (`docs/client.md` "Fight viewer"), and `scripts/determinism-check.sh --fight <code>` checks it native against wasm (`--fight runner_gauntlet` a gauntlet); `--run two_starts_locked` and `--run mixed_trials` (every trial kind and goal) check an evolution run, straight and resumed. **The balance check** (GDR-0122): `scripts/arena-check.sh` after every content or sim change (seven fixed fights against `crates/core/arena/check/baseline.ron`, about 6 s; exit 1 when a fight leaves its tolerance), `--update` after an intended change, committing the new baseline with it. **Ratings and part reports** (GDR-0124): `-- report [<design>...] [--run run_file.ron --best N] [--markdown F] [--ron F]` (no designs: the pool `crates/core/arena/report/pool.ron`, about 5 s) rates a round robin (Bradley–Terry, Elo scale) and prints each part's pick rate, win rate when used and flags (DOMINANT / UNDERUSED / NEVER PICKED).
- Later: `net` (host-authoritative co-op, join codes, a tiny relay; browser transports are no longer required, ADR-0006/ADR-0023).

## Hard rules

- **No remote repositories.** Local git only. Never create, push to or link a GitHub/GitLab repo.
- **All Rust.** No C++ anywhere. C (Box2D v3.1) only as a reference number in the physics benchmark, never in the game. (Node is fine for dev tooling such as the headless screenshot helper, never for game code.)
- **No Godot, Unity, Unreal or JavaScript game engines.** Bevy is the client. The web build comes from the same Rust code (WebAssembly).
- **Scripting is Rhai** (not Lua).
- **Every game crate must build for wasm32** (a player platform, ADR-0048) as well as Android + desktop. No threads or filesystem required inside `sim`/`data`; data comes in as bytes/strings.
- **Cross-platform determinism in `sim`:** fixed 30 Hz step; seeded RNG streams per system; ordered containers only (`BTreeMap`/`Vec`, never iterate a `HashMap`/`HashSet` in simulation logic); software math for trig (`libm` or own impl, never `f32::sin` etc. from std which may differ per platform); no wall-clock time; physics engine in its deterministic mode; a state hash every tick in debug builds. No FMA / fast-math.
- **Performance is the top priority** when choosing between options.
- **Placeholder numbers stay placeholders**: concept-board numbers are for feel, not a spec.
- **Don't silently change settled decisions** (see `docs/adr/`, `docs/gdr/`). If something doesn't work, report it with numbers.
- **The main repo holds only chosen code** (ADR-0025). No dead code, no abandoned alternatives, no experiment harnesses; those go to separate local repos next to this one. Reports about them may stay in `docs/results/`.
- **No fixed raster graphics** (Niklas, 11 Oct 2026). All art is vector: SVG, meshes, procedural shapes, vector fonts. Raster textures (icon atlases, glow sprites) are generated in memory at runtime at the resolution the display needs (zoom × DPI), and regenerated when that changes. Never commit PNGs for the game to use (screenshots in `docs/` are fine).
- **Content lives in data, not code.** Names, descriptions, stats, ship layouts: all in `content/` RON files, never hard-coded in Rust. See `docs/content-model.md`.

## Current priorities (decided by Niklas, 11 Oct 2026; the web's role changed 20 Oct 2026)

- **Web first, and the web is for players too** (Niklas, 20 Oct 2026, ADR-0048, superseding the 11 Oct "development platform only"). The web build (wasm in the browser) is how Niklas develops and looks at the game, and it is a first-class player platform beside the future desktop and Android builds, published on GitHub Pages (`.github/workflows/pages.yml`). So wasm size, loading time and browser performance matter for players now; native performance still matters too. `?dev=1` stays in the public build (its own save). Android comes later; phone measurements from the spike plan are deferred until then. Keep everything Android-compatible (no desktop-only APIs).
- **Minimal content** at the start: a few materials, the ~20 drydock parts, a couple of ships. Build the systems, not the content.
- **Wiki-ready content:** a wiki will come later. All content must be organised so the same files feed the game and the wiki (stable ids, text in data, stat labels/units in data, optional long-form wiki prose next to the data). See `docs/content-model.md`.

## Development environment (the VM)

(Moved to a more powerful VM on 12 Oct 2026; numbers and paths below re-checked there. Outside this repo the tooling also needs `../tools/headless/` (the headless helper, rebuilt on the new VM with Playwright) and the guard hook in the workspace's `.claude/settings.json`, which runs `scripts/hooks/guard-bash.py`. The physics benchmark repo `../space-game-physics-bench` was not copied to the new VM; nothing in the game needs it.)

- Linux VM without GPU, 17 cores, ~16 GB RAM (re-measured on the evening of 20 Oct 2026, after the VM grew again; it had 9 cores and ~9 GB earlier that day). **No sudo** (apt is unavailable). Native Bevy windows are pointless here (software rendering); look at the game through the web build.
- Rust via rustup (`source ~/.cargo/env` if `cargo` isn't on PATH): stable 1.99, targets `x86_64-unknown-linux-gnu`, `wasm32-unknown-unknown` and `wasm32-wasip1` (for `scripts/determinism-check.sh`). `trunk` 0.21.14 and `cargo-binstall` are in `~/.cargo/bin`. Node 24 via nvm. A cold `cargo test --workspace` took about 2.5 minutes on 16 cores (12 Oct 2026); expect longer now.
- crates.io and GitHub are reachable (crates.io API needs a User-Agent header: use a generic one like `space-game-dev-tools`; never put personal data such as names or email addresses into requests).
- Heavy builds (Bevy, physics engines): several agents may build at the same time, so pass `-j 5` (or `CARGO_BUILD_JOBS=5`) to keep RAM in check, and have at most three agents building at once.
- **Builds are cached and the web build is fast by default** (ADR-0034): every cargo build on the VM goes through sccache (`~/.cargo/config.toml`), so a worktree's own target dir reuses compiled dependencies. `scripts/web-build.sh` is the everyday build (profile `web-dev`, no wasm-opt, about 30 s after a change); `scripts/web-build.sh --full` is the optimised one (profile `web-release`: the client for size, the sim for speed, ADR-0049; about 15 minutes cold); it is what GitHub Pages gets.
- **Wait loops must end.** Wrap every polling loop in `timeout <secs> bash -c '…'`. Never wait with `pgrep -f "<pattern>"`: it matches the loop's own shell, so `until ! pgrep -f …` spins forever (this stuck an agent for 5 hours). Wait on a PID (`while kill -0 $PID; do …`), a file, or the command itself instead. Never delete a directory a server or build is still running from, and stop your servers when you're done.
- **Disk is tight** (48 GB total, ~11 GB free after pruning on 20 Oct 2026; ~26 GB free after the first full build on 12 Oct 2026; the old VM ran full twice). Every extra target dir costs 1–5 GB. Prefer the shared `target/`; if you need your own `CARGO_TARGET_DIR` to avoid lock contention, use `target/<your-name>` and **delete it when you finish**. Check `df -h /home` before big builds. `scripts/prune-target.py` (`--dry-run` to see; profiles as arguments, default `debug`) removes the old hashes of the workspace's own crates, tests and examples from `target/<profile>/deps` and `examples` and empties `incremental` (17 Oct 2026: gigabytes a day on a busy VM); run it before and after a long session, never during a build. **Never point two checkouts (e.g. git worktrees) at the same target dir**: cargo hashes path packages relative to the workspace root, so both write the same artifacts and `data`'s embedded content goes stale (`embedded_pack_matches_disk` fails; `touch crates/core/data/build.rs` fixes it).
- **The wiki** (`docs/wiki.md`): `cd wiki && npm install` once, then `npm run dev` (dev server), `npm run build`
  (runs `content-tool wiki-data .` first, then VitePress; about 10 s), `npm run check` (every page, link and search
  entry; nothing raster) and `npm run deploy` (build, check, copy to `../serve/wiki`: `http://10.77.9.2:8080/wiki/`).
  `cargo test -p content-tool` checks the hand-written pages against the content (unknown references, rule examples,
  navigation, raster images), so content changes that break the wiki fail the tests. Hand-written pages never type a
  name or number the data holds: `[[part:thruster]]`, `[[part:thruster.thrust]]`, `[[sim:tick_hz]]`.
- **Headless browser check:** `node /home/devbox/workspace/tools/headless/shot.mjs <url> <out.png> [waitMs] [w] [h]` opens a page in headless Chromium (WebGL2 via SwiftShader, slow but correct), prints console messages and page errors (wasm panics show up here), and saves a screenshot. Use it to verify any web work, then look at the PNG with the Read tool. SwiftShader timings say nothing about real performance. Both helpers are a mouse by default; `shot.mjs … --touch` (a touch screen, coarse pointer, at the given size) or `--phone[=dpr]` (a phone like DevTools' device mode, e.g. `844 390 --phone`), and `scripts/web-drive.mjs … --touch` or `--dpr=N`, emulate touch, which the game detects on its own (no URL flag since ADR-0046). For development screens use `?dev=1` (its own save, everything open, the whole galaxy, the debug keys); the tool flags (`scene`, `behavior`, `fight`, `load`, `seed`, `hashlog`/`hashticks`, `hud`, `slowmo`) are in `docs/client.md` "Launch parameters".
- **Every UI change is checked at three sizes** (Niklas, 14 Oct 2026, after a run of layout bugs that only showed on his screen): `scripts/ui-check.sh <url> <out-dir> "<web-drive steps>"` screenshots 1280×720, 1600×900 and 2560×1440 (the UI scaled ~1.8×, like his, where fractional pixels round unevenly). Look at all three; for anything small (a ring, a border, an icon edge) take a close-up with web-drive's `crop` step and `scripts/png-zoom.py`. Overlays drawn in the world over UI nodes (icons, scrims) take their geometry from the laid-out nodes (`ComputedNode`, `UiGlobalTransform`), never from copied layout numbers.
- The VM's address on the LAN side is `10.77.9.2` (new VM, 12 Oct 2026; the old one was `10.77.6.2`). Serve web builds with `--address 0.0.0.0` so Niklas can open them from the host browser (e.g. `http://10.77.9.2:8080`).
- **Automatic updates can kill the session.** Ubuntu's unattended-upgrades runs daily (and right after a boot if the VM was off at 06:30); its `needrestart` hook restarts services after library upgrades, including `vmdev-agent.service`, which hosts VS Code, Claude Code and every agent and build (13 Oct 2026: 8 restarts in 2 minutes, then a reboot for a new kernel). Fix (root, Niklas): `/etc/needrestart/conf.d/50-vmdev.conf` with `$nrconf{override_rc}{qr(^vmdev-agent\.service$)} = 0;`. Until it exists, commit work in small steps; after a kill, worktree agents can be resumed (their worktrees and transcripts survive).
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
| Style 2a | The chosen ship look: flat tiles with seams, machines drawn as their own components on hull-toned cells (GDR-0069), soft teal hull glow (coral for enemies). |
