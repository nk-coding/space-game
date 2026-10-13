# Plan: co-op multiplayer and the arena

Written 13 Oct 2026 by a planning agent for the lead session. This is a plan, not a decision: the records it
needs are proposed in the text (no ADR/GDR files yet), and the questions for Niklas are in
[section 8](#8-questions-for-niklas). Numbers marked *placeholder* are placeholders for feel.

## 1. Summary

- **Co-op**: the host stays the authority, but it sends **inputs, not world snapshots**. It puts every player's
  input for a tick in order and sends that list to the guests. Every peer runs the same deterministic `sim::Sim`,
  and a state hash once a second catches desyncs. That is a few KB/s per player, so a simple relay can forward
  all of it, and the client's rendering code stays as it is. This changes the snapshot part of ADR-0023, so
  Niklas has to agree first (Q1).
- **Arena**: an engine-free `arena` crate builds a fight from a spec, runs it headless to the end and scores it.
  A native runner spreads fights over every core. The first slice is a balance tool for the developers. It
  replaces the throwaway harness behind `docs/sim.md` "Sandbox fights at the new speeds". Ratings, evolution
  and player-facing modes come after it.
- **Order**: arena track A1 comes first, since it needs no sim changes. The sim changes for sessions (S1) and a
  no-network input-delay prototype (P1) run next to it. Then `net`, relay and the co-op sandbox.

## 2. Already decided vs open

Decided (do not change silently):

| What | Record |
|---|---|
| Co-op voyages in the host's galaxy. Every boss can be beaten solo. Arena PvP with the ships you built. Both come after the single-player foundations | [GDR-0033](../gdr/0033-multiplayer-coop-and-arena.md) |
| Host-authoritative peer hosting, with no dedicated servers. **Clients get snapshots, interpolate and predict their own ship.** Determinism gives desync detection, replays and a later rollback path in the arena. Join codes and a tiny relay are the only server piece. Co-op needs a matching mod-list hash | [ADR-0023](../adr/0023-host-authoritative-netcode.md), [ADR-0015](../adr/0015-ron-content-packs-and-mods.md) |
| The web build is a development platform. WebRTC and other browser transports are no longer required. The native transport is listed as open | [ADR-0006](../adr/0006-web-as-development-platform.md), `docs/adr/README.md` "Open" |
| Bit-exact determinism on every platform. A state hash every tick in debug builds. Native vs wasm is checked by `scripts/determinism-check.sh` | [ADR-0010](../adr/0010-cross-platform-determinism.md), `docs/sim.md` "Determinism" |
| 30 Hz fixed step, 60 fps interpolation. Input is applied per tick | [ADR-0011](../adr/0011-30hz-sim-60fps-interpolation.md) |
| One sim space per cluster. In co-op, players may split up and the host simulates several spaces | [ADR-0022](../adr/0022-one-simulation-space-per-cluster.md) |
| Saves are seed plus deltas. RNG stream ids are part of the save/replay format | [ADR-0024](../adr/0024-saves-seed-and-deltas.md) |
| Rhai is sandboxed and bit-exact (f32, libm, operation budget) | [ADR-0027](../adr/0027-rhai-sandbox-and-determinism.md) |
| Player storage. Designs and behaviors travel as `SGD2:` share codes | [ADR-0031](../adr/0031-player-storage-and-design-sharing.md), [GDR-0059](../gdr/0059-behavior-editor-wave-a-defaults.md) |
| Arena: headless, every core, fixed seeds, any fight replays in the client. Hand-made archetypes plus evolution within a build-point budget. Genome = part list (optionally mirror-locked) + behavior. Fitness from fights against a rotating pool. MAP-Elites, Elo or TrueSkill, reports. An outer loop tunes numbers and a human reviews it. Exploit hunts become regression scenarios. A short run on every data change | [GDR-0032](../gdr/0032-balancing-arena-evolution.md) |
| Vessels, focus and mind capacity are per owner. The focus never goes dormant | [GDR-0013](../gdr/0013-vessels-and-piloting.md), [GDR-0044](../gdr/0044-control-cost-priority-idle-dormant-newest.md) |
| No ram, exhaust or shot damage between vessels of one owner | [GDR-0045](../gdr/0045-shields-absorb-rams-no-friendly-ram-damage.md), [GDR-0046](../gdr/0046-exhaust-hits-own-parts-and-enemies.md), [GDR-0048](../gdr/0048-no-friendly-fire.md) |
| Behaviors think every 3 ticks. Rule slots per core | [GDR-0034](../gdr/0034-behavior-first-match-fire-at-will.md), [GDR-0035](../gdr/0035-rule-slots-per-core.md) |
| Always real time, no pause | [GDR-0016](../gdr/0016-combat-pace-and-aiming.md) |
| Test range at home: simulated fights against archived enemies and your own past designs | [GDR-0002](../gdr/0002-awakened-ship-mind-light-story.md) |

The concept board's netcode text (WebRTC with a WebSocket fallback; `concept-board.html` "Netcode") came from
ADR-0005, which ADR-0006 superseded. The board's balancing section (`11-balancing.png`) shows the five-step loop
(generate, fight headless, rate, report, adjust data). It also shows an example report with pick rate, win rate
when used and flags (DOMINANT, UNDERUSED, NEVER PICKED).

Open (this plan proposes answers):

- Snapshots or inputs between host and guests (ADR-0023 says snapshots), and how guests deal with latency.
- The native transport, the relay, join codes, and who runs the relay.
- Joining mid-voyage. What a guest flies and sees. Whether teammates can hurt each other.
- Checking that builds and content match between peers (beyond the mod-list hash).
- How an arena fight ends and is scored. Where fight sets live. First evolution target. How results reach
  balance decisions.
- The form of arena PvP: live, or asynchronous with shared designs. The asynchronous form is parked in GDR-0033.

## 3. Gaps: options, trade-offs, recommendation

### 3.1 State sync vs lockstep

What the code offers today: `Sim::step(&TickInputs)` is the one seam (`crates/sim/src/world.rs`). The per-tick
input is `TickInputs { controls: Vec<(ShipId, ShipControls)>, commands: Vec<(ShipId, ShipCommand)>, damage:
Vec<DamageCommand> }`. A player's input is `ShipCommand::Pilot(PilotInput)`. `PilotInput` (`flight.rs`) holds
`stick: Vec2`, `lock: Option<Target>`, `drift`, `fire`, `aim: AimMode`, `axes: Option<PilotAxes>` (three f32)
and `aim_offset: f32`. All of these are plain `Copy` data with no pointers, about 40 bytes when encoded, so they
**can be serialised and sent**. `sim` has no serde today; `data` has serde 1.0 and RON. `ShipId`/`PieceId` are
monotonic and never reused (`ids.rs`), so an id means the same ship on every peer. `Sim` is `Clone`, and a clone
continues identically (`tests/determinism.rs`).

| Option | How | For | Against |
|---|---|---|---|
| A. Snapshots (ADR-0023 as written) | The host sends poses, part states, projectiles and events. Guests render a copy of the world | Guests need little CPU and cannot desync | Estimated 20–80 KB/s per guest in a 20-ship fight (to measure). The client needs a second, render-only model of `Ship`/`Piece`, because today it reads a real `Sim` everywhere (views, HUD, fleet bar, events). Breakage topology has to be sent too. Host cost grows with the number of guests |
| B. Classic peer lockstep | Every peer waits for every peer's input for tick N | Tiny traffic | The slowest peer stalls everyone. There is no authority, which goes against ADR-0023 |
| **C. Host-ordered input stream** | The host never waits. It collects guest inputs, applies each at the next host tick and broadcasts the full `TickInputs` of every tick. Guests run the same `Sim` a few ticks behind. Hashes detect desyncs, and the host's state is the truth | Tiny traffic (§7). The client code stays as it is: guests run the real sim. Replays, desync reports and arena replays share one format. Host cost does not depend on the number of guests. This is the model Factorio uses (FFF #76, #83, #188) | Guests need the CPU to run the whole fight (the same as single player). A guest sees its own input one round trip later (§3.3). Joining needs a catch-up (§3.6). A determinism bug becomes a visible desync |
| D. Rollback (GGRS) | Each peer predicts the others' inputs and re-simulates on a miss | No felt delay | Peer-to-peer, not host-authoritative. Every peer re-simulates the whole world several ticks per frame. The big-fight step is 0.43 ms median / 1.6 ms p95 natively (`sim_perf` combat), more on phones. A fit only for small duels |

**Recommendation: C.** The host is still the authority on what happens and when, so the spirit of ADR-0023
holds. Only "clients receive snapshots" changes, to "clients receive the ordered inputs and check hashes". Full
snapshots are kept for later joins and resyncs if they are ever needed (§3.6). Proposed record: *ADR "Co-op sends
the host-ordered input stream, not snapshots"* (refines ADR-0023 and supersedes its snapshot part). This is Q1.

### 3.2 What the guest sees and controls

- **Sees**: everything in the cluster, since the guest runs the full sim. Interest management is not needed
  for bandwidth. Sensor limits (GDR-0017) can filter what the HUD shows.
- **Controls**, options:
  - **Own ships** (recommended): the guest's own designs and behaviors from their storage, as their own `Owner`,
    with their own focus, fleet bar and mind capacity. The sim already keeps focus and mind capacity per owner
    (`Sim::focus(owner)`, `SimConfig::mind_capacity[owner]`), and GDR-0033 says "the ships you actually built".
  - Ships lent by the host: no design transfer, but the guest's builds don't count.
  - Escort mode: the guest pilots one of the host's vessels.
- The host checks that every guest command names a ship of that guest's owner and drops the rest (`Focus`,
  `Wake`, `Power`, `Pilot`, `Behavior`, `Leader`).
- **Teammates** (Q6): today `sim::friendly(a, b)` is "same owner" (`vessel.rs`), and behaviors target "active
  vessels of other owners". Two co-op players would shoot and ram each other, and their escorts would target each
  other. Recommendation: a team table in `SimConfig`, with teammates treated like own vessels (no shots, rams or
  exhaust damage, never a behavior target). Proposed record: *GDR "Co-op teammates are friendly like own
  vessels"* (extends GDR-0045, 0046 and 0048).

### 3.3 Latency handling

With C, the host's own input applies at once. A guest's input reaches the host after half a round trip, applies
at the next host tick, and comes back confirmed after the other half. The guest also keeps a small jitter buffer.

| Option | Felt delay for a guest | Cost |
|---|---|---|
| **Input delay plus cosmetic latency hiding** | One round trip + buffer + up to one tick: about 60–150 ms on a relay in the same region (estimate). The stick indicator, aim ring, lock ring and camera react at once (Factorio's "hide the latency", FFF #83) | None. Ships are heavy: a 90° turn takes about 1.5 s (GDR-0058) and full speed takes about 2.5 s, so 100 ms is small next to that. Judge it with P1 before any network exists |
| Rollback prediction for the guest | None for its own ship | Each frame: clone the confirmed `Sim` and step it K ticks with the guest's unconfirmed inputs. K × step cost, for example 4 × 1.6 ms at the big-fight p95 natively, more on a phone. Events from the predicted world (shots, hits) must not be shown twice |
| Equal delay for everyone, host included | The same for all | Fair, which is needed for live PvP. The host gives up its zero delay in co-op |

**Recommendation**: input delay plus latency hiding first, with an adaptive buffer. Add rollback for the guest's
own view only if play-tests call it sluggish and the measured cost fits. Equal delay is a session option for
live arena duels. This is Q2.

### 3.4 Join codes, the relay and the transport

With C, traffic is a few KB/s per player (§7). So **relaying all traffic is cheap**, and NAT hole punching
becomes optional (it would only lower latency).

| Option | For | Against |
|---|---|---|
| **Own tiny relay**: peers connect out to it (QUIC via `quinn` natively, WebSocket from the browser dev build). It holds rooms by join code and forwards frames in a star (host ↔ each guest) | Small and fully ours. No NAT problems, since everyone dials out. The browser build can join through WebSocket, so co-op works in two browser tabs on Niklas's host, which matters because the web build is how he develops | Every packet goes through the relay (adds a detour of about 10–30 ms if it is in the same region). Someone has to run a server with a public address. `quinn` needs tokio and rustls with `ring` (C and assembly) |
| `iroh` 1.x: QUIC dialled by public key, hole punching (about 9 in 10 connections go direct, per its FAQ), relay fallback, browsers through the relay | Direct paths, end-to-end encryption, a stable 1.x API since mid-2026 | Heavy dependencies (tokio, rustls, DNS lookup, its own QUIC fork `noq`). Its default discovery and relays are n0's servers ("free for development and testing", rate-limited), so for real use we self-host `iroh-relay`. A short code → key lookup is still needed |
| WebRTC with `matchbox` (data channels, a signalling server) | Browser and native, unreliable channels | A heavy native WebRTC stack. Hard NATs need a TURN server. `bevy_matchbox` 0.14 targets Bevy 0.18. The browser is no longer an end-user platform |
| `renet`/`renet2` (UDP, netcode.io-style) | Simple client/server UDP | No NAT traversal, so the relay would still have to forward UDP |

Not a fit for the transport/state layer: ECS replication crates (`bevy_replicon` 0.45 on Bevy 0.20,
`lightyear` 0.30 on Bevy 0.19). They replicate Bevy components, but our state lives in `sim::Sim`, outside the
ECS, and they would tie `net` to Bevy's release cycle.

**Join codes**: 6 characters from an alphabet without look-alikes (no 0/O/1/I/L), shown as `K7Q-M2X`
(*placeholder*). A code lives as long as the host's room, and the relay limits join attempts. No accounts and no
stored data on the relay: only room codes and opaque frames.

**Recommendation**: our own relay with QUIC (native) and WebSocket (browser dev build), forwarding everything,
behind a small `Transport` trait so `iroh` can replace it if direct paths are wanted later. For development the
relay runs in the VM (`10.77.9.2`). For real use it needs a small public server (Q3). Proposed record: *ADR
"Co-op transport: own relay, QUIC natively, WebSocket for the dev build"* (answers the open ADR question).

### 3.5 Versioning of content and builds between peers

C needs every peer to run *identical* code on *identical* data. Matching packs alone is not enough: two builds
of the game can produce different hashes from the same content.

| Check | How | When it fails |
|---|---|---|
| Protocol version | `net::PROTOCOL: u16`, bumped on any message change | "Different game version" |
| Sim fingerprint | At start, each peer runs a short fixed scenario (about 60 ticks of `scenario::Core`-like content on the session's content) and hashes the result: a few ms. It catches any change to sim behavior, and also a platform that is not deterministic | "Different game build" |
| Content id | `data` hashes every pack's files (sorted path + bytes) and lists `(pack id, version, hash)`. The embedded base pack's hash is computed at build time | Names the missing or different packs |
| Player designs and behaviors | Sent at join (the `SGD2` payload: blueprint RON + custom behavior texts), validated by the host (`data::validate_ship`), and behavior ids namespaced per player (`p2:<id>`). `BehaviorLibrary` hands out ids in the order behaviors are added, and `id()` returns the newest one under a name, so the order and the names must be the same on every peer | The host refuses the design, with the reason |

**Recommendation**: exact match in v1. Later, the host could send missing mod packs to the guest (RON and SVG
are small; Rhai is sandboxed). Never run with a mismatch. The live tuning panel (`tuning.rs`: `Sim::set_config`
and content rewrites) is host-only in a session, and changes it makes travel as inputs. The per-owner content
override `SimWorld::others` is off in co-op.

### 3.6 Joining after the start

| Option | For | Against |
|---|---|---|
| **Catch-up replay**: the joiner gets the session header and the input log since the space started, and steps through it fast | No world serialisation needed. The same code runs desync recovery and replays. A 30-minute voyage is 54 000 ticks, about 0.6 s natively for a 20-ship fight (0.011 ms per tick), roughly 1.5× that in wasm, phones unknown | Time grows with the age of the space. Big fights take longer (0.43 ms per tick: about 23 s for 30 minutes) |
| Lobby only | Simplest | Nobody joins a running voyage |
| Full snapshot | Instant at any age | The whole `Sim` must be serialised, including Rapier's internal state (contacts, warm starts, islands). Rapier can serialise its world (`serde-serialize`) but does not promise that a restored world continues bit-exactly. Prototype first |

**Recommendation**: lobby plus catch-up replay. A default voyage is one cluster of 15–30 min (GDR-0028), which
fits. Snapshots only when the galaxy needs them (for example a guest warping into a long-running cluster). This
is Q4.

### 3.7 How the arena scores a fight

End conditions (*placeholders*): a side is out when it has had no running vessel (`Sim::vessels(owner)` with
`VesselState::Running`) for 5 s, so a short brown-out is not a loss. The time limit is 90 s (2700 ticks).

| Option | For | Against |
|---|---|---|
| **Win/loss/draw first, then build points**: win 1, loss 0. At the time limit, 0.5 + 0.5 × (share of the enemy's build points destroyed − share of own lost). Averaged over seeds | Clear wins count most. Timeouts still tell evolution which side was ahead. Build points already exist (`data` part `build_points`, `Content::ship_totals`, `DesignReport::build_points`) | Two parameters to explain |
| Win/loss/draw only | Simplest | Kiting or fleeing builds give many draws that say nothing |
| Damage ratio only | Smooth for evolution | Rewards trading damage over winning |

Ratings: a Bradley–Terry/Elo fit over the score matrix for 1v1 sets, and TrueSkill later for team fights
(GDR-0032 allows either). **Recommendation**: the first row. Proposed record: *GDR "Arena fights end at no
running vessel or the time limit, scored win-first"*. This is Q7.

### 3.8 Evolution method

The method is decided (GDR-0032: mutation and crossover on part list + behavior, fitness from fights against a
rotating pool, MAP-Elites). What is open is the shape and the order:

- **Operators come from the editing models**, so every mutant is legal. Designs use `data::edit::Drydock`
  (`place`, `paint`, `erase`, `move_part`, mirror on by default). Behaviors use `data::behavior_edit::BehaviorEditor`
  (`add_rule`, `delete_rule`, parameters, the target line). Crossover splices rule lists and hull halves, later.
- **Cheap pre-filter**: `sim::design_report` (24 µs for the Runner) rejects designs with no core, no power or an
  overheat warning before any fight.
- **MAP-Elites**: one elite per niche. Design niches use report stats (*placeholder*: top speed × weapon reach,
  8 × 8). Behavior niches use fight telemetry (mean engagement distance × time spent closing in). The loop: pick
  an elite, mutate it, fight it against the pool (hall of fame top 8 + hand-made archetypes such as `raider_basic`
  and `skirmisher`, 4 seeds each), keep it if it beats the niche's elite.
- **Order**: behaviors on fixed hulls first. The genome is small, and it answers today's open question:
  `raider_basic`'s stand-off sits on a knife edge, and `skirmisher` wins 3–5 of 12 waves. Designs with fixed
  behaviors come next, then both together, then exploit hunts with no budget.

### 3.9 How the arena feeds balance decisions

| Step | Output | Who acts |
|---|---|---|
| Fight sets and matrices (A1) | Markdown + CSV tables in `docs/results/arena/` (win rate, time to kill, hp lost, shots, rams, burns), each row with a fight code to replay | Lead / Niklas read them, edit content RON |
| Balance check (A3) | `scripts/arena-check.sh`: a fixed set (*placeholder*: about 50 fights, under 10 s) compared with a stored baseline. Large shifts are flagged | Run after every content change (local, ADR-0008) |
| Part reports (A4) | Pick rate among top designs, win rate when used, flags as on the concept board | Input for content changes |
| Outer loop (A7) | Proposed RON diffs with the before/after reports | A human reviews. Never applied automatically (GDR-0032) |
| Exploit hunts (A7) | Each confirmed exploit becomes a fight in the check set | Lead decides the fix |

## 4. Design proposal

### 4.1 Co-op, as the player sees it

1. **Host**: CO-OP → HOST (later in the home ring menu; for development `?host=1`). The game opens a room at the
   relay and shows the join code with a copy button. Up to 4 players (*placeholder*).
2. **Guest**: CO-OP → JOIN, types or pastes the code (`?join=K7Q-M2X`). The version and content check runs (§3.5).
   On a mismatch the guest gets a plain reason ("the host runs a different game build", "missing pack X").
3. **Loadout**: the guest picks a design from their BLUEPRINTS (own storage) and its behavior. The host checks it
   (frames and later unlocks), and the guest's mind capacity comes with it.
4. **Lobby and launch**: the co-op sandbox first (raider waves, as in single player). When the galaxy exists,
   players gather at the host's shipyard and launch the voyage together.
5. **In flight**: each player pilots their own focus, with their own lock, fleet bar and HUD. Teammates' ships
   have a teammate tint and a name tag (pick the tint with a visual comparison). Off-screen markers show
   teammates. Behaviors never target teammates (§3.2).
6. **Lost ship**: a guest's ship re-forms near the host's focus after 3 s, as in the sandbox (*placeholder*).
   With the galaxy: GDR-0029's re-form at home, adapted to the voyage.
7. **Lag and desync**: `LAG` banner after 1 s without confirmed ticks, dropped after 10 s (*placeholders*).
   `DESYNC · REJOINING` rejoins once by catch-up. A second desync leaves the session and saves a report.
8. **Leaving**: a leaving guest's vessels warp out (removed). The host leaving ends the session (`HOST LEFT`),
   with no host migration in v1. (Guests hold the full state, so host migration is possible later.)
9. **Rewards** (when salvage and research exist): what a guest brings home goes to the guest's own save.

### 4.2 Arena modes

| Mode | For whom | What |
|---|---|---|
| Developer arena (CLI) | Lead, Niklas | `arena run <set>`, `matrix`, `check`, `evolve`, `hunt`: reports and fight codes |
| Fight replay | Both | `?scene=fight&fight=<code>` in the web build: the same world, stepped at 1×/2×/4×, with the final hash shown so the replay is checked against the report |
| SIM BATTLE in the test range | Players | Pick a design + behavior and an opponent set (archived enemies, own past designs, GDR-0002). The client runs N fights in time slices (*placeholder*: 12 seeds), shows the win rate in seconds, and any fight can be watched. Builds on TEST IN RANGE (GDR-0057) |
| Async arena (PvP-lite) | Players | Opponents from shared designs (share codes). AI vs AI, ladder per build-point band. Un-parks GDR-0033's parked idea (Q8) |
| Live duel | Players | 1v1 or 2v2 piloted over the co-op layer, build-point cap, equal input delay. Later |

### 4.3 Placeholder numbers

| What | Value |
|---|---|
| Players per session | up to 4 |
| Owners | host `Owner(0)`, AI `Owner(1)` (as today), guests `Owner(2..=4)`, other AI sides from `Owner(5)`, within `MAX_OWNERS = 8` |
| Guest jitter buffer | 1–4 ticks, adaptive |
| Hash check | every 30 ticks in release; every tick in debug builds (the hash is computed anyway) |
| Join code | 6 characters, look-alikes removed |
| Arena time limit / out after | 90 s / 5 s without a running vessel |
| Arena start distance | 120 m |
| Seeds per pairing | 8 (matrix), 4 (evolution) |
| Evolution | population 64, archive 8 × 8, pool 8 + archetypes, 100 generations |

## 5. Technical plan

### 5.1 Crate layout

```
crates/net/            engine-free, no Bevy; builds for wasm32 (transports behind features)
  src/proto.rs         messages (serde + postcard), PROTOCOL
  src/wire.rs          WireTick <-> sim::TickInputs (design and behavior tables)
  src/session.rs       HostSession, GuestSession: sans-IO state machines (tick bookkeeping, buffer, acks,
                       hash compare, catch-up)
  src/replay.rs        Replay = session header + per-tick inputs; record, play, desync reports
  src/code.rs          join codes
  src/transport/       Transport trait; mem.rs (tests, simulated latency/jitter/loss on a seeded RNG),
                       ws.rs (web-sys on wasm, native client for tests), quic.rs (quinn, native, feature "quic")
crates/relay/          native server binary (tokio): rooms by join code, QUIC + WebSocket, forwards frames
crates/arena/          engine-free library (no threads, no filesystem; builds for wasm32) + native runner
  src/spec.rs          FightSpec (RON), fight codes (`SGF1:` + compressed RON, like share codes)
  src/run.rs           build_world(&Content, &FightSpec) -> Sim; run_fight -> FightResult
  src/score.rs         outcome, score, Bradley–Terry/Elo
  src/report.rs        Markdown / CSV
  src/evolve/          genome, operators, MAP-Elites archive (A5, A6)
  src/bin/arena.rs     native runner: std::thread::scope over all cores, reads packs with `data`'s `fs` feature
  sets/                developer fight sets and the check baseline (RON)
```

Neither `net` nor `arena` depends on Bevy, so Bevy's API breaks every 4–5 months don't reach them. Threads,
files and tokio stay in the native binaries (`relay`, `arena` runner). The libraries keep the rule "no threads or
filesystem required". Wire encoding is `postcard` 1.x: a stable, documented format, varints, f32 as its exact 4
bytes. `bitcode` 0.6 is smaller but its format may change between versions.

### 5.2 What the sim must change (track S1)

1. **Every world change goes through `step`.** Today the sandbox calls `spawn_ship`, `remove_ship`, `command`
   and `set_config` directly (`crates/game/src/scene.rs` `scene_tick` for waves, cleanup and re-forming;
   `toggle_waves`; `tuning.rs`). This is although `sim_bridge.rs` says "nothing writes back into the sim except
   the inputs". Add:

   ```rust
   pub enum WorldCommand {
       Spawn { design: Arc<ShipDesign>, owner: Owner, body: Body,
               behavior: Option<BehaviorId>, leader: Option<ShipId>, focus: bool, tag: u32 },
       Remove(ShipId),
       Teleport { ship: ShipId, piece: PieceId, body: Body },
       Config(SimConfig),
       AddBehavior { id: String, text: String },   // compiled on every peer, in the same order
   }
   pub struct TickInputs { pub world: Vec<WorldCommand>, pub controls, pub commands, pub damage }
   ```

   World commands apply first, in list order. `SimEvent::ShipSpawned` gets the `tag`, so the host's scene can
   tell which spawn became which `ShipId`. The wire form replaces `Arc<ShipDesign>` with a key into the session's
   design table (`net::wire`). The direct methods stay for tools and tests. In a session, only `step` changes the
   world. Wave positions computed with std `cos`/`sin` in `scene.rs` are fine, because the host sends the
   resulting f32 bits and guests never recompute them.
2. **Teams**: `SimConfig::teams: [u8; MAX_OWNERS]` (default: each owner its own team, so nothing changes in
   single player), part of the hash. `friendly` compares teams. It is used for behavior targets, no friendly fire
   (`Projectile::team`), plumes, ram damage in `collect_impacts`, and the client's friendly checks.
3. **Serde**: an optional `serde` feature on the input and config types: `Vec2`, `ShipId`, `PieceId`, `Owner`,
   `Target`, `AimMode`, `PilotAxes`, `PilotInput`, `Maneuver`, `FlightGoal`, `PowerSetting`, `ShipCommand`,
   `ShipControls`, `DamageCommand`, `Body`, `BehaviorId`, `SimConfig`, `FlightTuning`, `PhysicsTuning`. A test
   checks that −0.0, subnormals and every input round-trip bit-exactly.
4. **Hash parts**: `Sim::state_hash_parts()` returns sub-hashes (world header with tick, seed, config, RNG, ids and
   minds; ships; projectiles; Rapier bodies), plus a per-ship hash, so a desync report names what diverged.
   This costs the same as `state_hash` (0.285 ms for a 40 000-part world, so it is cheap once a second).
5. Nothing else in the tick changes. The determinism rules and the scenario hashes stay as they are. Scenarios
   may keep their direct calls.

### 5.3 Messages

Relay (`relay` ↔ peers): `Open` → `Opened { code }`; `Join { code }` → `Joined { peer }`; `Data { peer, bytes }`;
`PeerLeft { peer }`; `Close`. The relay never looks inside `Data`.

Session (inside `Data`, postcard):

```rust
enum ToHost {
    Hello { protocol: u16, build: BuildId /* game version, sim fingerprint */, content: ContentId, name: String },
    Loadout { designs: Vec<String> /* blueprint RON */, behaviors: Vec<(String, String)> /* id, text */ },
    Input { ack: u64 /* last confirmed tick held */, frames: Vec<InputFrame> /* unconfirmed, repeated */ },
    Hash { tick: u64, parts: HashParts },
    Ping { id: u32 }, Pong { id: u32 }, Bye,
}
struct InputFrame { seq: u32, commands: Vec<(ShipId, ShipCommand)> }

enum ToGuest {
    Welcome { player: u8, owner: Owner, header: SessionHeader },
    Refused { reason: Refusal },
    CatchUp { first: u64, ticks: Vec<WireTick> },                   // chunked
    Ticks { first: u64, ticks: Vec<WireTick>, landed: Vec<(u32, u64)> /* seq -> tick */ },
    Desync { tick: u64 }, Ping { id: u32 }, Pong { id: u32 }, End { reason: EndReason },
}
struct SessionHeader { seed: u64, start_tick: u64, config: SimConfig,
                       designs: Vec<(DesignKey, String)>, behaviors: Vec<(String, String)>, players: Vec<PlayerInfo> }
struct WireTick { world: Vec<WireWorldCommand>, commands: Vec<(ShipId, ShipCommand)> }
```

Unconfirmed frames and unacked ticks are repeated in every packet until acked, so a lost datagram costs nothing.
The QUIC transport sends them as unreliable datagrams; the WebSocket transport is reliable anyway. New designs
mid-session (a guest changes ship) travel in `WireTick.world` before the `Spawn` that uses them.

### 5.4 The session loop

Host, every `FixedUpdate`:

1. The scene and the local pilot fill `SimWorld::inputs` (world commands and the host's commands).
2. Guest frames that arrived since the last tick are added in arrival order (player slot breaks ties). Commands
   for ships the guest does not own are dropped.
3. The tick is encoded as a `WireTick`, appended to the session log (the replay) and sent to every guest.
4. `Sim::step`. Every 30 ticks the hash parts go into a ring buffer, to compare with the guests' reports.

Guest, every `FixedUpdate`:

1. Local input becomes an `InputFrame` and is sent. The guest's scene adds no world commands.
2. Step 0, 1 or 2 confirmed ticks (0 when the buffer is empty: hold; 2 when it is more than 2 above the target;
   *placeholder*: at most 4 per frame while catching up). Apply events after each step.
3. Every 30 ticks, send `Hash`.

The host never waits for a guest, so a slow guest only hurts itself. Clock drift between the peers is absorbed
by the 0/2-step rule.

### 5.5 Determinism, hashes and desync detection

- **Before a session**: the sim fingerprint and content id (§3.5) catch different builds and a platform that is
  not deterministic, before anyone flies.
- **During**: guests send hash parts every 30 ticks (debug builds: every tick, since `last_tick_hash` exists
  there anyway). The host compares them with its ring buffer. On a mismatch it sends `Desync`, and both sides
  save a report to player storage (`replays/` namespace, ADR-0031): the session header, the log up to the tick,
  and both sets of hash parts.
- **After**: `cargo run -p sim --example determinism -- --replay <file>` (new) replays a report natively and
  under `wasm32-wasip1`. `scripts/determinism-check.sh --compare` (existing) prints the first divergent tick, and
  the per-ship hash shows which ship diverged.
- **Replays everywhere**: a single-player sandbox session recorded the same way is a bug report, and an arena
  fight spec is a replay with no inputs (AI only). ADR-0010's "exact replays" come for free.

### 5.6 Client bridge changes (`crates/game`)

- `sim_bridge.rs`: a `NetSession` resource (none / host / guest) and a new `SimSet::Net` between
  `SimSet::Inputs` and `SimSet::Step`. `step_sim` steps the number of ticks the session allows, so `apply_events`
  must run after **each** step: `Sim::step` drops the previous step's events (factor out the event handling, or
  collect events into a buffer per step). `HashLog` is reused for hash reports.
- `scene.rs`: `PLAYER`/`ENEMY` constants become a `LocalPlayer { owner, team }` resource. `PilotFocus` mirrors
  `Sim::focus(local owner)`. Waves, cleanup and re-forming emit `WorldCommand`s and run only on the host or in
  single player, through the same code path, so single player becomes recordable too. The scene also spawns,
  re-forms and removes guest ships.
- `tuning.rs`: host-only in a session. `Config` changes go out as world commands. Content-rewriting scales and
  `SimWorld::others` are disabled in co-op.
- Friendly checks (clicks, Tab targets, markers) use teams. Teammates get a tint and a name tag.
- UI: HOST / JOIN (URL first: `?host=1`, `?join=CODE`, `?relay=ws://10.77.9.2:<port>`), code display and copy,
  lobby list, banners (`LAG`, `DESYNC`, `HOST LEFT`).
- `?inputdelay=N` (P1): delays the local pilot input by N ticks in single player, to judge the feel of guest
  latency before any network exists.
- `?scene=fight&fight=<code>` (A2): builds the world with `arena::build_world` and steps it with empty inputs.

### 5.7 The arena crate

- `FightSpec { seed, config overrides, sides: [Side { owner, entries: [Entry { design: content id | blueprint
  RON, behavior: content id | text | design default, count, formation }] }], start_distance, time_limit,
  stand_in: Option<StandIn> }`. `StandIn` is a scripted pilot that produces `PilotInput` per tick (the "passive"
  and "closing" stand-ins of the sandbox table in `docs/sim.md`), for player-like measurements. Everything else
  flies by behavior, so a fight needs no recorded inputs.
- `build_world` follows `scenario::load_test_with`: compile designs once per set (`Arc<ShipDesign>`), build
  `BehaviorLibrary::new(&content)` plus text behaviors, spawn, then command behaviors.
- `run_fight` steps until the end condition and collects `FightResult` from events: outcome, ticks, build points
  per side at the start and destroyed, parts and vessels lost, shots, hits, shield hits, rams, burns,
  overheated part-ticks, time to first hit and first kill, and the final `state_hash`.
- The runner always uses **release** builds. Debug builds hash every tick, which would dominate small fights.
- Cost (estimate): Runner vs 3 raiders is about the 5-ship load test, 0.003 ms per tick, so 90 s is about 8 ms.
  That is on the order of 1000–2000 fights per second on the VM's 16 cores. The hand-made table in `docs/sim.md`
  (144 fights) takes well under a second. One evolution generation (64 × 16 opponents × 4 seeds = 4096 fights)
  takes a few seconds.

### 5.8 Tests

- `sim`: world commands through `step` give the same hash as the direct calls they replace. Teams: co-op owners
  don't shoot, ram, burn or target each other, and single player is unchanged (scenario hashes identical). Serde
  round-trips are bit-exact. Hash parts change only where state changed.
- `net`: in-memory transport with seeded latency, jitter and 20 % loss. A host and two guests run 1800 ticks of
  a scripted session, and all hash streams are identical. A guest joining at tick 900 by catch-up matches the
  host. An injected divergence is caught at the next hash tick. Version and content mismatches are refused.
  Commands for foreign ships are dropped.
- Cross-platform: a session recorded natively replays identically under `wasm32-wasip1`
  (`determinism-check.sh` grows `--replay`). Two headless browser tabs (`scripts/web-drive.mjs`) play the co-op
  sandbox through a relay in the VM for 60 s and log hash streams, which are compared.
- `game`: a headless Bevy test (MinimalPlugins) with a host and a guest `SimWorld` on the memory transport runs
  the real sandbox scene for 30 s with equal hashes. This catches direct sim writes that bypass the inputs.
- `arena`: the same spec gives the same result and hash. Native vs wasm agree on a fight set. The parallel
  runner gives the same results as a serial run. Mutation operators only produce designs that pass
  `data::validate_ship` and have a core. Scoring has unit tests.

### 5.9 How it fits the existing code

- `Sim::step(&TickInputs)` is already the only seam the bridge needs. Ids are monotonic, `Sim` clones
  identically, and `state_hash`, `HashLog` and `determinism-check.sh --compare` exist.
- `SGD2` share codes already carry a design and its custom behaviors (the loadout).
- `scenario::load_test_with` is already a fight builder, `sim::design_report` is the evolution pre-filter, and
  `data::edit` / `data::behavior_edit` are the mutation operators.
- `test_range.rs` (watch and fly alongside) is where SIM BATTLE goes.

## 6. Tracks

A1 is the cheapest useful slice: it uses only today's sim API, needs no sim change, and turns the hand-measured
fight tables into a command.

| Track | Goal | Depends on | Size | Can run in parallel with |
|---|---|---|---|---|
| A1 Minimal headless arena | `arena` lib (`FightSpec`, `build_world`, `run_fight`, `FightResult`, score) + native runner over all cores + Markdown/CSV report. First sets: the `docs/sim.md` sandbox fight (Runner vs 3 raiders, seeds 1–12, passive/closing stand-ins), `skirmisher` vs raiders, a sweep of `raider_basic`'s stand-off and orbit | – | S | S1, P1, P2 |
| A2 Fight replays in the client | `?scene=fight&fight=<code>`, final hash check, codes in report rows | A1 | S | A3, S1, N1 |
| A3 Balance check | `scripts/arena-check.sh`, stored baseline, tolerances | A1 | S | A2, A4 |
| A4 Ratings and part reports | Bradley–Terry/Elo, pick rate, win rate when used, flags; a pool of archetype designs in `crates/arena/sets/` | A1 | M | A2, A3, N-tracks |
| A5 Behavior evolution | MAP-Elites over rule programs on fixed hulls (`BehaviorEditor` operators); first target `raider_basic` / `skirmisher` | A1, A4 | M | N-tracks |
| A6 Design evolution | `data::edit` operators, mirror lock, build-point budget, report pre-filter; then joint genome | A5 | L | N-tracks |
| A7 Number tuning and exploit hunts | Outer-loop proposals as RON diffs for review; hunts → check-set fights | A6, A3 | L | – |
| A8 SIM BATTLE in the test range | Player-facing fights in the client (time-sliced), win rate, watch | A2 | M | N3 |
| S1 Sim seams for sessions | `WorldCommand` in `TickInputs`, spawn tags, team table, `serde` feature, hash parts; the scene emits world commands | – | M | A1–A4, P1 |
| P1 Input-delay prototype | `?inputdelay=N` in the sandbox; Niklas judges 2–5 ticks of delay | – | S | everything |
| P2 ARM determinism | Build the `determinism` example for `aarch64-linux-android`, run it with `adb shell`, compare with native | – (needs the phone on adb) | S | everything |
| N1 `net` core | Protocol, wire mapping, replay format, host/guest sessions (sans-IO), memory transport, the session tests | S1 | M | A-tracks |
| N2 Relay and transports | `relay` binary (rooms, codes, WebSocket + QUIC), ws transport (wasm + native), QUIC transport (native) | N1 | M | N3 |
| N3 Co-op sandbox | `NetSession`, `LocalPlayer`, per-step events, host/join by URL, teammate look, banners, guest ships in the scene | S1, N1 (N2 for a real network) | L | N2, A-tracks |
| N4 Desync tooling | Hash exchange and reports, `determinism --replay`, native-vs-wasm replay check, two-tab browser check | N1, N3 | S | N5 |
| N5 Latency handling | Adaptive buffer, cosmetic latency hiding; a measured rollback prototype for the guest view (separate repo per ADR-0025 until chosen) | N3, P1 | M | N4, N6 |
| N6 Joining mid-voyage | Catch-up replay with a progress screen; a Rapier snapshot prototype only if needed (separate repo) | N1, N3 | M | N5 |
| N7 Live arena duel | Arena scene over the net layer, equal input delay, build-point cap | N3, A1 | M | A5–A7 |

Suggested waves: **A1** with **S1**, **P1** and **P2** → **A2, A3** with **N1** → **N2, N3** with **A4** → **N4, N5,
N6** with **A5** → the rest.

## 7. Risks and what to measure first

| Risk | Measure or prototype first |
|---|---|
| Guest delay feels sluggish | P1: delay single-player input by 2–5 ticks and let Niklas fly. If 4 ticks (about 133 ms) feels bad, plan N5's rollback early |
| Bandwidth | Estimate for C: a `Pilot` command is about 40 bytes, so about 1.2 KB/s per player at 30 Hz. With repeats and packet headers, about 3–5 KB/s up per guest and 5–15 KB/s down per guest with 4 players. A relay with 4 players carries tens of KB/s. **Measure** in N1's tests (postcard size per tick of a recorded sandbox). For the decision record, also measure once what snapshots of the 20-ship load test would cost per tick (poses, part states, projectiles; a one-off in a separate repo) |
| Tick cost with N clients | With C the host cost is flat in N (encoding and forwarding only), and each guest runs the full sim like single player. **Measure** host step + net time with 1–3 guests in the memory-transport test, guest catch-up speed (ticks per second) natively and in wasm, and later on the phone. Worst case today: the big fight at 0.43 ms median / 1.6 ms p95 natively against the phone's 4 ms step budget |
| A platform that is not deterministic (Android/ARM is unmeasured) | P2 now (no APK needed). The sim fingerprint at join catches it in the field |
| Client paths that write to the sim directly (scene, tuning, `toggle_waves`, content rewrites) | S1 moves them to inputs. The headless two-world `game` test catches the rest |
| Behavior ids differ between peers (ids are handed out in order; names can clash) | Namespaced ids, one library build order from the session header, a test |
| Rapier snapshot not bit-exact after restore | Only needed for snapshot joins. Prototype in a separate repo before relying on it |
| TLS crypto brings C (`ring`) | The tree already holds small C pieces (`android-activity`, `blake3`, `wayland-backend`). Avoid `aws-lc-rs` (needs cmake on some targets). `rustls-rustcrypto` (pure Rust) is still alpha. Note it in the transport ADR |
| Relay hosting | A small public server (cost, upkeep). It stores nothing and sees only codes and frames. Development runs it in the VM |
| Arena draws, noise and overfitting | Win-first scoring with the build-point tie-break, 4–8 seeds, a rotating pool, and outliers checked by replay |
| Large ships make fights slow | `sim_perf`: 20 × 2000-part ships at 0.22 ms per tick. Capital duels are still under 0.1 s per fight (estimate). Keep sets small and measure A1's fights per second |

## 8. Questions for Niklas

Ask interactively, one at a time. Each question stands on its own.

**Q1. Co-op netcode: how should a guest's game stay in step with the host's?**
- **Host sends inputs, every peer runs the same deterministic sim (Recommended)**: a few KB/s per player, the
  client code stays as it is, hashes catch desyncs. Guests need the CPU to run the fight, and it changes the
  snapshot part of ADR-0023.
- Host sends world snapshots (ADR-0023 as written): guests need little CPU and cannot desync. About 10× the
  traffic, and a second, render-only copy of the world in the client.
- Snapshots now, inputs later: starts as decided, but much of the work is done twice.

**Q2. Guests see their own input one round trip late (about 60–150 ms). How do we handle that?**
- **Input delay plus instant cosmetic feedback (stick, aim ring, lock) first; prediction only if play-tests need
  it (Recommended)**: simplest, and ships are heavy (about 1.5 s per 90° turn), so the delay hardly shows.
- Rollback prediction from the start: no felt delay for guests, but several ticks of the whole world are
  re-simulated every frame, which is costly in big fights on phones.
- Equal delay for everyone, host included: fair, which live PvP needs, but the host loses its zero delay in co-op.

**Q3. Which transport and relay for co-op?**
- **Our own tiny relay on a small server: QUIC natively, WebSocket for the browser dev build, all traffic
  forwarded by join code (Recommended)**: small and fully ours, two browser tabs can test co-op. Every packet
  takes a short detour, and someone runs the server.
- `iroh` (QUIC with hole punching and relay fallback, stable 1.x): mostly direct paths and end-to-end encryption.
  Heavier dependencies, and its free public relays are for development only, so we would self-host its relay
  anyway.
- WebRTC with `matchbox`: browser and native peer-to-peer. A heavy native stack, a TURN server for hard NATs,
  and the browser is no longer an end-user platform.

**Q4. When can a guest join a session?**
- **In the lobby, and mid-voyage by fast-forwarding the host's recorded inputs (Recommended)**: no world
  snapshots needed, and a 30-minute voyage catches up in seconds. Joins into very long sessions get slower.
- Lobby only: simplest, but nobody joins a running voyage.
- Anytime by full world snapshot: instant, but the whole sim including Rapier must serialise bit-exactly, which
  is unproven.

**Q5. What does a guest fly in co-op?**
- **Their own designs and behaviors, as their own fleet with their own focus, fleet bar and mind capacity
  (Recommended)**: "the ships you actually built", and the sim already keeps these per owner.
- Ships lent from the host's fleet: no design transfer, but the guest's own builds don't count.
- Escort mode, piloting one of the host's vessels: simplest for the guest, the least ownership.

**Q6. Can co-op teammates hurt each other?**
- **No, teammates count like your own vessels: no shots, rams or exhaust damage, and behaviors never target them
  (Recommended)**: the same rules as for your own fleet (GDR-0045, 0046, 0048), fewer accidents.
- Yes, friendly fire between players (your own vessels stay safe): more tactical, more accidents in brawls.

**Q7. How does the arena decide and score a fight?**
- **Win, loss or draw first (a side is out after 5 s without a running vessel, 90 s limit); a timeout scores by
  build points destroyed minus lost; averaged over seeds (Recommended)**: clear, and evolution still learns
  from timeouts.
- Win, loss or draw only: simplest, but kiting builds produce many draws that say nothing.
- Damage ratio only (build points destroyed / lost): smooth for evolution, but rewards trading damage over
  winning.

**Q8. What form should arena PvP take first?**
- **Async: your design and behavior (as a share code) fight other players' designs AI vs AI; live piloted duels
  later over the co-op layer (Recommended)**: cheap with the headless arena and no latency problems. It un-parks
  the "shared designs as opponents" idea.
- Live piloted duels first: closest to PvP, but needs fair latency handling and two players online.
- Neither for now: the arena stays a developer and test-range tool.

## Sources (checked 13 Oct 2026)

- crates.io (versions and dates read from the API): [quinn 0.11.12](https://crates.io/crates/quinn),
  [iroh 1.3.0](https://crates.io/crates/iroh), [matchbox_socket 0.14.0](https://crates.io/crates/matchbox_socket),
  [bevy_matchbox 0.14.0 (Bevy 0.18)](https://crates.io/crates/bevy_matchbox), [renet 2.0.0](https://crates.io/crates/renet),
  [renet2 0.19.0](https://crates.io/crates/renet2), [bevy_replicon 0.45.0 (Bevy 0.20)](https://crates.io/crates/bevy_replicon),
  [lightyear 0.30.1 (Bevy 0.19)](https://crates.io/crates/lightyear), [ggrs 0.13.0](https://crates.io/crates/ggrs),
  [bevy_ggrs 0.22.0 (Bevy 0.19)](https://crates.io/crates/bevy_ggrs), [aeronet 0.22.0-rc.1 (Bevy 0.20 rc)](https://crates.io/crates/aeronet),
  [postcard 1.1.3](https://crates.io/crates/postcard), [bitcode 0.6.9](https://crates.io/crates/bitcode),
  [rustls-rustcrypto 0.0.2-alpha](https://crates.io/crates/rustls-rustcrypto).
- quinn: tokio-based, rustls + ring, application datagrams: https://github.com/quinn-rs/quinn
- iroh: FAQ (public relays free for development and testing, rate-limited; self-hosting; about 9 in 10
  connections go direct): https://docs.iroh.computer/about/faq ; browsers always go through a relay:
  https://docs.iroh.computer/languages/wasm-browser ; crate docs (hole punching, relay fallback, `noq`, tokio):
  https://docs.rs/crate/iroh/latest ; 1.0 release in June 2026 (third-party write-up):
  https://pinggy.io/blog/iroh_1_0_dial_keys_not_ips/
- matchbox (WebRTC, native + wasm, signalling server, GGRS socket, Bevy version table):
  https://github.com/johanhelsing/matchbox
- Rapier determinism and serialisation: https://rapier.rs/docs/user_guides/rust/determinism ,
  https://rapier.rs/docs/user_guides/rust/serialization
- Deterministic lockstep in practice (Factorio): https://factorio.com/blog/post/fff-76 ,
  https://factorio.com/blog/post/fff-83 , https://factorio.com/blog/post/fff-188
