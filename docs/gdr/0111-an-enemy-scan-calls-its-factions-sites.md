# GDR-0111: An enemy's scan calls its own faction's sleeping sites within the scanner's range; the first caller is the automaton picket, a rim site

- Status: Accepted (provisional, lead), 18 Oct 2026, for Niklas to review
- Date: 2026-10-18
- Decided by: lead session (content batch of 18 Oct 2026, the automaton picket; default for Niklas to review)
- Related: [GDR-0080](0080-exploration-searching-and-story.md) (scans wake enemies), [GDR-0095](0095-sensors-hidden-sites-and-scans.md) (scans and their wake radius), [GDR-0062](0062-first-playable-loop.md) (sites wake), [GDR-0068](0068-voyage-sites-clear-when-enemies-are-disarmed.md), `docs/results/picket-keeper-balance-2026-10-18.md`

## Context

The content batch's automaton picket is an unarmed scout that runs and calls the others: kill it before it gets
away, or the sites around you wake together. Until now a scan woke sleeping sites only when a player vessel made
it (GDR-0095: within the scanner's `wake`, 300 m); an enemy's sweep did nothing. Rim sites lie 500–1400 m apart,
so the player's 300 m would hardly ever reach another site.

## Options considered

- **The player's rule for enemies too** (every sleeping site within `wake`, any faction): the smallest change, but
  a 300 m sweep from a picket that runs away from you almost never reaches another site.
- **Its own faction's sleeping sites within the scanner's `range`** (chosen): the sweep is a call, not just noise;
  1200 m reaches the next one or two sites. Other factions' sites sleep on (the Grown do not answer automatons).
- **The same, and sites not yet spawned spawn to answer**: reaches farther, but the far sites cannot see the fleet
  (cores see 800 m), wait out of sight, and the runner cleared even more often than with the chosen rule.

## Decision

- An enemy vessel's sweep (`SimEvent::Scanned` by a ship of the voyage's enemy owner) wakes every **sleeping**
  site within the scanner's `range` that has a ship of the scanning ship's faction. It reveals nothing to the
  player and gives no bearings; a site not spawned yet is not called.
- The `automaton_picket` (3×3, automatons, lead default): a scanner in the nose, an RCS pod on each flank, a
  thruster and one core, no gun. Its `picket` behavior flees from the nearest enemy and scans whenever that enemy
  is more than 300 m off, so staying on its tail keeps it from calling.
- Its site, `picket_post` (the picket with a raider pair, wake radius 250 m), 0–1 per wreck drift and per
  hunting reach.

## Consequences

- Any enemy design with a scanner and a `scan` rule calls its faction this way; no other design has one yet.
- Unarmed, the picket never keeps its site from clearing or the fleet from the recall (GDR-0068).
- The voyage runner clears a test voyage with a picket post *more* often when the picket calls than when it never
  scans (Runner alone 42 against 38 %, with a skiff 62 against 48 %; with hunters 54 / 69 against 44 / 62 %): its
  pilot fights called raiders that come to it better than it flies into a sleeping site. What the call costs a player (the choice of fights, a
  quiet salvage run) is not measured. Open: whether called ships should gather on the caller first.
