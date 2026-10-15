# GDR-0074: Test-range waves follow the flown ship; recall also works when the fleet is disarmed; the cannon becomes an upgrade

- Status: Accepted; the wave rule refined by [GDR-0089](0089-play-test-15-oct-speed-salvage-waves-station.md) (the budget goes to the biggest enemies first; 2026-10-15)
- Date: 2026-10-13
- Decided by: Niklas (interactively, from the slice tuning report)
- Related: [GDR-0062](0062-first-playable-loop.md), [GDR-0064](0064-lock-feed-forward-and-ai-flight-fixes.md), [GDR-0068](0068-voyage-sites-clear-when-enemies-are-disarmed.md), [GDR-0073](0073-slice-tuning-v1.md), `docs/results/slice-balance-2026-10-13.md`

## Context

The slice tuning pass (GDR-0073) found three problems: the test range's wave of three raiders became too easy after
the lock feed-forward (a passive Runner wins 98 %), while a wave of four crushes the starter; a disarmed fleet can't
escape an armed raider of the same speed, so 8–30 % of runner runs ended out of time; the first research node, the
reverse-engineered automaton cannon, was a downgrade for the starter (12 % clears of voyage 1 against 58 % with
pulse lasers).

## Options considered

- Test range: **waves follow the flown ship** (chosen), a wave of four, or keep three.
- Escape: **recall also works when the fleet has no working gun (an emergency warp, same 3 s)** (chosen), a longer
  recall charge under fire, or keep the rule.
- Cannon: **buff it so the first unlock is a real upgrade** (chosen), swap the first node, or leave it.

## Decision

- The test range sizes each wave from the flown design (placeholder: by build points against a raider's), so it
  stays a fair fight for any ship.
- Recall works when no armed awake enemy is near (GDR-0068) or when none of the player's vessels has a working gun.
- The automaton cannon's numbers rise until a starter fitted with it clears voyage 1 at least as often as with its
  pulse lasers (measured with the voyage runner).

## Consequences

- One sim rule change (the recall check) and content numbers; determinism stays checked.
