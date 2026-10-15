# GDR-0064: The lock and `face` lead a moving target's turn; AI flight fixes from the balance study

- Status: Accepted; superseded in part by [GDR-0089](0089-play-test-15-oct-speed-salvage-waves-station.md) (the fade weighted by cos² of the push angle and a speed governor replace `FADE_ALONG`; 2026-10-15)
- Date: 2026-10-13
- Decided by: Niklas (the feed-forward, interactively); lead session (the bug fixes, defaults for Niklas to review)
- Related: [GDR-0040](0040-keyboard-flight-and-smart-lock-aim.md), [GDR-0055](0055-slower-crisper-turning.md), [GDR-0058](0058-turning-v2-defaults.md), [GDR-0061](0061-fight-balance-after-turning-v2.md), `docs/results/balance-2026-10-13.md` "Sim issues"

## Context

The fight-balance study of 13 Oct 2026 found sim problems that decide fights more than the content numbers: the
target lock and the `face` maneuver hold the nose on where a target is, not where it is turning to, so the error
against a crossing target grows with ω²/α (worse since GDR-0055's slower turning); AI ships get stuck at their top
speed because maneuver goals ask for more than the top speed and the whole-speed fade gives no thrust to a push
with even a small component along the motion; engine-less AI vessels never turn to aim; split-off vessels measure
their hull share against the whole design and flee at once.

## Options considered

- Lock tracking: **feed the target's angular rate forward so the nose leads a crossing target** (chosen by Niklas),
  or keep the lag (crossing targets stay hard to hold).
- The other problems are bugs against settled records (GDR-0041, GDR-0055, GDR-0013): fixed as bugs (lead).

## Decision

- The lock (GDR-0040's smart aim) and `face` add the bearing's rate of change to their heading goal (feed-forward),
  within the turning limits of GDR-0055; the keys' turn rate is unchanged.
- Maneuver goals are capped at the ship's top speed, and the whole-speed fade is continuous in the push angle, so
  no push direction is cut off entirely (lead).
- Engine-less vessels with a working core turn to aim with what they have (gyros, cores, RCS) (lead).
- A split-off vessel's hull share is measured against its own piece, not the whole design (lead).

## Consequences

- Fight results change (the balance study measured the top-speed fix: the lone skirmisher drops from 28% to 2%
  wins without GDR-0061's content changes, 40% with them); re-measure with the harness after the fixes.
- Determinism hashes change once.
