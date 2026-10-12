# GDR-0045: Shields absorb ram damage; vessels of the same team bump without ram damage

- Status: Accepted
- Date: 2026-10-12
- Decided by: Niklas (options and recommendations by the lead session)
- Related: [GDR-0019](0019-damage-model.md) (ramming), [GDR-0021](0021-shields.md) (shields), [GDR-0013](0013-vessels-and-piloting.md) (vessels, owners), `docs/sim.md` "Design questions" 12 and 13

## Context

Ramming v1 (Rapier merge, 12 Oct 2026) deals `(impulse − 20 kN·s) × 0.02` hp to the touching part of each side;
shields don't stop rams, and every body collides with every other, so raiders converging on one target rammed
each other.

## Options considered

Shields and rams:
- **Bubbles absorb ram damage** (chosen): a ram into a shielded area drains the bubble first, like a kinetic hit;
  the ships still collide physically at the hull.
- **Solid bubbles**: bubbles are colliders and ships bounce off them; extra colliders, keeps shielded ships
  apart. Not chosen.
- **Shields ignore rams** (the v1 behaviour): not chosen.

Same-team collisions:
- **Bump, no damage** (chosen): own vessels collide physically but deal no ram damage to each other, so formation
  flying and docking stay forgiving.
- **Bump with damage**: full physics and damage. Not chosen.
- **Pass through**: no collisions inside a team; cheapest, looks odd for physical ships. Not chosen.

## Decision

- Ram damage at a shielded contact point is absorbed by the bubbles covering it first (with the kinetic share
  of GDR-0021 unless the implementation finds a reason for its own share; placeholder), the rest hits the hull.
  The physical collision is unchanged.
- Contacts between vessels of the same team (owner) resolve physically but deal **no ram damage**.

## Consequences

- Needs owners/teams on vessels (the vessel model) and a team check in the impact handling; shield coverage of a
  contact point reuses the bubble test of shots.
- The ramming numbers themselves (threshold, damage per impulse) stay open and are retuned after the speed
  change (GDR-0041, GDR-0042).
- A separation term in the flight computer, so own vessels avoid each other, is still a good idea later.
