# GDR-0047: Ramming crushes inward; bumps below 100 kN·s are free

- Status: Accepted
  2026-10-17: a ram prow as the touching part scales the damage each side takes (2 × dealt, 0.5 × taken, [GDR-0102](0102-grown-mechanics-and-parts-defaults.md)); the rule above is unchanged.
  2026-10-17: a ram on an anchored asteroid rock follows this rule, scaled by the rock kind's `ram` (iron and slag 0.5, ice 0.35; [GDR-0104](0104-scenery-rocks-filler-landmarks.md)): against a body that gives nothing the impulse carries the ship's whole mass; the rock takes nothing.
- Date: 2026-10-12
- Decided by: Niklas (options and numbers by the lead session)
- Related: [GDR-0019](0019-damage-model.md) (ramming), [GDR-0045](0045-shields-absorb-rams-no-friendly-ram-damage.md) (shields, same team), [GDR-0041](0041-engine-rated-speed.md) and [GDR-0042](0042-agility-target.md) (the new speeds), `docs/sim.md` design question 11

## Context

Ramming v1 dealt `(impulse − 20 kN·s) × 0.02` hp to one touching part per side; damage beyond that part's hit
points was lost. With the agile ships (about 2× the speed) the impulses grew: the Halvex Runner at full speed into
a raider 378 kN·s (7.2 hp per side), head-on 720 kN·s (14 hp); raiders of one pack bumping on the way in
160–310 kN·s. Parts have 1–4 hp; a raider has about 12 hp in total, the Runner 72.

## Options considered

- **Crush inward, bumps free** (chosen): threshold 100 kN·s (closing speeds under about 6 m/s do nothing),
  0.02 hp per kN·s, and the damage carries inward past destroyed parts. The Runner at full speed into a raider:
  5.6 hp per side (2–3 raider blocks, a dent in the Runner's nose); head-on 12 hp (the raider mostly wrecked).
- **Crush inward, harsh**: keep 20 kN·s and 0.02 with the inward carry. Not chosen.
- **Keep as is** (one part per side, the rest lost): not chosen.

## Decision

- Ram damage per side = `max(0, impulse − 100 kN·s) × 0.02` hp (placeholders).
- It hits the touching part first; what exceeds that part's hit points carries on to the next part inward along
  the impact direction, and so on until it is used up (like a penetrating shot).
- Shields absorb it first and same-team contacts deal none (GDR-0045).

## Consequences

- A reinforced prow (high-hp armour at the nose) becomes a real ramming build (GDR-0019).
- The sim's ramming code changes together with the vessel model work (teams, shields).
