# GDR-0113: A call brings reinforcements: a picket post's first call warps a group in from the cluster's edge

- Status: Accepted (Niklas, 19 Oct 2026: reinforcements warp in; the details are lead defaults for him to review)
- Date: 2026-10-19
- Decided by: Niklas (the call hurts through reinforcements); lead session (the data, the timing, the numbers)
- Related: [GDR-0111](0111-an-enemy-scan-calls-its-factions-sites.md) (an enemy's scan calls), [GDR-0087](0087-voyage-transitions.md) (the warp look), [GDR-0068](0068-voyage-sites-clear-when-enemies-are-disarmed.md), `docs/results/picket-keeper-balance-2026-10-18.md`

## Context

A picket's call (GDR-0111) only woke the sites around it, and the voyage runner cleared picket voyages more often
when it called: called sites lie within the fleet's reach anyway, so a call only brought their fights forward.
Bigger posts, calling at once, spawning far sites and keeping the goal out of the call never made it a penalty.
For a call to cost, it has to bring ships the fleet would not otherwise meet.

## Options considered

- **Bigger called sites** (more ships at the sites a call wakes): tried, no penalty; and every other voyage with
  those sites changes too.
- **A group that warps in** (chosen, Niklas): new ships from outside the cluster, only when the picket calls.
- **The call wakes the whole cluster**: far sites spawn and come; tried, easier still for the runner.

## Decision

- An encounter's optional `calls` names the group: another encounter's id (ships rolled like any site's, wrecks and
  salvage as usual), one with ships that calls none itself; the calling encounter needs a ship with a scanner.
  The picket post calls `picket_call`: two or three automaton raiders.
- The plan rolls the group with the site, on a generator of its own, as a site on the cluster's edge at a rolled
  angle. The first enemy scan from the calling site calls it (once per site, so once per picket): 30 s later
  (`sim::CALL_SECONDS`, placeholder) its ships appear there awake and fly to where the picket saw the player
  (its target), as GDR-0111's called ships do, until they have the fleet in sight.
- The client shows the call as a banner (`PICKET CALLED · REINFORCEMENTS INBOUND`) and the group's arrival with the
  fleet's arrival warp in its own glow. Until then the group is not a site: not counted, not flown to.
- Tuned so a call costs 5–15 points of the runner's clear % against the picket that never calls (240 seeds):
  the Runner 23 against 36 % (raider sites) and 38 against 48 % (hunter sites), with a skiff 42 against 51 % and
  52 against 65 %.

## Consequences

- Plans without a calling site roll and hash as before.
- Any encounter can call a group; a future caller (a Grown scout, a station's alarm) needs only content.
