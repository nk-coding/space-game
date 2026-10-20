# ADR-0046: One dev mode replaces the URL flags

- Status: Accepted
- Date: 2026-10-20
- Decided by: Niklas
- Related: ADR-0006, ADR-0033, ADR-0044, ADR-0045, `docs/client.md` "URL parameters", `docs/career.md`

## Context

The client had grown about 25 launch parameters (the page URL on the web, `GAME_PARAMS` natively, parsed in
`kit::params`): save slots, free unlocks, play-test bands, practice launches into voyages and generated clusters, a
debrief fixture, wingmen, render switches, a start zoom, a forced planet, a forced touch screen and more. Each came
from one task; together they were confusing, several overlapped (`unlock=all`, `bands=`, `save=`), and some were
documented as if they were player features. Niklas wants one switch for his own dev play.

## Options considered

- **Keep the flags, document them better**: reason not recorded (Niklas found the many flags confusing).
- **One `dev=1` mode, a handful of tool flags for scripts, the rest dropped** (chosen).

## Decision

**`dev=1`** is the dev mode:

- It plays its own save (`saves/dev`) and never touches the career (`saves/career`). Without it the normal career.
- Everything is open: every research node researched, every part, material, frame and facility unlocked, every
  material and research family in store up to the store's bound (refilled at every load), launch stock unlimited.
- The whole galaxy is charted and every gate open, so every band is open and a jump reaches any cluster.
- The voyage map marks each cluster's notable sites (a gate's guardian, a boss, a station: from the encounters' tags
  and anchored ships) and a cluster's panel lists every encounter its interior holds, with counts (the interior from
  the cluster's own seed, `galaxy::interior`).
- The debug keys work only with it: F8 (debug overlay), F5–F7 (render toggles), Shift+R (restart the test range), C
  (waves on and off); elsewhere the controls overlay, SETTINGS and the flight menu leave them out. P (pause) and H
  (hide the HUD) stay player keys.
- `newgame=1` works only with `dev=1` and starts the dev save over.

**Tool flags** (for scripts and agents, no player feature, they work without `dev=1`, listed apart in
`docs/client.md`): `scene`, `behavior`, `fight`, `load`, `seed`, `hashlog` / `hashticks`, `hud`, `slowmo`.

**Dropped** (code, tests, docs and words): `twin`, `fire`, `voyage` (and `scene=voyage`), `cluster`, `galaxy`, `save`,
`unlock=all`, `bands` (with `content/base/playtest.ron` and its loader), `debrief`, `lab_workers` (the lab always uses
its workers), `fleet`, `debug` / `bloom` / `glow` / `stats` as URL flags (they stay keys in the dev mode; the saved
display settings for glow and bloom stay), `zoom`, `planet`, `touch` (auto-detected only: the browser's coarse
pointer, as DevTools' device mode gives it, or a first touch).

## Consequences

- No practice runs any more: every launch the map allows is recorded, in the dev save too.
- Screenshots of the deep bands, the debrief or the touch layouts go through the dev mode, the map and the headless
  helpers' touch emulation (`scripts/web-drive.mjs --touch` / `--dpr=N`, `tools/headless/shot.mjs --touch` /
  `--phone`); `web-drive.mjs` emulates a mouse unless asked.
- Older docs and records that name a dropped flag describe how something was checked then; they are history, not a
  current feature.
