# GDR-0088: Pilot assists as rules, weapon groups, and an action map for key bindings

- Status: Accepted (the choices below by Niklas; details marked "lead default" for him to review); step 1 (the action map, SETTINGS) built 2026-10-15, its open points decided in [GDR-0090](0090-action-map-settings-and-pause-menu.md)
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
