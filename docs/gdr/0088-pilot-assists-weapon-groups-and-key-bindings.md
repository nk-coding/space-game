# GDR-0088: Pilot assists as rules, weapon groups, and an action map for key bindings

- Status: Accepted (the choices below by Niklas; details marked "lead default" for him to review); step 1 (the action map, SETTINGS) built 2026-10-15, its open points decided in [GDR-0090](0090-action-map-settings-and-pause-menu.md); step 2 (weapon groups: the drydock's GROUP pills, FIRE GROUP 1–3 on 1 / 2 + right button / 3, Space fires all, the pilot's fire mask in the sim) built 2026-10-15; step 3 (the assists) built 2026-10-15: a PILOT ASSISTS tab next to GRAPH and TEXT on the BEHAVIOR screen edits the design's list (blueprint `assists`) with the same graph; actions `face_target`, `brake`, `auto_fire(group)`, each on one channel (turning, thrust, a group's trigger), the pilot's use of a channel winning it at once; toggle slots ASSIST TOGGLE 1–4 on F1–F4 and ALL ASSISTS on Z, the slot and the start state being each rule's `toggle` and `start` values; pilot conditions `lock_held`, `idle(any/turn/thrust/fire)`, plus `speed` and `can_stop_straight` for both lists; a HUD block under the speed and a banner per switch. Choices where this record was open (lead defaults): the assists' target is the pilot's lock while one is held, else the list's target line (default nearest), so face only acts without a lock (a lock aims by itself); brake asks zero velocity without turning (only thrusters against the motion brake); every matching rule decides its channel (no first-match stop, no rule slots); a list may be empty; switch states live on the ship for the mission (a split-off vessel inherits them, a re-formed ship starts again); the debug overlay moved from F3 to F8 to free F1–F4; ALL ASSISTS defaults to Z (free in flight); assists are saved with the design, not as their own documents; step 4 (hold station, collision avoidance) built 2026-10-15: assists `hold_station(point|ally)` and `avoid_collisions`, both on the thrust channel; the BEHAVIOR screen restructured (Niklas, 15 Oct 2026, the step 3 split was weird): two tabs BEHAVIOR | PILOT ASSISTS over the graph, each shown as GRAPH or TEXT (one view choice for both), the assists' TEXT view editing the list in the blueprint's rule text. Choices where this record was open (lead defaults): hold station keeps the point where the assist took thrust over (where the pilot let go), or with `ally` the offset then from the nearest other active vessel of the owner, moving with it; the pilot's thrust drops the station (flying moves it), a rule above that takes thrust for a while (a dodge, a brake) leaves it waiting; it flies back like an escort at distance 0 and never turns the ship; collision avoidance looks at every piece of every ship (vessels of any owner, wrecks, own debris) as bounding circles on straight lines, 6 s ahead with 2 m to spare, decides thrust only while something is on a collision course (so the rule below keeps it otherwise) and sidesteps along the miss (left of the relative motion head-on) without turning; both use only the thrusters that push the needed way, like brake; behaviors don't get avoidance or hold station (their `hold` and `escort` stay as they are, and every AI stream stays byte-identical); a determinism scenario `assists` flies the flight scenario with every assist; thrust assists turning (Niklas, 2026-10-15, as no base ship has RCS pods): brake, hold station and avoid collisions keep owning thrust and also turn the ship while turning is free (the pilot doesn't steer or hold a lock, no rule such as face target decides it), with the behaviors' flip and burn (`HeadingGoal::Auto`); lead defaults: only for a velocity change of at least 0.5 m/s (no spinning around for a slight drift), and a ship that must turn around to stop comes back to its station no faster than it can still turn and stop, and within 1 m of it only stops; with turning taken they are thrust-only as before; AI streams unchanged, the `assists` stream changes
  - 2026-10-15, what above is stale (the text stays as written): brake, hold station and avoid collisions turn the
    ship while turning is free (the status's last part), so "without turning" and "never turns the ship" hold only
    while the pilot or a rule has the turning channel; a matching rule decides its channel only when no rule above
    it already does (two brakes are a priority list of their own); group 1 also fires while the left button is held
    on empty space.
  - 2026-10-15, the assists act on the lock only (Niklas: "an auto target would be an assist in itself (which
    chooses the target automatically, e.g. on a button press), but the assists should not by default just target
    sth which is then not even communicated to the user"; he chose both, a lock key and an auto-lock assist): this
    replaces the lead default above that gave a list a target line (default nearest). An assist list has no target
    line; what needs a target (auto-fire) acts on the pilot's lock, the one the HUD shows, and without one does
    nothing. `face_target` is removed (lead): acting on the lock only, it would never act, as a held lock already
    turns the strongest guns onto its ship by itself (`pilot_goal`'s lock heading, smart aim, GDR-0040) and holds
    the turning channel, so no part was left for it; the turning channel goes with it. New assist
    `auto_lock(pick, toggle, start)` on a new lock channel, its pick one of the behaviors' target selectors that
    suit a pilot (`nearest`, the default, `weakest`, `threat`, `attacker`; not `locked`): while the pilot holds no
    lock it asks for the ship its pick finds, and the client locks it as a click would (one lock for the HUD, the
    target panel and the sim; the rules below get the channel while its pick finds nobody). Lead default for the
    re-lock guard: a lock the pilot drops while its ship is still there (Esc, a click on empty space) pauses
    auto-lock until the pilot locks again or switches an auto-lock rule (its toggle, ALL ASSISTS), and the HUD shows
    it PAUSED; a lock lost with its ship (destroyed, gone dark) is no drop, so auto-lock moves on to the next.
    LOCK TARGET is the existing `lock_next` action on Tab (free in flight; renamed "Lock target", its id kept for
    saved bindings): the first press locks the nearest enemy, each next press the next by distance, round again;
    the order now comes from the sim (`sim::flight::next_lock`, the ship's own position, ties by id), not the
    interpolated camera. Old blueprints and saves: an assist list's target line and its `face_target` rules are read
    and left out, and written back without them (no format version change: the new text reads in older builds).
    Every AI stream unchanged; the `assists` scenario changes (it now exercises auto-lock, dropping a lock and LOCK
    TARGET).
  - 2026-10-17: Accepted (Niklas, 17 Oct 2026): the assists and the lead defaults above are kept.
  2026-10-17: `avoid_collisions` sidesteps asteroid rocks too (an anchored circle on the same course test, [GDR-0104](0104-scenery-rocks-filler-landmarks.md)); behaviors' and maneuvers' goals now bend round rocks (not round ships: the rule above stands for them).
  2026-10-18: assist lists with the old `target` line or `face_target` rules are no longer read ([ADR-0044](../adr/0044-no-save-compatibility-before-release.md)): such a design fails to load.
- Date: 2026-10-14
- Decided by: Niklas (assist model, scope, first actions, override, default, weapon groups, fire keys, bindings);
  lead defaults where marked
- Related: [GDR-0086](0086-letting-go-coasts.md), [GDR-0015](0015-layered-behavior.md),
  [GDR-0016](0016-combat-pace-and-aiming.md), [GDR-0034](0034-behavior-first-match-fire-at-will.md), [GDR-0040](0040-keyboard-flight-and-smart-lock-aim.md),
  [GDR-0009](0009-stick-and-lock-controls.md), `docs/behavior-format.md`, `docs/client.md` "Controls",
  `docs/plans/android.md` (touch questions)

## Context

GDR-0086 took the brake-on-release out and left it to come back "as a configurable assist". Niklas, 14 Oct 2026:
"in general ship automation should be configurable via this behavior stuff, there should be a way to automate not
only autonomous vehicles, but also assists for the main ships, as well as bindings (e.g. different keys to shoot
different weapons or so …)", and "regarding the assist customization, I think that is something we can discuss
later in detail when the subagent does the code refactoring" (ADR-0038).

The state on 14 Oct 2026:

- Keys are `KeyCode` literals in about 20 files; nothing can be rebound or saved; gamepads are compiled out.
- The flight computer's only player-switchable assist is drift (X). Lock aims the ship (smart aim) but never fires.
- The trigger fires every gun at once (`Fire::All`). Per-gun firing exists in `ShipControls` but only behaviors
  use it (fire at will, GDR-0034). No groups, no turrets.
- Behaviors are rule lists that pick one maneuver; only unpiloted vessels run them. The focus parks its behavior.

## Decision

### Pilot assists are rules, per design (Niklas)

- A design gets a second rule list, **PILOT ASSISTS**, next to its behavior, edited on the BEHAVIOR screen with the
  same editor, conditions and target selectors (`docs/behavior-format.md`). It runs only on the ship the player
  flies (the focus); the behavior keeps running only when nobody flies the ship.
- Saved with the blueprint, like its behavior. A design without assists gets **none: pure coast**, today's feel
  (Niklas). A player-wide default for designs without their own can come later.
- **First actions** (Niklas: all four):
  - **Auto-fire when bearing**: fire a weapon group by itself when its guns bear on the target (the same test as
    fire at will: within 0.12 rad and 93.3 % of reach, GDR-0034).
  - **Face target when idle**: keep the strongest guns on the lock (smart aim) while the ship coasts.
  - **Brake when idle**: the old stop on release, now opt-in and conditional (e.g. only with no enemies near).
  - **Hold station / avoid collisions**: keep near a point or an ally, and steer clear of collisions. Collision
    avoidance does not exist in the sim yet (GDR-0015 promised it); it is new work.
- **Channels, the player always wins** (Niklas: "fill idle channels"): turning, thrust and each fire group are
  separate channels. An assist acts only on channels the player is not using right now: holding W leaves the nose
  free for "face target", pressing a group's trigger takes that group from auto-fire. Releasing a channel hands it
  back.
- **Switchable during a mission, by key and by condition** (Niklas, 14 Oct 2026: "there should also be a way to
  disable/enable assists during the mission, both via keys and conditions (e.g. auto stop only when no turning
  necessary, etc.)"):
  - Keys: each assist rule can be given a toggle action in the action map (and a key that turns all assists off
    and on); the HUD shows which are on. The on/off state lasts for the mission; each mission starts with the
    design's setting.
  - Conditions: assist rules use the behavior conditions plus new ones about the pilot's own flight, e.g. "the
    ship can stop without turning" (an engine already faces against the motion), the speed, whether a lock is
    held, the time since the player last used a channel. So "brake when idle, only when no turn is needed" is one
    rule.
- Lead default: assists are evaluated in the sim on the focus (deterministic, at the behavior rate of 10 Hz, so
  replays and the coming host-authoritative co-op see the same thing); `PilotInput` says which channels the player
  holds. For each channel the first matching rule that claims it decides (behaviors pick one maneuver per ship; an
  assist rule claims only its channels, so "face" and "fire group 2" can run together).

### Weapon groups set in the drydock (Niklas)

- Every gun has a group, 1 to 3, set in the drydock; a gun without one is group 1, so a fresh design fires
  everything with one trigger as today.
- Default inputs (Niklas): **left mouse: group 1, right mouse: group 2, Space: all groups, keys 1 / 2 / 3: each
  group.** Left-click still locks a ship, and holding it on empty space still fires (GDR-0040's 0.18 s hold), now
  group 1.
- Lead default: the touch FIRE button fires all groups until the touch layout (`docs/plans/android.md`) is decided.
- Lead default: the debug toggles on 1 / 2 / 3 (debug shapes, glow sprites, bloom) move to F5 / F6 / F7.
- Behaviors (unpiloted ships) keep firing each gun at will; groups are for the pilot and the pilot's assists.

### An action map for bindings, now, with a settings screen (Niklas)

- One table of named actions (fire group 1–3, fire all, lock next, unlock, drift, focus next, recall, zoom, …),
  read by every screen instead of key literals. The defaults live in the content pack (content lives in data);
  the player's changes are saved in the `settings/` storage namespace that `docs/drydock.md` planned.
- A settings screen rebinds them (lead default: reached from home and from the pause menu; one action can have
  two inputs, mouse buttons included). The map is shaped so that gamepad and touch inputs can be added to it
  later.

### Order of work (lead default)

After the structure refactor (ADR-0038) has landed, since all of this touches code it moves:
1. The action map and the settings screen (every later step adds actions to it).
2. Weapon groups (data, drydock, input, the per-group fire in `PilotInput`).
3. The assist rules: the list on the BEHAVIOR screen, the channel model in the sim, face / brake / auto-fire,
   the in-mission toggles and the pilot's flight conditions.
4. Hold station and collision avoidance (new flight-computer work).

## Consequences

- Automation stays one system: the same rules drive unpiloted ships and assist the piloted one, as Niklas asked.
- The blueprint format gains a group per gun and an assist list; the behavior format gains the assist actions
  and channels. Both must stay wiki-ready (labels and text in data).
- The sim's determinism hashes for behavior and flight scenarios will change when assists and groups land, and
  are recorded again then; with no assists set, flight stays exactly as GDR-0086 left it.
- Open: the assist list's layout on the BEHAVIOR screen; how collision avoidance steers (and whether behaviors get
  it too); the touch controls for groups and assists (`docs/plans/android.md`); gamepads; a player-wide assist
  default.
