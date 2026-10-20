# GDR-0131: The tractor beam: a utility part that pulls salvage and ore in, never ships; escorts with one fly `salvager`

- Status: Accepted (Niklas: the part, what it pulls, the reach, the salvage action and the escorts' switch, 20 Oct
  2026; every number and the details below lead defaults)
  20 Oct 2026 (later, Niklas, after the measurement below): the voyage runner's escorts with a tractor keep plain
  `escort`; the beam alone lifts the haul (5 to 18–20 items) and `salvager` on top cost about 12 goals of 230.
  `salvager` stays the player's choice. The Decision's "escorts with one fly `salvager`" is replaced by this.
- Date: 2026-10-20
- Decided by: Niklas (the design); lead session and its agent (numbers, details)
- Related: [GDR-0018](0018-weapon-families.md) (the utility family), [GDR-0025](0025-research-from-salvage.md) (loose
  debris by tractor), [GDR-0031](0031-materials-and-fabrication.md) (harvesting tools), [GDR-0063](0063-salvage-and-research-v1.md)
  (collecting by flying slowly over salvage), [GDR-0114](0114-mining-and-ore-refining.md) (ore), [GDR-0123](0123-behavior-words-for-salvage.md)
  (the `salvage` action, the `salvager`, the runner's escorts), [GDR-0129](0129-blinded-cargo-leader-near-blink-button-and-the-railgun.md),
  `docs/results/tractor-2026-10-20.md`

## Context

GDR-0018 names the tractor beam in the utility family, GDR-0025 and GDR-0031 as the way to pick up loose debris, and
GDR-0123 kept the voyage runner's escorts on `escort` until "a tractor or another salvage part can say so". Collecting
was only GDR-0063's: fly slowly over every piece.

## Options considered

- **What it pulls**: salvage and ore only, never a ship (chosen, Niklas); ships too (not chosen: it would be a weapon).
- **How it collects**: it pulls salvage in and slows it at the hull, and collecting stays GDR-0063's rule (chosen,
  Niklas: one collecting rule); collecting at range (not chosen).
- **How it pulls** (lead): a velocity goal towards the ship (closing at most `speed`, easing off at the hull, never
  below 1 m/s), its change capped at `force` over the salvage's mass, so heavy pieces come slowly (chosen); a plain
  force (not chosen: pieces would arrive fast and ram the hull). A beam never pushes salvage away (found in the
  tests: a ship flying at a wreck faster than the beam closes pushed it ahead of itself).
- **Power** (lead): a steady draw on the Low tier like the mining laser (chosen: simplest); a draw only while it holds
  something (not chosen: state of the last tick that would have to be hashed).
- **Who pulls** (lead): running vessels of owners that collect, with a free hold unit, salvage that fits it (chosen:
  enemies never collect, so their beams would only steal wrecks); everyone (not chosen).
- **The player's own use** (lead): automatic, no key (chosen: the simplest that fits the action map; it only works
  with hold room, and it never touches ships); a key or an assist (not chosen: one more action for something that
  should just happen).
- **The `salvage` action with a tractor** (lead): hold half the beam's reach short of the salvage (where the ship is,
  once that close) and let the beam pull (chosen); fly onto it as before (not chosen: what the tractor is for). The
  voyage runner's pilot does the same.
- **The runner's escorts with a tractor**: fly `salvager` (chosen, Niklas, GDR-0123's foreseen switch); escorts
  without one keep `escort`.
- **Hashing** (lead): nothing new: the pull lives in the salvage's velocities, the tick's beams are worked out from the
  state every tick (chosen: worlds without tractors run and hash exactly as before).

## Decision

- **The tractor beam** (`parts/tractor.ron`, category cargo, locked): 1×1, 3 build points, 1.5 t, draws 1.5 (Low),
  heat 0.4; `tractor: (range: 80, force: 40, speed: 5, beams: 2)`; salvage 3 utility data; recipe copper 3, crystal 1,
  steel 2. The utility research's **`tractor_beam`** (`research/salvage.ron`, 5 utility data). Icons `tractor.svg`,
  `tractor.ship.svg` (cargo grey: an emitter and its beam). **`sample_salvager`**: the rim skiff with a tractor for its
  scanner and cargo bays for its rear armour (hold 12), flying `salvager`.
- **The sim** (`combat/tractor.rs`, `docs/sim.md` "Tractor beams"): every tick before the physics step, each running
  vessel of a collecting owner with hold room and working tractors on its controlling piece takes hold of the nearest
  salvage within reach (from its centre of mass; loose items, salvage pieces, ore) that fits the hold, up to its beams,
  and pulls it; loose items weigh 1 t a cargo unit; held salvage does not settle.
- **The voyage runner**: escorts with a tractor fly `salvager`; the pilot with one stops short of salvage.
- **Measured** (`docs/results/tractor-2026-10-20.md`, 320 seeds, rim skiff with two escorts): sample salvagers flying
  `salvager` clear `yard_approach` 222 (haul median 18 items) and `halvex_drift` 152 (17), against plain rim skiffs'
  232 (5) and 162 (4); the same escorts flying `escort` clear 234 (20) and 150 (16). The beam brings the haul; the
  `salvager` adds little on top of it.

## Consequences

- Escorts with a tractor bring home three to four times the salvage at a cost of a few goals (the scanner and armour
  they trade for the beam and hold).
- The content's fingerprint changes (saved lab runs restore stale); fights and voyages without tractors are unchanged.
- Open: the client's beam look (a soft line to each held piece, `Sim::tractor_beams`) is not built yet, nor
  screenshots; whether escorts with a tractor should fly `escort` instead (the numbers say it does as well); a
  tractor on enemies (pulling wrecks away); tractors in the evolution lab's trials; every number above.
