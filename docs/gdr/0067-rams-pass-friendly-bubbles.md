# GDR-0067: Rams pass friendly shield bubbles, like shots and plumes

- Status: Accepted
- Date: 2026-10-13
- Decided by: Niklas (interactively)
- Related: [GDR-0045](0045-shields-absorb-rams-no-friendly-ram-damage.md), [GDR-0043](0043-shields-as-energy-bubbles.md), [GDR-0048](0048-no-friendly-fire.md), `docs/sim.md` "Ramming between vessels"

## Context

GDR-0045 lets shield bubbles at the contact point absorb ram damage first. The sim review of 13 Oct 2026 found that
the ram path skipped only the rammer's own bubble, while shots and plumes skip every bubble friendly to their
source: a wingman's bubble covering the contact point soaked the ram damage meant for the enemy.

## Options considered

- **Rams pass friendly bubbles** (chosen): one rule for shots, plumes and rams; only bubbles not friendly to the
  damage's source absorb.
- Keep the ram rule (any bubble but the rammer's own absorbs).

## Decision

Damage from a ram, a shot or a plume is absorbed only by bubbles that are not friendly to its source. GDR-0045
otherwise stands.

## Consequences

- One shared helper in the sim decides which bubbles absorb; ram fights next to wingmen change slightly.
