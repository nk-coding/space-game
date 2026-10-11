# GDR-0016: Combat is real-time, fast-leaning and mid-range; fixed guns are aimed by turning with a lock

- Status: Accepted
- Date: 2026-10-10
- Decided by: Niklas
- Related: [GDR-0009](0009-stick-and-lock-controls.md), [GDR-0017](0017-combat-support-repair-capture-sensors.md), [GDR-0018](0018-weapon-families.md), [GDR-0023](0023-shot-leading.md), `docs/concept-notes.md` "Combat"

## Context

Fights should be real-time, physical and readable, on a phone as well as on a PC, with ships that range from small
drones to capital ships.

## Options considered

Not recorded, apart from the rejected control schemes in GDR-0009 (twin sticks made aiming fixed guns hard).

## Decision

- Pace scales with ship size under the same physics: small ships are fast and twitchy, big ones slow and heavy.
  Overall the feel leans fast and arcade.
- Fights happen at mid range, one to two screens apart, with smooth zoom; weapon ranges differ.
- Always real-time: no pause, no slow motion.
- Fixed guns are aimed by turning the ship, helped by the target lock. Turrets aim through behaviors.
- The player sets target priorities (engines first, weapons first, nearest threat).

## Consequences

- Turning speed matters for aiming (GDR-0010), and the lock aims at the lead point for projectiles (GDR-0023).
- Open: ship speed and fight length. Current placeholders give ~0.7 m/s², ~10 m/s and about 15 s for a Halvex
  Runner to kill three raiders after contact; Niklas tunes this with live sliders in the browser.
