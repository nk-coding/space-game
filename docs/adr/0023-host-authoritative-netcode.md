# ADR-0023: Co-op netcode is host-authoritative, with join codes and a tiny relay

- Status: Accepted
- Date: 2026-10-10
- Decided by: Niklas
- Related: [ADR-0005](0005-web-as-full-end-user-platform.md), [ADR-0006](0006-web-as-development-platform.md), [ADR-0010](0010-cross-platform-determinism.md), [ADR-0015](0015-ron-content-packs-and-mods.md), [ADR-0022](0022-one-simulation-space-per-cluster.md), GDR-0033

## Context

The brief says "no real server (P2P ok)". v1 had working peer-to-peer multiplayer. v2 plans co-op voyages in the
host's galaxy and an arena (GDR-0033). Nothing is built yet; `net` is a planned crate.

## Options considered

- **Dedicated servers**: ruled out by the brief.
- **Host-authoritative peer hosting** (chosen). Other netcode models were not recorded.

## Decision

- The host simulates. Clients receive snapshots, interpolate, and predict their own ship's movement.
- Determinism (ADR-0010) is not required for this, but it gives desync detection, exact replays and a later
  rollback path in the arena.
- Players connect with join codes through a tiny relay server for NAT traversal: the only server piece.
- Co-op requires a matching mod-list hash (ADR-0015).
- Transport: WebRTC data channels with a WebSocket fallback, the relay doubling as signalling server, was decided
  for the browser (ADR-0005). Since the web is a development platform (ADR-0006) that is no longer a
  requirement; the native transport is open.

## Consequences

- The host carries the simulation cost of every occupied cluster (ADR-0022).
- `net` comes after the single-player foundations (roadmap "Later").
