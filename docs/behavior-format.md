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

Guns **fire at will** whatever the action: whenever the target (or, for cannons, the point where the target
will be) is in front of the nose and within reach.

A ship can run only as many rules as its **logic cores** give it rule slots (a logic core gives six, an
automaton core two). Rules below that line are inactive; if a core is shot away or overheats, the lowest rules
switch off first. If no active rule matches, the ship stops and holds.

## One line per rule

```text
target weakest(engines)
when hull < 30% -> flee                                   # run when badly hurt
when shield_down and target_on_me -> orbit(80%, ccw)      # dodge while the shields recharge
when target_in_range(120%) and cycle(9s) >= 5s -> orbit(60%)
always -> approach(67%)
```

| Line | Meaning |
|---|---|
| `target <selector>` | Optional, first line: who the rules are about. Default `target nearest`. |
| `when <condition> and <condition> ... -> <action>` | A rule: all conditions must hold. |
| `always -> <action>` | A rule without conditions (usually the last one). |
| `... # note` | A note at the end of a line. It belongs to the rule and survives the graph view. |

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

`approach(range)`, `orbit(radius, dir)`, `kite(range)`, `face`, `flee`, `hold`. See the vocabulary in the wiki
(or `content/base/behavior_nodes/actions.ron`) for what each does.

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
The printed form is always the canonical one; parsing it again gives the same behavior.

### Errors

Every error names the line and the column and says what was expected, e.g.:

```text
line 4, column 6: unknown condition `sheild_down`; did you mean `shield_down`?
line 2, column 13: write a percentage with `%`, e.g. 30%
line 3, column 17: expected `and` or `->`, found `flee`
line 5, column 29: `sideways` is not an option; choose one of: auto, ccw, cw
```

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
note      = "#" text to the end of the line
name      = letter { letter | digit | "_" }        case-insensitive
```

Keywords (`when if always and or not target`) cannot be node or option names. Limits: 64 rules, 8 conditions
per rule, 4 values per node, 240 characters per line.

## In content files

A behavior in `behaviors/*.ron` holds exactly these lines: the `target` field is the target line without the
keyword, `rules` has one string per rule. No conversion is involved, so the wiki and the editor show what the
file says.

```ron
(
    id: "raider_basic",
    name: "Raider pack",
    summary: "Close in and shoot, then strafe around the target for a few seconds, and again.",
    target: "nearest",
    rules: [
        "when target_in_range(120%) and cycle(9s) >= 5s -> orbit(60%)",
        "always -> approach(67%)",
    ],
)
```

A faction names the default behavior of its ships (`behavior: "raider_basic"`); a ship can name its own. The
behavior a ship flies must fit the rule slots of its logic cores (validation checks it).

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
| `.reach` | weapon reach (m) |
| `target.distance` `target.bearing` `target.closing` `target.aims_at_me` | distance (m), angle off your nose (rad, left positive), closing speed (m/s), whether it aims at you |
| `me.enemies` | number of active enemy ships |
| parameters | by id, as numbers in base units (metres, shares 0..1, seconds) |
| functions | `abs min max clamp floor ceil sqrt sin cos atan2 exp pi to_float to_int` |

Rules for scripts: no `print`, no time, no files, no `import`, no `eval`, no `**` (powers go through the
platform's maths library, which is not bit-identical everywhere; multiply instead). Each run has a budget of
2000 operations; a script that runs out, errors or returns something other than `true`/`false` counts as
**false**, and the debug view shows why. Scripts are checked when the content loads (syntax, unknown variable
names).
