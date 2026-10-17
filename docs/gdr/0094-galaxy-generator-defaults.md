# GDR-0094: The galaxy generator's defaults: clusters at 0.9 S fill five ring bands (about 80, most of them in the rim), three hidden gates per band, trails walked towards home, the rim real and the rest stubs

- Status: Accepted (provisional, lead). 2026-10-16: the open sensor range (E3) and how far sites stream set in
  [GDR-0095](0095-sensors-hidden-sites-and-scans.md) (800 m on cores; records until 600 m, no despawning); a cluster's
  interior rolls into a voyage plan (`sim::VoyagePlan::roll_interior`), its trail places signals' sites.
  2026-10-16: the format-2 migration of slice careers and the jump-range nodes set in [GDR-0096](0096-career-galaxy-format-3.md) (track E4).
  2026-10-17: where the slice's voyages go set in [GDR-0098](0098-slice-in-the-galaxy-and-rim-rewards.md) (set pieces placed with home, the Warden on the rim's
  first gate, whose trail is the slice); home needs 4 rim clusters without a set piece within range (was 3
  neighbours); fresh signal areas 3–4 S; the caches', lore's, graveyards' and gates' own encounters (track E8).
  2026-10-17: Accepted (Niklas, 17 Oct 2026): the galaxy's shape and pace are kept (with [GDR-0098](0098-slice-in-the-galaxy-and-rim-rewards.md)).
  2026-10-17: the outer reach's first own cluster type, `scorched_deep` beside a hot star (a stub with the rim's
  automatons standing in; weight 1 of 3), and a cluster type's optional `star` rolled into the interior after its sites
  ([GDR-0102](0102-grown-mechanics-and-parts-defaults.md)).
  2026-10-18: a band's cluster type may be rolled only near the band's outer edge (`border`); the outer reach is no
  longer a stub (its types, gates and trails in [GDR-0103](0103-the-grown-in-play-defaults.md)); generator version 3.
  2026-10-17: cluster types gain `scenery` ([GDR-0104](0104-scenery-rocks-filler-landmarks.md)): rock fields, belts and scattered rock rolled into the voyage plan after the sites, and drawn-only dust, debris, derelicts, landmarks and distant fights; a generated interior carries its type (`Interior::kind`), so the interiors' hash changed (the skeletons' did not).
- Date: 2026-10-16
- Decided by: lead session (defaults for Niklas to review, galaxy tracks E1 and E2)
- Supersedes: the plan's placeholder cluster counts per band (12, 18, 22, 24, 4; `docs/plans/galaxy-voyages.md` 4.8; not a record)
- Related: [GDR-0026](0026-procedural-galaxy-and-bands.md), [GDR-0078](0078-galaxy-exploration-v1.md), [GDR-0080](0080-exploration-searching-and-story.md), [GDR-0093](0093-galaxy-exploration-decisions-16-oct.md), [ADR-0035](../adr/0035-saves-keep-the-galaxy-skeleton.md), [ADR-0042](../adr/0042-galaxy-generator-crate.md)

## Context

Tracks E1 (the galaxy's content kinds) and E2 (the generator) need numbers: how far apart clusters lie, how big the
galaxy and its bands are, where home and the gates go, how trails are laid. Niklas fixed the unit (S, the slice's
spacing) and the drive (2.5 S at the start, 4 S at most) in GDR-0093. The plan's counts per band (rim 12, outer 18,
middle 22, inner 24, core 4) do not fit them: at the slice's spacing a rim of 12 clusters around the whole disc leaves
gaps of about 3 S between neighbours, out of a 2.5 S drive's reach, and a disc small enough to close them would crowd
the inner 68 clusters at half the spacing.

## Options considered

- **Counts per band as content, rejection sampling** (the plan): needs re-rolls until each band connects, fails
  outright for a 12-cluster rim at this spacing.
- **Clusters fill the disc at one spacing; counts follow the bands' areas** (chosen): a maximal Poisson-disc packing
  (Bridson) leaves no spot of a band further than about one spacing from a cluster, so neighbours are at most about
  two spacings apart and a band connects at any range of at least twice the spacing (validation requires it). The
  bands' edges set how many clusters each holds.
- **Per-band spacing** (sparser rim): fewer rim clusters, but the slice's voyages (0.9–1.3 S apart) must fit the rim.

## Decision

*(All numbers placeholders, in content: `galaxy.ron`, `bands.ron`, `cluster_types/`, `trails.ron`,
`signal_sources.ron`, `lore/`.)*

- **Spacing 0.9 S** (the slice's closest pair: home to the Shipyard approach), the whole disc sampled at once.
- **Bands** as rings out to their edges: rim 5.5 S, outer 4.2 S, middle 3.2 S, inner 2.2 S, core 1.1 S. That gives
  77–85 clusters (GDR-0026's "about 80"): rim about 37, outer 18, middle 13, inner 8, core 3. Most clusters lie in
  the rim, the band the game is played in for now.
- **Home**: the rim cluster nearest a random spot on the galaxy's edge with at least 3 rim clusters within the
  starting jump range (usually 5–8).
- **Gates**: 3 per band but the core, the cluster nearest the band's inner edge in each third of the disc.
- **Reachable**: every band's clusters, hop by hop within the starting range, from its entrances (home; the gates of
  the band outside it). Placement is re-rolled when a check fails (at most 16 attempts).
- **Trails** per band: one per gate (3–4 signals), 2 caches (2–3), 2 graveyards (1–2), one per lore fragment (1–2).
  The reward goes into a cluster of its own; earlier places are walked back from it 0.9–2.5 S at a time, towards the
  band's entrances where possible (else the nearest place left). No trail place at home, none in a gate but its own
  reward. A fresh signal's area is 2–3 S across with its place up to 60 % of the radius off its centre.
- **Cluster types of the rim**: wreck drift (weight 3), hunting reach (2), quiet field (2): 1.8–3 km radius (at most 4
  km, E0), sites 0.5–1.4 km apart, filled with the slice's encounters. Names: a start and an end syllable and the
  type's word ("Halvex Drift").
- **Signal sources**: a hauler's log (in a derelict field), a precursor log (in precursor wrecks), an old beacon
  (alone), a distress call (kept by a raider pair). **Lore**: three rim fragments, draft texts.
- **Stubs**: the outer, middle, inner and core bands are stand-ins (tag `stub`): one cluster type (uncharted deep,
  the rim's automatons) and a gate trail guarded by the Warden, until each band has its race and guardian.
- Trails end at the slice's encounters (a cache at precursor wrecks) until E8 gives caches and lore wrecks their own:
  a new encounter changes the rules' fingerprint, which marks saved lab runs stale (GDR-0092).

## Consequences

- The rim is a ring about 1.3 S deep and 30 S around: 12 or more jumps to circle it at the start, gates on all sides.
- Still open lead defaults, for their tracks: sensor range for behaviors (E3), how far sites stream (E3), the
  format-2 migration of slice careers and the jump-range nodes (E4), where the slice's voyages are pinned (E7).
