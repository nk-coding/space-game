# GDR-0033: Multiplayer later: co-op voyages in the host's galaxy and arena PvP with your own ships

- Status: Accepted
- Date: 2026-10-10
- Decided by: Niklas
- Related: [GDR-0026](0026-procedural-galaxy-and-bands.md), [GDR-0032](0032-balancing-arena-evolution.md), [ADR-0022](../adr/0022-one-simulation-space-per-cluster.md), [ADR-0023](../adr/0023-host-authoritative-netcode.md)

## Context

v1 had working peer-to-peer multiplayer; the brief floated co-op raids for later. The concept notes' table lists
multiplayer as "Later".

## Options considered

- **Co-op raids**: parked until base co-op works.
- **Shared blueprints and behaviors as AI opponents** (bounty hunters, async PvP): parked; the balance arena already
  does this internally.

## Decision

- Co-op voyages in the host's galaxy, host-authoritative (ADR-0023). Every boss can be beaten solo with a good
  design; friends make it easier.
- Arena PvP with the ships you actually built.
- Both come later, after the single-player foundations.

## Consequences

- Players may split up in co-op, so the host simulates several clusters (ADR-0022).
- A blueprint plus its behavior is a complete opponent, which keeps the parked ideas cheap to add later.
