# GDR-0109: A ram prow and a working engine arm a ship: a rammer without guns keeps its site from clearing and blocks the recall

- Status: Accepted (provisional, lead), 18 Oct 2026, for Niklas to review
- Date: 2026-10-18
- Decided by: lead session (content batch of 18 Oct 2026, the seedling shoal; default for Niklas to review)
- Related: [GDR-0068](0068-voyage-sites-clear-when-enemies-are-disarmed.md), [GDR-0074](0074-test-range-waves-emergency-recall-cannon.md), [GDR-0099](0099-the-grown-and-the-outer-reach.md), [GDR-0102](0102-grown-mechanics-and-parts-defaults.md), [GDR-0103](0103-the-grown-in-play-defaults.md), `docs/results/encounters-balance-2026-10-18.md`

## Context

GDR-0068 clears a site once none of its ships is armed, and "armed" meant a gun and power on the controlling piece.
The seedling shoal brings the first ship without a gun: the `grown_seedling` (2×2) is a ram prow, a ganglion and
a thruster, and its charge is its weapon. Under the old rule a shoal cleared the moment it woke, its seedlings never
blocked a recall, and the voyage runner's pilot never fought them.

## Options considered

- **A ram prow and a working engine count as a weapon** (chosen): the rule names what can still hurt you. A rammer
  with its thrusters shot off can't charge, so it is disarmed, as a gunship whose guns are gone.
- **Give every seedling a gun**: no room in 2×2 (the prow takes two cells), and a shoal of gunships is another
  fight than a swarm of rammers.
- **A gunned rammer at every shoal to hold the site open**: the site would clear with the shoal still charging, and
  seedlings would still never block the recall.

## Decision

- A ship is armed when its controlling piece has a power source and either a gun or a ram prow (a part that deals
  more in a ram than it takes, `ram.dealt > 1`) and a working engine (not an RCS pod). The sim's `Ship::is_armed`
  and the content's `ship_armed` (validation: a goal encounter must roll an armed ship) use the same rule.
- Everything that reads "armed" follows: a site clears (GDR-0068), a recall is blocked (GDR-0068), the emergency
  recall (GDR-0074: a fleet with a prow and an engine is not disarmed), the voyage runner's targets.

## Consequences

- A Grown rammer or guardian whose thorn guns are gone stays armed until its prow or its thrusters go too. The outer
  reach's grove run hardly moved (Runner + cannon skiff 55 → 52 %, `docs/results/encounters-balance-2026-10-18.md`).
- One more cached flag per piece (`Piece::rams`, set on rebuild): `is_armed` stays one look-up.
