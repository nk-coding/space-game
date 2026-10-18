# GDR-0018: All six weapon families; the capacitor bonus is for lasers only; guns fire along their facing

- Status: Accepted
  2026-10-18: the kinetic weapons' own support part (lead, content batch): the ammo feeder gives +20 % fire rate to adjacent parts tagged `cannon` (the reverse-engineered and long-barrel cannons; not the automatons' own), at most two per cannon, unlocked by the utility node `ammo_feeder`. The capacitor stays lasers only.
  2026-10-18: the ammo feeder +20 % → +15 % fire rate, the capacitor's step (lead, docs/results/sidegrades-2026-10-18.md): at +20 % it lifted the cannon starter's Warden clear from 28 to 41 %
- Date: concept phase (families; exact day not recorded, settled by 2026-10-11); 2026-10-11 (capacitor, facing)
- Decided by: Niklas
- Related: [GDR-0004](0004-grid-ship-building-no-wiring.md), [GDR-0016](0016-combat-pace-and-aiming.md), [GDR-0019](0019-damage-model.md), [GDR-0020](0020-damage-v1-defaults.md), [GDR-0021](0021-shields.md), `docs/content-model.md` "Adjacency bonuses"

## Context

Each family should lean on a different ship system, so a loadout shapes the whole build. The concept-board drydock
gave the capacitor's fire-rate bonus to every gun, while the part text said lasers. Rotated guns raised the
question of where they shoot.

## Options considered

- Fewer families: not recorded.
- Capacitor bonus for **every gun** (concept-board drydock) or **lasers only** (chosen, 11 Oct).

## Decision

- All families:
  - **energy** (laser, beam, pulse): strong against armor; leans on power and heat;
  - **kinetic** (autocannon, railgun, flak): strong against shields; ammo; recoil pushes the ship and off-centre
    guns twist it;
  - **missiles** (rocket swarms, torpedoes): strong against large, slow targets; countered by point defense and
    flares;
  - **disruption** (ion, EMP): against logic cores and sensors; blinds enemy behaviors;
  - **drones** (fighters, repair drones): flanking and swarming; need hangar blocks and rule slots;
  - **utility** (tractor beam, harpoon, mines): salvage, pulling ships off course.
- The capacitor bonus applies to lasers only; kinetic weapons get their own support part later (11 Oct).
- Guns fire along their rotated facing, so broadside and rear guns are possible (11 Oct).

## Consequences

- Content gives the capacitor bonus to parts tagged `laser`.
- Missiles, disruption and drones are not built yet (shield bubbles already stop missiles, GDR-0021).
