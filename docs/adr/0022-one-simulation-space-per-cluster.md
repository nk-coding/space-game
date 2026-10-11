# ADR-0022: One simulation space per cluster

- Status: Accepted
- Date: 2026-10-10
- Decided by: Niklas
- Related: [ADR-0010](0010-cross-platform-determinism.md), [ADR-0023](0023-host-authoritative-netcode.md), [ADR-0024](0024-saves-seed-and-deltas.md), [GDR-0026](../gdr/0026-procedural-galaxy-and-bands.md), [GDR-0028](../gdr/0028-clusters-and-voyages.md)

## Context

The galaxy is one seamless procedural galaxy of about 80 clusters (GDR-0026), each 10+ minutes across
(GDR-0028), simulated in `f32` with many physical ships.

## Options considered

Not recorded.

## Decision

- Each cluster is its own simulation space with small, precise local coordinates.
- Moving between clusters is a short warp sequence while the next one loads; the map, gates and lanes keep the
  galaxy feeling seamless.
- Only occupied clusters simulate. Empty ones freeze and settle their respawn timers when someone arrives.
- A cheap galaxy-wide event layer moves Swarm raids and roaming bosses between clusters.
- In co-op, players may split up, so the host simulates several spaces at once.

## Consequences

- One `Sim` world corresponds to one cluster; `f32` precision only has to cover a cluster.
- The co-op host's cost grows with the number of occupied clusters.
