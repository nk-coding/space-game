> Exported copy of the project notes (11 Oct 2026). The claude.ai artifact links below are Niklas's private pages; everything they show is also in this bundle under `boards/`. The generator scripts mentioned below are in `reference/generators/`.

# Space Game: Concept Notes

Backing notes for future sessions. **The main, user-facing version is the concept board artifact:**
https://claude.ai/artifact/MhdtJiNFHLnnwP2Ayruimh (revision 16, 11 Oct 2026). Update that page when the concept changes (read it with the Artifact tool, edit, republish to the same URL). Niklas prefers artifacts over markdown docs.

## Background

- Personal hobby project. Maybe a Play Store release in the far future; Steam unlikely (heavy AI use in development).
- v1 (previous iteration) had: a ship builder, working peer-to-peer multiplayer, and a scripting language for ship control.
- v1 lesson: scripting drove the ship at too low a level. It was tedious and ships easily went out of control. v2 needs a higher abstraction (node/behavior-tree like Gladiabots), with detailed scripting still possible.
- Tech preferences: Rust first. Dislikes C++ (only acceptable for physics if absolutely necessary). Avoid Godot. Flutter is acceptable for (parts of) the UI. Bevy sounds reasonable.

## Original brief

2D; Android, Windows, Linux; no real server (P2P ok); grid ship building from components (weapons, structure, energy, engines...); easy editor, no wiring; default navigation plus deep behavior customization; build/upgrade ships and use them in a vast world; AI enemies; long playtime with lots of content (alien races); efficient native code and 2D physics; location-based idea floated but feels odd; co-op raids maybe later.

## Decisions

| Topic | Direction | Status |
|---|---|---|
| Player role | Pilot one ship + small scripted escort | Settled |
| World | One seamless procedural open galaxy | Settled |
| Physics | Fully physical: mass, CoM, thrust torque, breakage | Settled |
| Hull | Distinct flat blocks with thin dark seams (no bevels) and a faint teal glow along the hull edge (style 2a). Parts have footprints (1×1, 1×2, 2×2, later 3×3+). Armor: blocks + slopes 1×1, 1×2, 1×3 (three angles), in materials (steel, composite, reactive, ...) | Settled |
| Ship systems | Shared power pool (per connected piece) + heat/radiators + adjacency bonuses; no wiring | Settled |
| Behavior | Layered: behavior tree (user edits) → built-in maneuvers → always-on flight computer. Trees have a text format (1 line per rule) | Settled |
| Scripting | Rhai for custom nodes (core is Rust). Niklas dislikes Lua's `end` | Leaning |
| Touch controls | Stick to move, tap to lock a target; flight computer keeps the nose on it. Without a lock the stick means 'fly that way' (push distance = speed). Letting go brakes with the ship's own thrusters by default; a drift toggle (player setting) turns braking off. Soft top speed from thrust and mass. PC: WASD = stick, mouse locks and fires | Settled |
| Weapons | All families: energy, kinetic (physical recoil), missiles, disruption, drones, utility | Settled |
| Tech | Big tech trees per family (~200 nodes total). Node kinds: new part, variant (trade-off), size class, refinement (short I–V chains), alien (needs reverse-engineering). NO Mk I–III levels. Cross-links between trees. All data-driven | Settled |
| Progression | Research data from dismantling salvaged parts (per family); alien nodes need artifacts/lore. Other sources undecided | Leaning |
| Balancing | AI-vs-AI arena on the headless simulation with evolutionary search (see below), MAP-Elites archive, automatic number tuning, Elo/TrueSkill ratings, reports, short CI run | Settled |
| Tech stack | All Rust. Engine-free simulation core crate. Bevy client for native and web (WebAssembly). No C++, no Godot. Next step: the spike (see Technical decisions) | Settled |
| Physics engine | Rapier2D in the core (pure Rust, standalone, determinism mode). box2d-rust (pure Rust port of Box2D 3.1) to watch. Avian ties core to Bevy | Leaning |
| Ship scale | ~60x60 minimum, capital ships if achievable | Settled |
| Death | Mild: bonded matter re-forms the ship at home for free; unbonded haul stays behind as a wreck | Settled |
| Economy | None (no credits, no trading); parts still have build-point costs for balancing/enemy scaling | Settled |
| Story | Light: procedural missions + lore on derelicts (lore also unlocks reverse-engineering) | Settled |
| Art | Ship style 2a: flat vector tiles, one flat colour per machine with its glowing icon on the tile, soft teal glow (coral for enemies); each color = one meaning. UI: reference look (floating HUD, tracked labels, pill buttons, dotted rings) | Settled |
| Multiplayer | P2P co-op, host-authoritative; raids later | Later |

## Technical decisions (10 Oct 2026)

- Stack: all Rust. Crates: `sim` (engine-free; ships, parts, physics, heat, power, damage, behavior trees, Rhai; fixed 30 Hz), `game` (Bevy client, 60 fps with interpolation), `net` (host-authoritative sessions, join codes, tiny relay), `arena` (headless fights, evolution, reports, determinism checks), `data` (RON; base game loaded like a mod). Flutter dropped (menus-only fallback if Bevy UI fails badly). No C++, no Godot.
- Targets: Android mid-range 2021+ (Snapdragon 7-series class, 6 GB), Windows, Linux, and the web. 60 fps render, 30 Hz fixed sim.
- Web (decided 11 Oct 2026): a FULL platform, not just a dev build. The same Rust code is compiled to WebAssembly and served as static files.
  - Single player, co-op, mods and saves all work in the browser.
  - Rendering: WebGL2 baseline, WebGPU where available.
  - Bevy's web build is probably single-threaded, so the web gets its own performance bar (set after measuring) and the drydock device warning knows about it.
  - Audio starts after the first tap or click.
  - Every crate must compile to wasm32: no threads or filesystem required inside sim; data comes in through the asset layer.
  - Co-op transport must work in browsers: WebRTC data channels with a WebSocket fallback; the relay doubles as the signalling server.
  - Mods on the web load from a picked zip or a URL. Saves use browser storage (IndexedDB) plus the same export.
  - Rapier's determinism docs list WASM among the covered targets; box2d-rust and Avian must be checked in the benchmark.
- Development setup (decided 11 Oct 2026):
  - Niklas develops in a Linux VM without GPU passthrough. Claude Code runs in the VM.
  - The sim, tests and arena run natively in the VM. The game is viewed as a web build served from the VM to the host browser, because native windows in the VM would be software-rendered.
  - Android APKs reach the flagship phone via adb over Wi-Fi (Wireless debugging), so the VM network must reach the phone.
- Determinism: cross-platform bit-exact. Fixed step, seeded per-system RNG streams, ordered containers only (no HashMap iteration in sim), software trig, no wall-clock, physics in deterministic mode, per-tick state hash in debug.
- Netcode: host-authoritative snapshots + interpolation + local prediction; determinism used for desync detection, replays, later arena rollback. Connect via join codes + tiny relay server (the only server piece).
- Mods from day one: mods folder with RON data + Rhai scripts, same loader as base game, versioned/validated schemas, deterministic script API, co-op requires matching mod list hash.
- Authoring: hand-edited RON files; enemy/guardian ships from drydock export; hot reload in dev.
- Saves: versioned with migrations; store seed + generator version + deltas (galaxy must stay expandable); export/import by file/share code now, optional cloud sync later.
- Text: English only at launch; all strings keyed in data.
- Simulation spaces: one per cluster (small local coords); warp sequence between clusters while loading; empty clusters freeze (respawn timers settled on arrival); cheap galaxy-wide event layer (Swarm raids, roaming bosses); co-op players may split → host simulates several spaces.
- Ship size: no fixed block cap; drydock measures simulation cost and warns per device; merged colliders + LOD.
- Spike (Bevy only), decided 10 Oct 2026:
  - Length: no fixed time box; it runs until every pass/fail question has an answer.
  - Who and where: Claude Code writes and runs it in the Linux VM, in a local folder with local git. No remote repo.
  - Test device: Niklas's flagship Android phone, so a headroom rule applies. Pass = 20 ships with debris at 60 fps using at most half the frame budget (about 8 ms per frame, sim step under 4 ms), plus phone temperature.
  - Steps:
    1. Physics benchmark (Rapier2D, box2d-rust, Avian): natively, on the phone and in the browser (wasm). The rest of the spike builds on the winner.
    2. Core sim with breakage and a same-hash test.
    3. Web, Android + feel: one ship in the 2a look (atlas tiles, icons, hull edge, glow sprites, bloom on and off) as a web build and an APK; stick + lock, WASD + mouse, 30 Hz with interpolation.
    4. Load test with 1, 5 and 20 ships. Browser numbers are measured first; the web bar is set from them.
    5. Cross-device state hash: native, phone and browser must match.
  - Units: 1 block = 1 m.
- Physics benchmark: FIRST STEP OF THE SPIKE (user decision, 10 Oct 2026; was 'later'). Candidates Rapier2D (determinism mode), box2d-rust, Avian 2D (ties core to Bevy ECS); Box2D 3.1 C as reference only. Scenarios: 20 compound ships of 200–2000 blocks, 2000 debris chunks, 5000 raycasts/tick, breakage every second, asteroid field. Metrics: mean/p95 step at 30 Hz, single/multi-thread, desktop + phone, cross-platform hash.
- Repository: closed for now; DO NOT create repos.

## Development start (11 Oct 2026)

- Work started in the VM repo `space-game` (branch `dev`, local git only). The concept bundle lives under `docs/concept/`, art sources under `art/`.
- **Web first** (Niklas, 11 Oct 2026): the web build is what matters now. Android comes later, so the phone parts of the spike (APK, phone timings, temperature, phone hash) are deferred. Everything stays Android-compatible.
- **Minimal content at the start**: build the systems first, content later.
- **Wiki later, content wiki-ready now** (Niklas, 11 Oct 2026): all content (parts, materials, ships, later races, tech, lore) is organised so the same data files feed both the game and a future wiki. Details in `docs/content-model.md`.
- The spike is built as the foundation of the real game, not as throwaway code. Plan and status: `docs/roadmap.md`.
- Provisional sim rules (lead session, 11 Oct 2026; concept-consistent defaults, Niklas may change them; details in `docs/sim.md`):
  - After a split, only the controlling piece (the one holding the controlling logic core) stays active; every other piece goes dark and becomes salvage, even if it has a core.
  - A logic core needs power: an unpowered piece goes dark (supports "kill all power → capture").
  - Brown-out order when supply < demand: logic → shields → engines → weapons → other (fixed default; per-ship reordering maybe later).
  - Heat is simulated per part (not per cell) until a design needs hot spots.
  - Chain reactions come later as an `explosion` block in content (reactors, capacitors, drives).
  - Capacitor bonus applies to parts tagged `laser` (as its text says), not to every gun.
  - Guns fire along their rotated facing.
- Open (asked 11 Oct 2026): how ships turn. With thrusters only, the Halvex Runner turns 90° in ~9 s and the raider can't turn at all. Until Niklas decides, the flight computer has an EXPERIMENTAL "turn assist" torque that can be switched off to compare.
- Open (asked 11 Oct 2026): Halvex Runner sample layout: its four radiators are boxed in (never vent), both wing-tip pulse lasers are blocked by the slope in front, and a boxed-in shield emitter overheats after ~8 s at full load.

## Combat (decided 10 Oct 2026)

- Pace scales with ship size (small = fast/twitchy, big = slow/heavy, same physics), overall leaning fast/arcade. Mid-range fights (1–2 screens, smooth zoom); weapon ranges differ. Always real-time, no pause/slow-mo.
- Aiming: fixed guns aimed by turning + target lock; turrets via behavior trees; player can set target priorities (engines first, weapons first, nearest threat).
- Damage: outer blocks first; penetration by type (laser burns first block, railgun pierces ~3, missiles splash); slopes deflect kinetic shots at shallow incidence (>60° from face normal); chain reactions (reactors, capacitors, drives explode); ramming damage by mass and speed (ram prows are a build).
- Shields: a bubble per emitter over nearby blocks, overlapping bubbles stack, need power. Energy absorbed well, kinetics partly pass, missiles detonate on the bubble.
- Overheat: shutdown by default; overdrive option for burst output at risk of damage/explosion.
- Repairs mid-fight: repair drones (use repair material, can be shot down).
- Capture: kill all power → dark logic core → copy yourself in.
- Sensors moderate (targeting range, scan detail; nearby always visible). HUD minimal + hull map of own ship.
- Race styles: automatons small packs; Grown regenerate + ram; Swarm drone floods from hive carriers; Lattice long-range prisms + refractive crystal armor; Phase walkers blink strikes with armor-piercing lances; guardians breakable machines whose behavior changes as sections break.
- Board has a damage sandbox (laser/railgun/missile vs sample or drydock ship; click to fire at CoM, drag to aim; shows ricochets, shield bubbles, chain explosions, hull-map HUD).

## World (decided 10 Oct 2026, interactive Q&A)

- Player: an awakened ship-mind that wakes in an abandoned precursor shipyard at the rim; the shipyard is the fixed home (drydock + research). Story light.
- Galaxy: one seamless procedural galaxy of star clusters linked by lanes; handcrafted set pieces placed in; new seed per game. Board sketch uses ~80 clusters.
- Journey inward: rim → outer reach → middle reach → inner reach → core (precursor archive: final gate + best tech). ~50 h to the core; core loop (explore, bigger ships, co-op/arena) continues after. New game+ decided later.
- Gating: difficulty by distance + race territory + environmental hazards: stellar heat (outer), ion storms & nebulae (middle), radiation rising (inner → core). A precursor guardian at every band gate; beat it to add the gate to your network.
- Bands/races: Rim = rogue automatons (feral precursor drones, early foes, salvage-rich); Outer = The Grown (wary; organic regrowing hulls; boss: giant Grown ships); Middle = The Swarm (hostile by nature, no standing; hives = bosses; raids neighbours); Inner = The Lattice (neutral, open to standing, likely allies) + Phase walkers (wary; phase tech; giant phase ships). Precursors gone: gates, guardians, megastructures, archive, and you.
- Races exist mainly for variety and tech (each teaches its own branch); access, allies, missions exist but stay light. Races rarely clash (they stay in their regions), the Swarm is the exception.
- World state: static; cleared encounters come back slowly over time.
- Clusters: large, 10+ min to cross; a default voyage covers one cluster (15–30 min, phone-friendly). Longer voyage types exist (deep expeditions, boss hunts, megastructure dives) but are never forced.
- Activities in clusters: fights, salvage & scanning, light missions (from race stations). (Design challenges/puzzles not chosen as a main activity.)
- Travel: cruise inside clusters, warp along lanes for shorter hops, precursor gates for long distances.
- Supplies per voyage: ammo, fuel, repair material; refilled at home; outposts refill + heavy repairs. Long-term only research matters.
- Haul: salvage only becomes research once brought home. Death = respawn home with ship, part of haul left as a wreck.
- Salvage: loose debris by fly-through or tractor beam; intact parts on big wrecks cut free. Cargo bays are ship parts (cargo 1×1 = 4 units, hold 2×2 = 20) → design trade-off.
- Scanning: research data (counts once home), persistent cluster map, lore, enemy intel (parts + behavior tree).
- Content types: derelicts/graveyards, race stations, outposts, bosses (precursor guardians, swarm hives, giant ships), megastructures/anomalies; more added over development (all data).
- Fleet secondary: escorts built at home (command capacity), drone hangars, capture disabled ships by copying yourself into their logic core; faction allies maybe.
- Enemy designs: handcrafted + evolved offline (curated per race style) + runtime variation (not full generation).
- Multiplayer: co-op voyages in the host's galaxy; arena PvP with your actual ship. Bosses solo-able, easier in co-op.
- Board has an interactive galaxy generator (seeded) with clickable cluster close-ups.
- Shipyard (home): research lab, fabricator (parts + escorts; upgrades unlock bigger/alien parts), test range (simulated fights vs archived enemies and own past designs).
- Ship size growth: drydock expansions (frigate/cruiser/capital frames) + mind capacity (how many parts the ship-mind controls; raised by logic cores and research).
- Part cost = bonded matter: a new part consumes its materials once to bond its precursor programmable matter to your mind. Destroyed ships' bonded matter is recalled through the gates and re-forms at home (rebuild free). The haul isn't bonded, so it stays behind as a wreck. Refitting returns a removed part's matter.
- Guardians: huge breakable precursor machines built from parts; their parts are precursor tech loot. Solo-able.
- Allies: only inside specific race missions.
- First hour: wake as a tiny drone in a dead shipyard → salvage nearby automaton wrecks → repair the drydock → build first real ship → first rim voyage → first research.
- Galaxy size ~80 clusters for now; MUST stay expandable (cluster count, band sizes, races, set pieces are generator params/data; saves must tolerate added clusters/bands).

## Materials and fabrication (decided 10 Oct 2026)

- No generic scrap. 30+ materials tied to where you've been (board sketch: 46 = 25 raw + 21 refined).
- Sources: harvesting in clusters (asteroids, comets, gas, star skimming), salvage (dismantling returns a part's materials + research data), race-specific sources (Grown tissue/spores/heartwood, Swarm chitin/resin/nerve cores, Lattice crystal shards/resonant crystal, Phase residue/rift silt), precursor sites (precursor alloy, programmable matter, gate shards, archive keys), mission rewards (rarest race materials).
- Materials follow hazards: He-3 needs heat shielding (star skimming), ionized gas from ion storms, hot isotopes from radiation zones.
- Recipes are chains, but NO factory buildings/logistics: the fabricator resolves chains automatically if raw materials are in storage and the hub has the needed upgrades (smelter, kiln, cryo plant, electronics bench, magnetics lab, fuel works, munitions bay, fusion workshop; race: bio-vat, hive press, crystal annealer, phase chamber; precursor forge last).
- Bonded matter: a new part consumes its materials once; destroyed ships re-form free at home; refit returns matter. Salvaged parts: install as-is (small programmable-matter bonding cost) or dismantle for research + materials.
- Every logic core needs programmable matter (hosts a copy of you); automaton cores at the rim are the first source.
- Harvesting: tool parts (mining laser, gas scoop, tractor) + harvest drones later.
- Storage at home unlimited; voyage cargo hold is the only limit. Every source refills slowly (rare ones too). Fabrication only at home; on voyages repair material only patches damage.
- Board has an interactive recipe explorer (23 example parts) and a material catalog.

## Evolution / balancing design

- Genome: part list (optionally mirror-locked) + behavior tree.
- Operators: add/remove/move/swap parts, change materials; reorder rules, nudge thresholds, swap subtrees; crossover splices hull sections and subtrees.
- Fitness from real fights: win rate vs a rotating pool (hall of fame of past champions + hand-made archetypes) to avoid overfitting.
- Diversity: MAP-Elites archive (best design per trait niche, e.g. speed × range); empty niches show unviable designs.
- Outer loop tunes part stats to maximize viable niches / reduce dominance; human reviews proposals.
- Exploit hunting: unconstrained runs to find ramming/spinning/recoil/AI loopholes; each becomes a regression scenario.
- Board has a live toy "Evolution lab" (in-browser GA on the drydock rules, formula fitness, 60 build points, 30×80). First run found: engines mid-ship/at the nose (rules only require open space behind an engine), no radiators, long reactive 1×3 slope wings.

## Tech research (10 Oct 2026)

- Bevy 0.20 released 8 Oct 2026 (Feathers widgets: color input, list view, dropdown, menus; BSN scenes progressing; sprite renderer rebuilt; custom 2D/sprite materials). Breaking releases every ~4–5 months. bevy_egui + egui-snarl available for dev tools/node graph prototyping.
- flutter_rust_bridge 2.12 stable (2.13/2.14 betas).
- box2d-rust 1.4.0 (18 Sep 2026): pure Rust port of Box2D 3.1, bit-exact with C in FallingHinges determinism test.
- Box2D v3 itself is C (not C++), bindable via bindgen. Box2D 3.1 lists cross-platform determinism, SIMD.
- Rapier: determinism guide/mode.
- Godot (ruled out by Niklas): 4.7.x latest.

## Scripting candidates (researched 10 Oct 2026)

- Rhai (Rust): braces, built-in max-operations limit, only registered API visible; tree-walking interpreter (fine for goal-level logic). → chosen direction.
- pocketpy, QuickJS, Godot Sandbox, WASM (expert tier), Lua/Luau (rejected, `end`).

## UI (decided 10 Oct 2026)

Mockups: Design canvas "Space Game UI Mockups", https://claude.ai/artifact/T3u2Bdp7jB6zpLJLRdBAjn (6 boards: phone drydock, flight HUD, home ring menu, behavior editor, galaxy map + autopilot; desktop drydock 1280×720).

- Orientation: landscape only (phone and desktop).
- Drydock input: paint for blocks/armor (drag to paint, material picked once), drag and drop for big machines (reactors, drives, holds). Live stats (power, heat, thrust/mass, yaw, firepower, cargo, mind capacity, device load) + test-fly straight from the drydock. Overlays: heat, mass, bonus (adjacency), shields, arcs.
- Desktop drydock adds: search + category tabs in the palette, keyboard shortcuts, part inspector with variant picker (stat diff preview), upgrade materials (bonded once) with "missing → where to find it" and "track on map".
- Behavior editor: node graph on all platforms (priority list of rules: condition → maneuver), Rhai nodes tagged, limited rule slots per logic core, graph/text toggle, "test in range".
- Style: diegetic "ship-mind interface": thin lines, corner brackets, mono labels, same palette as the board; no skeuomorphic cockpit.
- Flight HUD: customizable (move/hide widgets), classic default: hull map + supplies top-left, target panel with priority chips top-right, stick bottom-left, fire + ability buttons bottom-right, off-screen markers.
- Home: ring menu around the docked ship (drydock, research, fabricator, test range, launch voyage, storage, galaxy map, behavior).
- Travel: autopilot for transit (route over lanes on the galaxy map, supplies check), manual in fights; auto hand-over when a hostile locks on.

## Ship look (in progress, 10 Oct 2026)

- Feedback: ships looked too blocky in flight; the UI and ship look should be "a bit" fancier. "UI" here mainly means the ship-related screens.
- Style reference from Niklas: a dark blue-black, bioluminescent look. Glowing teal line art (#8cefdb), a gold accent (#eed595) and white numbers. Floating HUD numbers with wide-tracked uppercase labels, no boxed panels. Dotted rings, particles, soft halos. Background #050e15 → #010409.
- Decided direction: the drydock stays a grid of tiles. In flight, a cosmetic skin pass applies; physics and damage still use the cells.
- Silhouette options (mockups canvas, "Ship look" board):
  - A plated (2 px chamfers)
  - B sculpted (rounded corners, filleted steps, internals hidden under 2×2 plates; recommended)
  - C streamlined (cosmetic fairings)
- Style options artifact: "Ship Style Options", https://claude.ai/artifact/Rje4XirT3EEV1s56EvMhpa
  1. Metal: shaded lit plating, glass HUD panels.
  2. Luminous: closest to the reference. Dark hull, soft teal edge glow, aura, particle trails, floating HUD.
  3. Neon line: hull drawn as glowing line art, strongest glow.
  4. Hybrid: lit plating with a faint glow, in the reference atmosphere with a floating HUD.
- Revision 2 of the options, after feedback that the ship looked weird and Niklas wants to see the blocks:
  - The smoothed skins are dropped. In flight, every part stays its own visible tile with seams (same as the drydock).
  - New sample ship: 12×7 cells, arrowhead shape. Slopes form the leading and trailing edges, beams sit at the nose with the fusion reactor right behind them, pulse turrets on the wing tips, torch drive plus 2 thrusters at the tail.
  - All four options use the identical block layout, glyphs, scene and floating HUD. Only the tile surface and glow differ: metal / dark + soft teal edge / neon seams / mid-tone + faint glow.
- Round 2 feedback:
  - Not 1 or 3; something between 2 and 4, with 2 the favourite.
  - He dislikes the 3D bevel look of the blocks ("outdated ppt shapes").
- Round 2 boards, all flat with no bevels and no per-block gradients, same scene and HUD:
  - 2a flat dark tiles, thin seams, teal edge glow
  - 2b flat + one soft light across the whole hull
  - 2c flat tiles with small gaps and slightly rounded corners
  - 2d flat mid-tone (4 without the 3D), faint glow
- DECIDED (10 Oct 2026): style 2a, refined:
  - Flat blocks in dark teal-grey (#142a2e) with thin dark seams (#03070a). No bevels, no per-block gradients.
  - Machine tiles are one flat colour (#0f1c25) with the glowing icon placed directly on it (no dark inner square).
  - Soft teal hull glow (halo .18, line .32) and a faint aura.
  - Particle engine trails, dotted shield rings.
  - Enemies use the same tiles with a coral glow.
  - The drydock and flight use the identical tiles.
  - Applied to every board on the mockups canvas (flight HUD, phone and desktop drydock, home ring menu, galaxy map, behavior editor). All of them also use the reference UI look: floating tracked labels, pill buttons, glowing ring buttons, dotted rings, particles, and a dark blue-black backdrop.
  - The old flight-skin experiment board is replaced by "Ship parts · style sheet". It shows every part tile (armor materials, slopes, power/heat, defense/control, cargo, propulsion, weapons, automaton core) and the sample ship "Halvex Runner" (12×7 cells).
  - Galaxy map details: dotted band rings with the gate ring in coral, and a glowing dotted route. The voyage panel holds destination chips, route legs with warp times, supply bars, autopilot toggles and SET COURSE.
  - Concept board revision 14 (10 Oct 2026) renders every ship in style 2a: drydock sketch, part palette, hull and machine sheets, early/mid compare, evolution lab, damage sandbox.
    - Icons are the exact style 2a glyphs embedded as SVG images. The hull glow follows the outer edges of the main piece, so split-off pieces lose it.
    - Shields in the sandbox are dotted rings.
    - Art direction text, the Hull/Art decisions and a new UI decision row were updated. Page accents moved to teal #8cefdb, coral #ec8f7c and shield #8fb0f2.
- Generator scripts live in the session scratchpad only (gen2/style2a.py, frag.py, rollout.py, rollout2.py). A future session should rebuild from the artboards.

## Drydock sketch (in the artifact, placeholder numbers)

Grid 16×12, mirror axis between columns 7|8. Parts: Block, Slope 1×1/1×2/1×3 (auto-orient, click to cycle 8 orientations, Bevel converts outer block corners), Fission 1×1 (10 power), Fusion 2×2 (30), Capacitor, Pulse laser 1×1, Beam laser 1×2, Shield, Radiator, Thruster 1×1 (12), Torch drive 2×2 (45), Logic core. Materials: steel (mass 2/hp 1 per cell), composite (1.5/1.4), reactive (2.4/2.1); slopes scale by area. Build points: block 1, slopes .5/1/1.5 (×1/1.5/2 by material), fission 4, fusion 10, cap 2, pulse 3, beam 7, shield 3, radiator 2, thruster 3, torch 9, core 3. Heat = steady-state cell diffusion; overheated parts shut down. Guns need a clear column ahead; engines need open cells behind.

## Still open

Pre-spike questions: all answered 10 Oct 2026 (see Technical decisions → Spike and Touch controls). Glow method (sprites vs bloom) is measured in the spike.

Can wait until after the spike:
- Spike results (Bevy on a mid-range phone, 60 fps with 20 ships?)
- Physics benchmark: performance is top priority. Compare Rapier2D, box2d-rust, Avian (C Box2D only as reference) on game-like scenarios (compound ships, debris, thousands of raycasts, breakage) on desktop and phone. Note: this sandbox can't download rustup components (static.rust-lang.org blocked) but has cargo 1.97 installed.
- Mind capacity numbers (start value, growth)
- Engines anywhere vs. rear only? (evolution found nose/mid-ship engines are legal)
- Tree size per family that stays fun
- Working title

## Parked

- Location-based play (light version: real location seeds home region)
- Co-op raids
- Shared blueprints + behavior trees as AI opponents (bounty hunters, async PvP); the balance arena already does this internally
