# GDR-0125: Disruption: ion pulses fill a vessel's cores with static, and a full charge blinds its mind for a while

- Status: Accepted (provisional, lead), 20 Oct 2026, for Niklas to review
- Date: 2026-10-20
- Decided by: lead (disruption agent; Niklas was away, defaults where GDR-0018 leaves things open)
- Related: [GDR-0018](0018-weapon-families.md) (the families: disruption against logic cores and sensors, blinds
  enemy behaviors), [GDR-0021](0021-shields.md) (shields' per-family shares), [GDR-0013](0013-vessels-and-piloting.md),
  [GDR-0038](0038-logic-cores-control-parts.md), [GDR-0044](0044-control-cost-priority-idle-dormant-newest.md) (cores and
  vessels), [GDR-0095](0095-sensors-hidden-sites-and-scans.md) (sensors), [GDR-0115](0115-missiles-and-point-defence.md)
  (the last family added end to end), `docs/sim.md` "Disruption (GDR-0125)"

## Context

GDR-0018 named the fourth weapon family, disruption (ion, EMP): against logic cores and sensors, it blinds enemy
behaviors. `WeaponFamily::Disruption` existed in the content schema since the start, but no gun had it, and nothing said
what blinding means, how it builds up, how shields and armour take it, or how long it lasts.

## Options considered

- **What a hit does**: damage the cores directly (a sniper's weapon: one lucky hit ends a ship, and it is only damage),
  or a **charge that builds up** on the struck vessel and blinds it when full (chosen: a new state, readable, tunable,
  and it never kills on its own).
- **What blinding means**: the cores go dark (the ship stops flying: too close to a kill, and dark already means
  "unpowered"), or the **mind sees nothing**: no target, no enemies in its conditions, fire at will holds, sensors dark;
  it still flies the rules that need no target (chosen: exactly "blinds enemy behaviors").
- **Lock-out**: none (enough ion guns hold a ship blind for good), or an **immunity after each blinding** (chosen).
- **Bigger minds**: one fixed threshold, or **per alive core** on the controlling piece (chosen: "against logic cores";
  a ship with many cores takes longer).
- **Shields**: let static through (the anti-shield family is kinetic already, GDR-0018/GDR-0021), or **absorb it whole
  like energy** (chosen: static rides on the shot; a bubble takes the shot, so it takes the static).
- **Armour**: stops it like a laser (then the weapon only works on bare cores), or **conducts it** (chosen: any hit on
  the vessel's controlling piece counts; a hit on a core or a sensor counts twice).
- **Parts**: an ion gun and an EMP burst (an area pulse around the ship), or **one ion gun first** (chosen: minimal
  content; the EMP waits).

## Decision

- **Static** (`WeaponSpec::disrupt`, static per second at full power, hit-scan guns only): each hit's damage that gets
  past the shields carries `damage × disrupt / weapon damage` static into the struck ship, when the part is on its
  controlling piece (the vessel; salvage takes none); twice that on a logic or automaton core or a sensor part.
- A vessel **holds** 2 static per alive core on its controlling piece and **sheds** 0.4 a second. At its limit it is
  **blinded for 3 s** (`SimEvent::Blinded`), then **immune for 4 s** (static does not build up). A split-off vessel
  starts clear.
- **Blinded**: its sensor range is 0; its behavior and pilot assists pick no target (the `locked` selector too);
  `enemies_near` reads 0, `under_attack` false, `missiles_incoming` 0; rules that need a target fail with no target, so
  fire at will holds and it flies the first rule that needs none (an escort keeps its leader). The piloted focus keeps
  its stick, lock and trigger; only its assists go blind.
- **Shields and armour**: every shield emitter (the base emitter, the wide emitter, the prism ward) absorbs `Disruption`
  whole (1.0). Armour takes the shot's small damage as usual and passes the static on.
- **Content** (placeholders): the **ion cannon**, 1 × 1, 3 BP, 1.5 t, 1 HP, draws 3 (tier Low), heat 1 (limit 3);
  `Pulse(rate: 1.5)`, 0.3 damage/s (0.2 a pulse), 50 m, **1.2 static/s** (0.8 a pulse, 1.6 on a core). A raider
  (one core, 2 static) is blinded by three hull hits or two on its core; the starter (two cores) by five. Locked; the
  logic research's node `ion_cannon` (5 logic + 3 energy data, no requirement; in the outer band's play-test kit)
  unlocks it; recipe 2 copper, 3 crystal, 1 steel in the plain workshop (it fires pulses, like point defence); its
  salvage feeds the logic family. **No faction carries one** (no record assigns disruption to an enemy).
- **Looks**: ion pulses and their hits in the logic violet; a blinding flashes violet over the vessel, and while it
  lasts violet sparks crackle from its live cores and a broken ring of static flickers round its hull. Icons
  `ion-cannon.svg` / `ion-cannon.ship.svg` (a violet ion core in a weapons-cyan ring, coil bands on the barrel).
  Screenshots (1280 × 720): `docs/results/ion-cannon-drydock-2026-10-20.png` (two ion cannons in the starter's nose,
  the part card) and `docs/results/ion-blinded-raider-2026-10-20.png` (a raider blinded in a replayed fight).

## Consequences

- Balance (arena, the starter with ion cannons for its two nose slopes against the starter, 60 seeds, 100 m, 120 s;
  wins / losses / undecided): against 2 gunners 27 / 27 / 6 (lasers 21 / 33 / 6), hunter + raider 31 / 25 / 4 (23 / 35 / 2),
  lancer + raider 20 / 39 / 1 (17 / 42 / 1), 3 raiders 2 / 57 / 1 (5 / 54 / 1): a mild sidegrade, best where the enemy's
  shields are down. Blindings are rare in these fights (none to two a fight in the 18 looked at): the lasers often
  decide first, pulses miss a turning target, and bubbles take them. The balance check's seven fights are unchanged fight for fight (only the rules fingerprint moved).
- The state hash takes a ship's `Jam` only while it holds static or runs a blinding, so worlds without disruption keep
  their hashes; the flight and assists determinism scenarios fly an ion ship beside a raider (native = wasm).
- Open: an EMP burst (area, maybe a short dark spell for engines), enemies that carry disruption, whether the player's
  own flown ship should lose its lock when blinded, a `blinded` behavior condition, and every number above.
