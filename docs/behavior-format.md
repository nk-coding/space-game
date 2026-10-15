# Behavior format

How ships think, written down. For players who switch the behavior editor to text, and for modders who write
behaviors in content files. Written 11 Oct 2026 (behavior system v1). The vocabulary (every condition, action
and target selector, with its parameters) is content: `content/base/behavior_nodes/*.ron`, and the wiki lists
it from there. This page explains the grammar.

## What a behavior is

A behavior is a **priority list of rules**. Several times a second the ship looks at the list from the top; the
**first rule whose conditions all hold decides** what the ship does until the next look. A rule has
conditions (violet in the editor) and one action (teal). Actions are built-in maneuvers (approach, orbit,
...); the flight computer flies them, so a behavior can never send a ship spinning.

Guns **fire at will** whatever the action, each gun on its own: a gun fires whenever the target (for cannons,
the point where the target will be when the shot gets there) is in front of **that gun** and within its reach.
Front guns fire while the ship faces the target; side and rear guns fire when the target crosses their line,
e.g. while the ship orbits.

A ship can run only as many rules as its **logic cores** give it rule slots (a logic core gives six, an
automaton core two). Rules below that line are inactive; if a core is shot away or overheats, the lowest rules
switch off first. If no active rule matches, the ship stops and holds.

## One line per rule

```text
target weakest(engines)
when hull < 30% -> flee                                   # run when badly hurt
when shield_down and target_on_me -> orbit(80%, ccw)      # dodge while the shields recharge
when target_in_range(120%) and cycle(9s) >= 5s -> orbit(58%)
always -> approach(85%)
```

| Line | Meaning |
|---|---|
| `target <selector>` | Optional, first line: who the rules are about. Default `target nearest`. |
| `when <condition> and <condition> ... -> <action>` | A rule: all conditions must hold. |
| `always -> <action>` | A rule without conditions (usually the last one). |
| `... # note` | A note at the end of a line: one line of text (no tabs or other control characters). It belongs to the rule and survives the graph view. |

There is no `end`, no `or` and no nesting. For "this **or** that", write two rules with the same action.

### Conditions

- **Flags** are true or false and stand alone: `shield_down`, `under_attack`, `target_in_range(60m)`.
- **Values** measure something and must be compared with `<`, `<=`, `>` or `>=`: `hull < 30%`,
  `enemies_near(150m) >= 3`, `cycle(9s) >= 5s`.
- `not` in front turns a condition around: `not target_in_range(100%)`.
- A condition about the target (`target_in_range`, `target_hull`, `target_on_me`, `cycle`, `target_time`, ...) is
  **false while there is no target**, with or without `not`. An action that needs a target does not run without
  one: the next rule gets its turn.

### Actions

`approach(range)`, `orbit(radius, dir)`, `kite(range)`, `face`, `flee`, `hold`, `escort(distance)`. See the
vocabulary in the wiki (or `content/base/behavior_nodes/actions.ron`) for what each does.

`escort(distance)` (added 13 Oct 2026, GDR-0065; default `30m`) keeps the ship within that distance of its
**leader**, the ship it escorts (for wingmen: the player's flagship): it closes in when farther and keeps pace
when near. It is the one action that needs no target, so a rule with it always runs when its conditions hold;
put it last, below the rules that fight, and it runs between fights. Without a leader the ship holds where it is.

### Target selectors

`nearest`, `weakest`, `threat`, `attacker`, `locked` (the ship your leader, usually the player, has locked).
Each takes an **aim point**: `centre` (default), `engines`, `weapons`, `power` or `core`, e.g.
`target nearest(engines)` aims at the nearest working engine of the nearest enemy. A ship keeps its target
until another is clearly better, so it does not flip between two that are nearly equal.

### Values and units

Values go in brackets after a name, separated by commas, in the order the node lists them. Values you leave
out take the node's default: `orbit` = `orbit(60%, auto)`.

| Kind | Write | Notes |
|---|---|---|
| Distance | `45m` or `45` | metres |
| Distance | `60%` | a share of **your weapon reach** (the range of your longest clear gun), so one behavior fits ships with different guns |
| Percentage | `30%` | always with `%` (`hull < 30` is an error, to avoid mixing up 30% and 0.3) |
| Time | `5s` or `5` | seconds |
| Count | `3` | whole numbers |
| Choice | `cw` | one of the words the node lists |

Numbers use a decimal point and a leading digit (`0.5`, not `.5`).

### Typing on a phone

Names and keywords are not case-sensitive (`When Hull < 30%` works; it is printed in lower case). `if` works
for `when`; `→` works for `->`; `≤` and `≥` work for `<=` and `>=`. Spaces are free (`orbit( 45 , cw )`).
The printed form is always the canonical one; parsing it again gives the same behavior. That is why a line must
fit 240 characters both as typed and as printed: `always->flee#…` typed compactly gains spaces when printed
(`always -> flee # …`), so a line that grows past 240 is refused ("too long once written out").

### Errors

Every error names the line and the column and says what was expected, e.g.:

```text
line 4, column 6: unknown condition `sheild_down`; did you mean `shield_down`?
line 2, column 13: write a percentage with `%`, e.g. 30%
line 3, column 17: expected `and` or `->`, found `flee`
line 5, column 29: `sideways` is not an option; choose one of: auto, ccw, cw
```

## Pilot assists

Added 15 Oct 2026 ([GDR-0088](gdr/0088-pilot-assists-weapon-groups-and-key-bindings.md) step 3). A design can carry
a second rule list, its **PILOT ASSISTS**, written in exactly this format and edited on the BEHAVIOR screen's PILOT
ASSISTS tab with the same GRAPH and TEXT views, inspector and palette (the TEXT view since step 4: the lines below are
what it shows and what the blueprint stores). Assists help the player fly the ship; they run only on the
ship the player flies (the focus), never on an unpiloted one, whose behavior flies it.

```text
target nearest
when idle(thrust) >= 1s and can_stop_straight -> brake(toggle2)   # stop when I let go, if no turn is needed
always -> face_target(toggle1)                                     # guns on the nearest enemy while I don't steer
when target_in_range(90%) -> auto_fire(2, toggle3, off)           # group 2 fires by itself once F3 is pressed
```

With step 4 (15 Oct 2026), steering clear and keeping a place:

```text
when idle(thrust) >= 1s -> avoid_collisions(toggle1)   # sidestep what I'm coasting into
always -> hold_station(ally, toggle2)                  # else keep my place beside my nearest ship
```

- **Actions**: the assists, and only they: `face_target` (turning: the strongest guns onto the target, as a lock
  aims), `brake` (thrust: zero velocity), `hold_station(at)` (thrust: back to the station and keep it; `point`, the
  default: where the assist took thrust over, e.g. where you let go of W; `ally`: your place then beside the nearest
  of your other vessels, moving with it; thrusting moves the station), `avoid_collisions` (thrust: sidestep a ship,
  wreck or piece of debris on a collision course within a few seconds; it decides only while something is, so the rule
  below it keeps thrust the rest of the time) and `auto_fire(group)` (that weapon group's trigger: each of its guns
  fires when it bears, the test of fire at will). The behaviors' maneuvers are refused in an assist list, and the
  assists in a behavior.
- **Thrust assists turn while turning is free** (Niklas, 15 Oct 2026; no base ship has RCS): while you don't steer
  or hold a lock and no rule (face target) decides turning, brake, hold station and avoid collisions turn the ship
  to bring its engines to bear (flip and burn, as behaviors' maneuvers do), but only when the thrusters already facing
  the needed way give less than half its best push, and never for a slight drift (under 0.5 m/s), so it doesn't spin
  around. While turning is taken they use only the thrusters that push the needed way: a ship with engines only at the
  back then can't brake, hold station or sidestep. A ship that must turn around to stop holds station within about
  1 m (inside it, it only stops).
- **Channels, the player wins**: each assist works one channel: turning, thrust, or one weapon group's trigger. A
  channel the player uses this tick (A/D or the stick, or a held lock, which aims by itself; W/S/Q/E or the stick;
  the group's trigger) is the player's; letting go hands it back at once.
- **Which rule decides**: the rules are read from the top at every think (10 Hz, with the behaviors); for each
  channel the first rule that is switched on, whose conditions hold (and that has a target, if it needs one;
  `avoid_collisions`: something on a collision course) decides. So an `always` rule doesn't stop the rules below it: they may decide other channels. A list may be
  empty (no assists: the ship coasts when let go, GDR-0086). Assists need no rule slots.
- **Toggles**: the last two values of every assist: `toggle` (`none`, `toggle1` to `toggle4`: the ASSIST TOGGLE key
  that switches it on and off in flight, F1 to F4 by default) and `start` (`on`, `off`: how each mission starts).
  ALL ASSISTS (Z) switches every assist off and back as the toggles left them. The switches last for the mission.
- **Target**: the player's lock while one is held; else the target line (default `nearest`). `locked` (the leader's
  lock) finds nothing for the ship the player flies.
- **The pilot's conditions** (assists only): `lock_held` (a lock is held) and `idle(control)` (seconds since the
  player last used `turn`, `thrust`, `fire` or `any` of them; from the start of the mission if not yet). Every
  behavior condition works too, and two new ones work in both lists: `speed` (a share of the top speed) and
  `can_stop_straight` (thrusters already push against the motion, so the ship can stop without turning).
- **Stored** in the design's blueprint, beside the behavior it names: `assists: (target: "nearest", rules: [...])`
  (`docs/content-model.md`). Checked with `data::behavior::check_assists`; the vocabulary marks the assists' nodes
  `pilot: true`.

## Grammar

```text
program   = { line "\n" }                         blank lines are ignored
line      = target-line | rule
target-line = "target" call [ note ]              at most one, before the rules
rule      = ( "when" | "if" ) cond { "and" cond } arrow call [ note ]
          | "always" arrow call [ note ]
cond      = [ "not" ] call [ cmp number ]
call      = name [ "(" [ value { "," value } ] ")" ]
value     = number | name
number    = [ "-" ] digits [ "." digits ] [ unit ]     unit: "m" | "s" | "%"
cmp       = "<" | "<=" | ">" | ">=" | "≤" | "≥"
arrow     = "->" | "→"
note      = "#" text to the end of the line         no control characters (tabs, line breaks)
name      = letter { letter | digit | "_" }        case-insensitive
```

Keywords (`when if always and or not target`) cannot be node or option names. Limits: 64 rules, 8 conditions
per rule, 4 values per node, 240 characters per line as typed and as printed (`check_program` checks the printed
form, so content, stored and shared behaviors and the editor agree). Reading a text stops at the line after the
target line and 64 rules (or lines with errors) with "at most 64 rules"; the RON form refuses more than 64 rules
before reading any.

## In content files

A behavior in `behaviors/*.ron` holds exactly these lines: the `target` field is the target line without the
keyword, `rules` has one string per rule. No conversion is involved, so the wiki and the editor show what the
file says.

```ron
(
    id: "raider_basic",
    name: "Raider pack",
    summary: "Close in and shoot, then strafe around the target for a few seconds, and again.",
    tags: ["automaton", "attack"],
    target: "nearest",
    rules: [
        "when target_in_range(120%) and cycle(9s) >= 3s -> orbit(58%)",
        "always -> approach(85%)",
    ],
)
```

A faction names the default behavior of its ships (`behavior: "raider_basic"`); a ship can name its own. The
behavior a ship flies must fit the rule slots of its logic cores (validation checks it).

The base templates: `raider_basic` (the automatons' default), `skirmisher` (the ship-mind's default for
unpiloted vessels) and `escort` (for wingmen; added 13 Oct 2026, GDR-0065):

```text
target locked(core)
when hull < 30% and target_on_me -> kite(150%)   # badly hurt and under its guns: open the gap
always -> approach(70%)                          # nose on the leader's target, inside every gun's reach
always -> escort(30m)                            # nothing locked: keep near the leader
```

It fights only what the leader has locked (`approach` fails without a target, so the next rule runs), aims at the
target's core, and keeps near the leader between fights.

The player's own behaviors are saved in exactly this format (one document per behavior, never under a content
behavior's id), and a design names one the same way; see `docs/drydock.md` "Behaviors: storage, designs and
tests".

## Rhai conditions

A condition node with a `script` is a custom condition written in [Rhai](https://rhai.rs) (tagged "Rhai" in
the editor). The script ends in `true` or `false`:

```ron
(
    id: "closing_in", name: "Closing in", role: Condition, tags: ["target", "rhai"],
    summary: "At the current closing speed, the target will be within your weapon reach in less than the given time.",
    params: [(id: "time", name: "Time", type: Seconds, default: "3s", min: 0, max: 60)],
    script: "target.exists && target.closing > 0.0 && (target.distance - me.reach) / target.closing < time",
)
```

What a script can see:

| Name | Meaning |
|---|---|
| `me`, `target` | read-only views of your ship and its target (`target.exists` is false without one; its numbers are then 0) |
| `.hull` `.shield` `.heat` `.power` | shares 0..1, as the conditions of the same name |
| `.x` `.y` `.vx` `.vy` `.speed` `.angle` | position (m), velocity (m/s), nose angle (rad, counter-clockwise, 0 = up) |
| `.reach` | weapon reach (m): the range of the longest working gun; `me.reach` is the reach `%` distances use, 60 m on a ship without one (`target.reach` is then 0) |
| `target.distance` `target.bearing` `target.closing` `target.aims_at_me` | distance (m), angle off your nose (rad, left positive), closing speed (m/s), whether it aims at you |
| `me.enemies` | number of active enemy ships |
| parameters | by id, as numbers in base units (metres, shares 0..1, seconds) |
| functions | `abs min max clamp floor ceil sqrt sin cos atan2 exp pi to_float to_int` |

Rules for scripts: no `print`, no time, no files, no `import`, no `eval`, no `**` (powers go through the
platform's maths library, which is not bit-identical everywhere; multiply instead). Each run has a budget of
2000 operations; a script that runs out, errors or returns something other than `true`/`false` counts as
**false**, and the debug view shows why. Scripts are checked when the content loads (syntax, unknown variable
names).

## Editing model

`data::behavior::edit::BehaviorEditor` (`crates/core/data/src/behavior/edit/`, added 13 Oct 2026) is the editing model
of the BEHAVIOR screen (GDR-0057), the counterpart of the drydock's `data::design::edit::Drydock`: no Bevy, no threads, no
filesystem, so it builds for wasm32. Both views edit through it: the graph view and the inspector call its
operations, the text view replaces the program with `set_text`. Tests: `crates/core/data/tests/behavior_edit.rs`.

### Program, ids and results

- `BehaviorEditor::open(nodes, &program)` (e.g. a content behavior's `program()`, used as a template) or
  `from_text(nodes, text)`; `open_as` / `from_text_as` with a `ListKind` open a design's pilot assists
  (`ListKind::Assists`): the list may have no rule, the palette and every edit keep to the assists' nodes
  (`Refusal::NotInList` otherwise), and `kind()` says which list it is. The editor copies the vocabulary (`Content.behavior_nodes`), so it owns its data.
  `open` reads the program through its printed text, so its errors locate lines of `Program::to_text`.
- **The program is always legal**: it passes `check_program` against the vocabulary, has 1 to 64 rules, at most 8
  conditions per rule and at most 240 characters per line, and its canonical text gives it back (model → text →
  model is the identity; the graph and the text view cannot disagree). An edit that would break that is refused
  with a `Refusal` (`Display` gives the reason for the player) and changes nothing.
- Rules and conditions have **stable ids** (`RuleId`, `CondId`); a node is a `NodeRef`: `Target`, `Action(rule)` or
  `Cond(cond)`. An id names the same rule or condition across edits, undo and redo until it is deleted; undoing the
  delete brings it back under the same id. Ids belong to the editor that handed them out. Rule positions
  (`RuleView::index`, the priority) shift when rules are added, deleted or moved.
- Edits that create something return its id; the others return `Ok(changed)`. Only changes bump `revision()` and
  make an undo step.

### Operations

| Operation | What it does | Refused when |
|---|---|---|
| `add_rule(index, action)` | `always -> action` at a position (0 = top) | `BadIndex`, `TooManyRules`, `UnknownNode`, `WrongRole` |
| `delete_rule(rule)` | | `LastRule` (a behavior keeps one rule; pilot assists may have none) |
| `duplicate_rule(rule)` | a copy (with its note) right below, new ids | `TooManyRules` |
| `move_rule(rule, to)` | to a position; up = `index - 1`, down = `index + 1` | `BadIndex` |
| `set_rule_note`, `set_target_note` | trimmed; empty clears it | `BadNote` (line breaks, control characters), `LineTooLong` |
| `add_condition(rule, node)` | appended to the rule's conditions | `TooManyConditions`, `LineTooLong` |
| `remove_condition`, `toggle_not` | a rule without conditions is `always -> ...` | `LineTooLong` (`not`) |
| `set_compare(cond, op, threshold)` | `hull < 30%`: a number of the condition's quantity, at least 0 | `NotAValue` (a flag), `BadValue` |
| `replace_node(at, node)` | another node of the same role (see below) | `WrongRole` |
| `set_param(at, k, value)` | checked like content (`check_param_value`: type, unit, range; NaN refused) | `NoSuchParam`, `BadValue` |
| `reset_param(at, k)` | back to the default (see below) | `NoDefault` (a required parameter) |
| `clear_target()` | removes the `target` line: back to the implicit `nearest` | |

Every operation also refuses stale ids (`NoSuchRule`, `NoSuchCondition`) and unknown node ids (`UnknownNode`).

### What the editor writes

- **New nodes leave optional values out**: the content default applies (and follows the content if it changes). A
  value set in the inspector is written, even when it equals the default. Values are positional, so setting a later
  value writes the earlier defaults out (`orbit` → `orbit(60%, cw)`). `reset_param` writes the default in place and
  then drops trailing values that equal their defaults, so resetting the last written value removes it.
- **Required parameters** (none in the base content) start at the first option of a choice, else at `min` (or 0) in
  the parameter's unit.
- **A new value condition** is compared with `<` and the example of its quantity: `hull < 30%`,
  `enemies_near < 2`, `cycle < 5s`.
- **Replacing a node** keeps the leading values that fit the new node's parameters (`orbit(80%, ccw)` → `kite(80%)`)
  and drops the rest. A condition keeps its `not`; a value condition keeps its comparison if the threshold fits its
  quantity, else gets the example threshold with the same comparator; a flag drops the comparison. The target line
  keeps its note.
- **The default target stays implicit**: there is no `target` line until the selector, its aim point or its note is
  set (replacing `nearest` with `nearest` writes nothing).

### Text view

- `check_text(text)` parses and checks a text without changing anything: every syntax and vocabulary error with its
  line and column (the live errors). `set_text(text)` replaces the program as one undo step, or is refused with the
  same errors. `text()` is the canonical form.
- Ids survive text edits where they can: the leading and trailing rules equal to the old ones keep theirs, rules
  edited in place between them keep theirs by position, added rules get new ids; conditions likewise within a rule.
  A line moved by cut and paste keeps the id of the position, not of the rule.

### Rule slots

`active_rules(slots)` says how many rules run with `slots` rule slots (GDR-0035; for a design,
`Content::ship_rule_slots`): rules at positions below it are active, the rest are shown below the line, inactive. It
never changes the program; moving a rule up is how the player keeps it.

### Queries for the screen

- `rules()`, `rule(id)`, `rule_at(index)`, `rule_of(cond)`; `RuleView::nodes()` lists a rule's nodes in reading
  order (conditions, then the action).
- `node(at)` gives a `NodeView`: the vocabulary entry (role, display name, summary, tags, value quantity), `is_rhai()`,
  `not`, `compare`, `written` (false for the implicit target), and a `ParamView` per parameter: the value in effect,
  whether it is written and whether it equals the default, `range()` for the value's unit (0–1000% for a share of
  the weapon reach), and the `NodeParam` (name, type with its choices, `min`, `max`).
- `palette(role)` lists the vocabulary of a role in content order.

### Undo

Undo and redo keep the last 256 steps (`data::design::edit::UNDO_STEPS`, as in the drydock); `begin_stroke` /
`end_stroke` make a slider drag one step. Every change, undo and redo bumps `revision()`.
