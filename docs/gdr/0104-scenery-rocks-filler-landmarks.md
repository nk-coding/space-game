# GDR-0104: Scenery between the sites, lead defaults: anchored asteroid rock as real bodies placed by the cluster's scenery, drawn-only dust, debris, derelicts, landmarks, a hot star's corona and distant fights

- Status: Accepted (provisional, lead), 17 Oct 2026, for Niklas to review
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
