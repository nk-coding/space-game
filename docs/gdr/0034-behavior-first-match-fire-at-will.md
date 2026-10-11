# GDR-0034: Behaviors pick the first matching rule about ten times a second; guns fire at will; no match means hold

- Status: Proposed
- Date: 2026-10-11
- Decided by: behavior agent (proposal for Niklas to review)
- Related: [GDR-0015](0015-layered-behavior.md), [GDR-0035](0035-rule-slots-per-core.md), [GDR-0036](0036-behavior-text-format.md), [GDR-0037](0037-behavior-vocabulary-as-content.md), [GDR-0023](0023-shot-leading.md), [ADR-0026](../adr/0026-behavior-programs-text-in-content.md), `docs/behavior-format.md`, `docs/sim.md` "Behaviors"

## Context

GDR-0015 settles the layers (rules → maneuvers → flight computer) but not how a rule list runs: how often, what
happens to firing, and what a ship does when no rule fits. Behavior system v1 needed defaults.

## Options considered

- **Firing**: (a) **fire at will whatever the action** (chosen): guns fire whenever the target (or its lead
  point) is within a narrow cone of the nose and within reach, exactly like the old raider brain; actions stay
  pure movement, and a rule list never forgets to shoot. (b) Separate fire actions or a fire flag per rule: more
  control, more rules spent on shooting, easy to get wrong. (c) Actions decide (attack actions fire, move actions
  do not): Gladiabots style; hides firing inside action choice.
- **No rule matches**: (a) **hold: brake and stay** (chosen), still firing at will if the target crosses the
  nose; predictable and visible in the debug view. (b) Keep the last action: hides mistakes, a ship can chase a
  target its rules no longer want.
- **Cadence**: (a) every tick (30 Hz): the decision costs three times as much for no visible gain; (b) **every
  3 ticks (10 Hz), staggered by ship id** (chosen): 100 ms reaction, a fleet spreads its thinks over the ticks;
  the chosen maneuver, aiming and fire-at-will still run every tick, so flight stays smooth; (c) slower (2–5 Hz):
  visibly sluggish.

## Decision

- Each think: pick the target (target line), walk the rules top to bottom; the first rule whose conditions all
  hold decides the maneuver. An action that needs a target fails without one and the next rule gets its turn.
  Conditions about the target are false while there is none, with or without `not`.
- Guns fire at will (cone 0.12 rad, 93% of reach; placeholders).
- No matching rule: hold (brake).
- Thinks every 3 ticks, staggered by ship id, and on the first tick after a behavior is assigned.
- A ship keeps its target unless another one is clearly better (cost × 0.8), so it does not flip between two
  nearly equal ones.

## Consequences

- Broadside and rear guns (GDR-0018) never fire under behaviors yet: the fire-at-will check looks at the nose
  only. Needs a per-gun facing check (or fire groups) later.
- A "hold fire" option (stealth, escape) would be an action modifier or a condition-free setting; not built.
- Future non-movement actions (e.g. a power-tier change, GDR-0014) can sit next to the maneuver of a rule
  (a rule could then do "maneuver + side effect"), without changing this first-match rule.
