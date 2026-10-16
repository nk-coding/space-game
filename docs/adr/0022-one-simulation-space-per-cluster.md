# ADR-0022: One simulation space per cluster

- Status: Accepted. 2026-10-16: `f32` measured far from the origin (galaxy plan track E0,
  `docs/results/2026-10-16-bigger-clusters.md`): with the cluster centred on the origin it covers a radius of about
  4 km (at 3 km steered flight, fights and rams play as at the origin and native and wasm hash identically; only
  slow coasting shows the 0.24 mm grid); beyond that a cluster would need `f64` physics or local frames.
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
