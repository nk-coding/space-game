# GDR-0034: Behaviors pick the first matching rule about ten times a second; each gun fires at will along its facing; no match means hold

- Status: Accepted
- Date: 2026-10-11 (proposal); 2026-10-12 (Niklas: per-gun facing; accepted)
- Decided by: Niklas (fire at will per gun facing); behavior agent (first match, hold, cadence; proposal accepted 2026-10-12)
- Related: [GDR-0015](0015-layered-behavior.md), [GDR-0018](0018-weapon-families.md), [GDR-0023](0023-shot-leading.md), [GDR-0035](0035-rule-slots-per-core.md), [GDR-0036](0036-behavior-text-format.md), [GDR-0037](0037-behavior-vocabulary-as-content.md), [ADR-0026](../adr/0026-behavior-programs-text-in-content.md), `docs/behavior-format.md`, `docs/sim.md` "Behaviors"

## Context

GDR-0015 settles the layers (rules → maneuvers → flight computer) but not how a rule list runs: how often, what
happens to firing, and what a ship does when no rule fits. Behavior system v1 needed defaults. Guns fire along
their facing (GDR-0018), so broadside and rear guns exist.

## Options considered

- **Firing**: (a) fire at will along the **nose** only (the old raider brain; the first proposal): side and rear
  guns never fire under behaviors; (b) **fire at will per gun** (chosen by Niklas, 12 Oct): each gun fires when
  the target is inside its own cone along its facing; (c) separate fire actions or a fire flag per rule: more
  control, more rules spent on shooting, easy to get wrong; (d) actions decide (attack actions fire, move
  actions do not): Gladiabots style; hides firing inside action choice.
- **No rule matches**: (a) **hold: brake and stay** (chosen), still firing at will if the target crosses a gun;
  predictable and visible in the debug view. (b) Keep the last action: hides mistakes, a ship can chase a
  target its rules no longer want.
- **Cadence**: (a) every tick (30 Hz): the decision costs three times as much for no visible gain; (b) **every
  3 ticks (10 Hz), staggered by ship id** (chosen): 100 ms reaction, a fleet spreads its thinks over the ticks;
  the chosen maneuver, aiming and fire at will still run every tick, so flight stays smooth; (c) slower (2–5 Hz):
  visibly sluggish.

## Decision

- Each think: pick the target (target line), walk the rules top to bottom; the first rule whose conditions all
  hold decides the maneuver. An action that needs a target fails without one and the next rule gets its turn.
  Conditions about the target are false while there is none, with or without `not`.
- **Fire at will, per gun** (Niklas): whatever the action, every gun fires on its own when the target (for
  projectile guns, its lead point) is within the gun's cone along its facing (0.12 rad) and within 93.3% of
  that gun's reach, measured from its muzzle. Actions stay pure movement.
- No matching rule: hold (brake).
- Thinks every 3 ticks, staggered by ship id, and on the first tick after a behavior is assigned.
- A ship keeps its target unless another one is clearly better (cost × 0.8), so it does not flip between two
  nearly equal ones.

## Consequences

- Broadside and rear guns fire under behaviors whenever the target crosses their line (e.g. while orbiting).
  Maneuvers still point the nose at the target; a "show the broadside" maneuver would make side guns count
  more (not built).
- A "hold fire" option (stealth, escape) would be an action modifier or a setting; not built.
- Future non-movement actions (e.g. a power-tier change, GDR-0014) can sit next to the maneuver of a rule
  without changing first-match.
