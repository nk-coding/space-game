# GDR-0104: Scenery between the sites, lead defaults: anchored asteroid rock as real bodies placed by the cluster's scenery, drawn-only dust, debris, derelicts, landmarks, a hot star's corona and distant fights

- Status: Accepted (provisional, lead), 17 Oct 2026, for Niklas to review.
  2026-10-17: the rocks spawned so far show on the minimap and the cluster map as faint specks ([GDR-0106](0106-flight-minimap.md)).
  2026-10-17: after Niklas's play-test ([GDR-0107](0107-depth-layers-zoom-and-draw-order.md)): the distant fights removed (code, content field `scenery.fights`, tokens); every layer scales with the zoom by its depth; landmarks moved behind the sky's planet (parallax 0.03–0.04, sizes in metres of their layer at 18 px/m); rocks drawn finer than their colliders (rounded, roughened outlines, shading and craters).
  2026-10-18, later (Niklas chose the backdrop study's approach C for rocks, "soft bands, vector detail"; numbers lead defaults): a rock's outline is the sim's own and is what it collides with: the convex hull of its kind's corners, then control points round it (corners rounded in to 0.9–0.95, each edge's middle dented in by up to the kind's new `dent`: iron 0.07, ice 0.04, slag 0.16, every other edge a second dent point a third along it) and through them a periodic spline of radius over angle (`rocks::curve`, 96 points, star-shaped, so it never crosses itself). A rock dented more than 0.05 of its radius below the curve's hull collides as two or three convex pieces cut through its centre at its deepest dents (`rocks::pieces`, one Rapier compound), so shots and rams land on what is drawn (of 150 rolled rocks 93 are one piece, 44 two, 13 three). Steering and the pilot assist keep the circle round the rock. The client draws it as the study's C: five soft bands towards the light, rim light, a ridge, craters with lips and floors, speckle, the kind's marks (`RockKind::look` in content: iron the most craters and a metal glint, ice bright cracks, slag small pits and ember veins on its shadow side; glints and embers in a new additive atlas material) and one mitred edge strip; colours per tone (`world.rock.<tone>` shadow / mid / lit / edge). The light comes from the sky: the hot star in a scorched cluster (from each rock towards it), else the sky's first planet's light, else the upper left. Only voyages with rock hash anew; the slice's curve stays within the noise (120 seeds, `docs/sim.md`). Screenshots `docs/results/rocks-c-field.png`, `rocks-c-iron.png`, `rocks-c-ice.png`, `rocks-c-slag-scorched.png`.
  2026-10-18: the ore materials are not in any recipe yet ([GDR-0108](0108-materials-economy-workshop.md): copper, ceramic and lattice crystal come from dismantling); the fabricator's freed slot at home could hold a refinery for them.
  2026-10-18, later (the stuck check's follow-up): in 6 of the slice's 1920 runs a vessel stayed on a rock 90 ticks or more (mostly the scripted flagship, up to 4814). Causes and fixes: steering left a ship at rest alone, so one braking or holding drifted onto a rock and stayed (now pushed off at 2 m/s while its circle overlaps the rock's); inside two rocks' gaps it was pushed out of the first listed onto the nearer one (now the deepest gap); the scripted pilot under a lock pushed only along its nose (it now lets go of the lock while against a rock, `Mover::against`); and a damaged Runner whose turn was blocked the short way by the rock it lay against waited for the turn for 5106 ticks (the flight computer now turns the long way round after 10 ticks of a blocked turn, `TurnAssist::long_way`). Rocks stay circles to steering. Stuck runs 6 → 0 of 1920 (longest contact 71 ticks); the slice's clear rates within the noise at 120 seeds; native = wasm, new hashes in `docs/sim.md`.
  2026-10-19: mining built ([GDR-0114](0114-mining-and-ore-refining.md)): a rock kind's `ore` per square metre of its circle (iron 0.02, ice 0.03, slag 0.015), cut by the mining laser's beam into the hold as its material; a spent rock stays as cover and is drawn darker. `RockHit` stays the event of every shot on rock; `Mined` follows a mining beam's cut.
- Date: 2026-10-17
- Decided by: lead session (defaults for Niklas's request of 17 Oct 2026: "currently flying somewhere is extremely
  empty except the few destinations"; future content adds more, so data-driven and extensible)
- Related: [GDR-0080](0080-exploration-searching-and-story.md), [GDR-0094](0094-galaxy-generator-defaults.md), [GDR-0095](0095-sensors-hidden-sites-and-scans.md), [GDR-0102](0102-grown-mechanics-and-parts-defaults.md), [GDR-0031](0031-materials-and-fabrication.md), [GDR-0047](0047-ramming-crushes-inward-bumps-free.md), [GDR-0088](0088-pilot-assists-weapon-groups-and-key-bindings.md), [GDR-0100](0100-fight-balance-targets.md), `docs/results/2026-10-16-bigger-clusters.md` (E0), `docs/results/scenery-2026-10-17.md`

## Context

The galaxy's clusters are 2–3 km across (GDR-0080, GDR-0094) and hold a handful of sites; between them there was only
the backdrop. Niklas asked for space that is less empty on the way: debris, dust and drifting wrecks, asteroid fields
that matter in a fight, landmarks and distant fights, all from data so that later content can add more. Mining comes
later (GDR-0031); rocks must leave a hook for it. E0 set the cost rules: a cluster in one `f32` space, records until
the fleet comes near, spawned things asleep.

## Options considered

- **What a rock is in the sim**: an anchored (fixed) Rapier body with a convex outline (chosen: costs the solver
  nothing, never drifts out of place, gives stable cover; the broad phase and ray casts see it); a very heavy dynamic
  body (not chosen: costs a body a tick like a sleeping ship, E0's 0.6–1.3 µs each, and can be pushed); a sim-side
  circle test outside Rapier (not chosen: shots, rams and the regrowth's overlap test would each need their own path).
- **How a rock is keyed**: a body key under a ship id no ship gets (`BodyKey::ROCKS`, the rock's index as the piece;
  chosen: rocks sort after every ship, contacts and ray hits keep one type, unknown-ship look-ups fail as they should);
  a second key space in the physics (not chosen: every contact and hit path would grow a branch).
- **Ramming a rock**: the existing rule (GDR-0047: damage from the impulse above the threshold, crushing inward),
  scaled by the rock kind's `ram` (chosen, lead default iron and slag 0.5, ice 0.35: against an anchored body the
  impulse carries the ship's whole mass, so an unscaled ram hurt two to three times as much as one on a ship of the
  same mass; the slice's clear rates fell up to 19 points); the unscaled rule (measured, not chosen).
- **Placement**: per cluster type and per voyage in content (`scenery.rocks`: fields, belts, scattered rock; chosen),
  rolled in the sim's voyage plan after the sites on a stream of its own (`"rock"`), so the sites lie where they lay;
  spawned field by field (a belt in pieces of about 180 m, a scattered rock alone) when a player vessel comes within
  the spawn radius of its reach, as sites spawn. Rocks keep 70 m from every site, 150 m from the entry, 4 m from each
  other (lead defaults).
- **AI near rocks**: behaviors' and maneuvers' velocity goals bend round the first rock gap the way enters (tangent
  past the near side, three passes, a look-ahead of at least 3 s and a second more than it takes to stop); a way
  still blocked (a gap too narrow) slows to what it can stop from; a ship whose own motion carries it into a rock is
  bent aside first (chosen); no avoidance (not chosen: ships rammed rocks and pressed against them). The pilot's
  flight is never steered; the assist `avoid_collisions` sidesteps rocks like pieces. The voyage runner's scripted
  pilot steers its stick the same way, holds fire with a rock in its line of fire and slides aside, and lets go of its
  lock when carried into a rock (a stand-in for a player who sees the rock).
- **Filler**: drawn by the client only, seeded from the voyage's seed (chosen: no sim cost, no effect on fights,
  free to change); sim bodies (not chosen: cost and clutter in fights).
- **Where landmarks and fights hang**: on parallax layers far behind the play (chosen: they read as far off and can
  never be reached); in the play plane (not chosen: a station you can fly through).

## Decision

- **Content** (data-driven, two new kinds and one block): `rocks.ron` (rock kinds: material, size, corners,
  roughness, stretch, `ram`, tone), `landmarks.ron` (form `Station`, `Beacon`, `Ring` or `Corona`, size, parallax),
  and `scenery` on cluster types and voyages: rock groups (`Field(radius)`, `Belt(width)`, `Scatter`; how many and how
  many rocks each), `dust`, `debris` (densities), `derelicts` (min, max), `landmarks` (each with a chance; a corona only
  beside a hot star) and `fights` (a minute). Two ore materials, nickel-iron and water ice, are what rock is made of:
  the mining hook (nothing gives them yet).
- **Rocks in the sim** (`sim::voyage::rocks`): anchored convex bodies; shots and plumes stop on them
  (`SimEvent::RockHit`, the rock takes nothing); rams as above; behaviors steer round them; spawned lazily; in the
  state hash only when a voyage has rocks.
- **Lead defaults in content**: the rim's wreck drift a field or two of iron rock and 30–45 scattered; the hunting
  reach a belt of 40–60; the quiet field two or three ice fields; the scorched deep slag fields and a corona; the outer
  reach ice; the slice's first three voyages a field, a field and scattered rock, a thin belt; the Warden's gate no
  rock (the curve, GDR-0100). Filler densities and landmark chances per type in `cluster_types/*.ron` and
  `voyages/rim.ron`.
- **Client** (`world::scenery`): rocks as they spawn (one mesh per spawn group), glints on a near layer (parallax
  0.85), hull debris on a tiled layer just under the play (0.97) and thicker round the sites in drifting groups,
  derelict shells along the way (0.9, turning slowly), landmarks on their layer between the backdrop's stars and dust,
  the corona at the view's edge towards the star, distant fights on a layer of 0.06 drawn the same size at any zoom.
  Everything moves with the sim's clock (pauses with P).

## Consequences

- The slice gets a little harder with rock (120 seeds, `docs/results/scenery-2026-10-17.md`): its curve holds (each
  voyage at most as easy as the one before), the Warden unchanged; Corran Reach and Halvex Drift lose up to 13 points
  for the Runner. Re-tune counts if the curve should stay exactly where GDR-0100 put it.
- Fight codes and lab runs made before are fought under other rules (the content's fingerprint covers voyages and
  rock kinds). Generated interiors carry their cluster type (`galaxy::Interior::kind`), so the interiors' hash changed;
  the skeletons did not.
- A player flying by hand rams rocks at speed and takes real damage: rocks are hazards as well as cover.
- Mining (GDR-0031) needs a laser part, a rock's hit points or yield, and `RockHit` turned into ore; the rock kind's
  `material` already names what it gives. Drifting or breakable rock, rocks in the test range and the cluster map's
  rocks are open.

Note (17 Oct 2026): a cluster's sky (a planet, moons, a nebula, a far galaxy) is drawn by the same client feature
(`world::scenery::sky`) behind everything here, from a `sky` block on cluster types and bands, and the backdrop's far
stars move at 0.02 of the camera's motion (0.12 before), farther than the sky's planet
([GDR-0105](0105-cluster-sky-planets-nebula-galaxy.md)).
