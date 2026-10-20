---
title: "Controls and settings"
description: "Every action and its default inputs, what you can rebind in SETTINGS and how, the display settings, the flight menu, and how you fly, fire and switch vessels."
---

# Controls and settings

Everything you do with a key or a mouse button is a named **action**: [[control:thrust_forward]],
[[control:fire_group_2]], [[control:lock_next]], [[control:undo]]. Each action has up to two inputs, and you can change them in SETTINGS. The game's tooltips, key
badges and the flight's controls list always show what you have bound, not the defaults.

In flight, CONTROLS in the [flight menu](#the-flight-menu) (Esc), or [[control:controls_help]]
([[control:controls_help.keys]]), lays the controls of where you are over the view: the test range's keys in the
test range, a voyage's on a voyage, the fight viewer's in a replay, each with what you have bound. Press the key
again, or Esc, to close it (back to the menu if you opened it there); the flight goes on underneath.

## Flying

Your ship flies on the keys below; the [flight model](/mechanics/flight) explains what the flight computer does
with them.

- **Turn**: [[control:turn_left.keys]] and [[control:turn_right.keys]] rotate the ship. A tap nudges the nose, a
  hold turns faster. Under a lock they nudge the aim instead, and the aim springs back when you let go.
- **Thrust**: [[control:thrust_forward.keys]] and [[control:thrust_back.keys]] push along the nose,
  [[control:strafe_left.keys]] and [[control:strafe_right.keys]] sideways. While you hold them, the ship flies
  where it points: the flight computer cancels sideways drift.
- **Drift**: [[control:drift.keys]] turns drift on and off. With drift on, every key is pure thrust and your
  momentum is kept. The HUD shows DRIFT next to your speed while it is on.
- **Letting go** of every key coasts with the nose kept, drift or not. Nothing brakes for you unless a
  [pilot assist](/behavior/pilot-assists) does.

### Locking a target

- **Click a ship** (or a broken-off piece) to lock it. The ship then turns its strongest guns onto it, leading a
  moving target with projectile guns.
- [[control:lock_next]] ([[control:lock_next.keys]]) locks the nearest enemy. Pressed again while one is locked,
  it moves on to the next one by distance, and after the farthest back to the nearest.
- **Click empty space**, or press Esc, to unlock.
- The [[behavior_node:auto_lock]] [pilot assist](/behavior/pilot-assists) locks an enemy by itself while you have
  nothing locked. Its lock is your own, so all of the above works on it; after you unlock, it waits until you lock
  something again or switch it.

A lock is dropped when its target goes dark or is destroyed, and when you switch to another vessel.

### Weapon groups and firing

Every gun belongs to a weapon group, set with the GROUP pills in the drydock's inspector. A gun without one is in
group 1, so a fresh design fires everything with one trigger.

- [[control:fire]] ([[control:fire.keys]]) fires every gun while held, whatever its group.
- [[control:fire_group_1]], [[control:fire_group_2]] and [[control:fire_group_3]] ([[control:fire_group_1.keys]],
  [[control:fire_group_2.keys]], [[control:fire_group_3.keys]]) fire one group each.
- **Holding the left button on empty space** fires group 1. A short click there unlocks instead; a click on a ship
  locks it and never fires.

Triggers add up: hold two and both groups fire. A tap shorter than one sim tick still fires once.

### Scanning

[[control:scan]] ([[control:scan.keys]]) starts a scan with the [[part:scanner]] of the vessel you fly, as does
the SCAN pill under the target panel, which also says what the scanner is doing (ready, charging, cooling down).
A vessel without a working scanner can't scan; [scanning](/mechanics/galaxy#inside-a-cluster-sensors-and-scans)
explains what a sweep finds.

### Flares

[[control:flare]] ([[control:flare.keys]]) releases flares from the [[part:flare_launcher]] of the vessel you fly:
decoys that pull enemy homing missiles off it. A launcher has a few releases per fight and cools down after each;
[flares](/mechanics/weapons#flares) explains them.

### The minimap and the cluster map

On a voyage the **minimap** sits in the bottom right corner: north is up and your ship is in the middle. It shows
only what your fleet knows: ships within your sensors (enemies coral, dark ships grey, wrecks gold), the sites you
have found, your scans' bearings and sweeps, the rock you have come near, the cluster's edge and your fleet in teal.
[[control:minimap]] ([[control:minimap.keys]]) widens it from 1 km round you to 2.5 km, then hides it, then brings it
back. Hold [[control:cluster_map]] ([[control:cluster_map.keys]]) to lay the whole cluster over your view, drawn the
same way; it goes when you let go. A click on the minimap opens the cluster map until you close it with Esc, a click or
the same key; the minimap's tooltip names the key.

### Switching vessels

You fly one vessel at a time, the focus. [[control:focus_next]] and [[control:focus_previous]]
([[control:focus_next.keys]], [[control:focus_previous.keys]]) step through your vessels. Clicking one of your
vessels in space, or its chip in the fleet bar, flies it at once. Clicking a dormant vessel's chip wakes it, and
your newest other vessel goes dormant in its place.

The vessel you leave flies on under its behavior. If the vessel you fly loses its last logic core, you move on to
your best other vessel by yourself.

### The camera

The camera follows your ship. The wheel zooms (so do [[control:zoom_in.keys]] and [[control:zoom_out.keys]]);
dragging with the middle button pans and frees the camera; [[control:camera_follow.keys]] switches between follow
and free. While the camera is panned away, **BACK TO SHIP** shows at the bottom of the view: click it (or press
[[control:camera_follow.keys]]) and the camera follows your ship again. While a target is locked, the camera zooms out enough to keep it in view, and it widens a little as you
go faster.

## Touch

The touch controls appear after your first touch.

- **The stick**, bottom left: "fly that way". The ship turns onto the stick and flies that way; how far you push
  sets the speed. Under a lock the stick strafes instead.
- **FIRE**, bottom right: fires every weapon group.
- **FLARE**, left of FIRE, on a vessel with a [[part:flare_launcher]]: releases flares.
- **BLINK**, above FIRE, on a vessel with a [[part:blink_drive]]: blinks along the stick while your other thumb pushes
  it, else straight ahead ([blinks](/mechanics/phase-walkers#the-blink-drive)).
- **Tap** a ship to lock it, one of your vessels to fly it, empty space to unlock.
- **Two fingers** zoom (and pan a free camera).

## What can't be rebound

A few inputs are fixed:

- **Esc** goes back or closes. In flight it closes the controls list, the flight menu or a cluster map opened from the minimap if one is open; otherwise it unlocks your target;
  with nothing locked it opens the flight menu. While the ABANDON VOYAGE? panel is open, Esc closes the panel.
- **Enter** confirms.
- **The left mouse button** is the pointer: it clicks, locks, selects and drags. It can't be bound to an action.
- **Middle-drag** pans and **the wheel** zooms.
- **Text fields** and the clipboard keep their usual keys.
- **Touch** works as described above.

## Rebinding in SETTINGS

Open SETTINGS from the shipyard (the gear ring top right, or [[control:open_settings.keys]]) or in flight from
the flight menu. The CONTROLS tab lists every action by section, with two input chips each.

- **Click a chip**, then press a key or a mouse button. Hold Ctrl, Shift or Alt with it for a combination. A
  modifier on its own binds when you release it. Esc cancels; a left click elsewhere cancels too.
- **CLEAR** empties the chip you picked. **RESET TO DEFAULTS** brings every control back.
- **Conflicts are settled per screen.** Two actions that work on the same screen can't share an input. If you bind
  an input another such action had, it moves over, and the status line says which action lost it. Actions on
  different screens may share keys: the same key can turn your ship in flight and open the drydock at the shipyard.
- **Modifiers**: an input without Ctrl or Alt never fires while Ctrl or Alt is held, so the editors' Ctrl
  shortcuts stay apart. Shift is ignored unless the same key is also bound with Shift on that screen: thrust keeps
  working while you hold Shift, but [[control:focus_next.keys]] and [[control:focus_previous.keys]] stay apart.

Every change is saved at once, on this device.

### DISPLAY

The DISPLAY tab holds the look:

- **GLOW**: the soft glow around ships, lights and icons, on or off.
- **BLOOM**: a full-screen bloom over the bright parts, on or off. It costs more to draw.
- **UI SIZE**: the screens' size as a share of what fits the window: 100 %, 90 % or 80 %.

Changes show at once and are saved on this device.

## The flight menu

Press Esc in flight with nothing locked. In the test range the menu pauses the flight (PAUSED); on a voyage the
flight goes on behind it (MENU). It offers RESUME, CONTROLS (the list of the controls here), SETTINGS, and SHIPYARD in the test range or
ABANDON VOYAGE… on a voyage. Esc or RESUME closes it.

## Every action

The tables below come from the game's own action list, with their default inputs.

### Flight

<ControlsTable section="flight" />

The assist toggles switch the [pilot assists](/behavior/pilot-assists) of the ship you fly, for the mission:
[[control:assist_toggle_1.keys]] to [[control:assist_toggle_4.keys]] each switch the assists placed on that
toggle, [[control:all_assists.keys]] switches all of them off and back on.

### Missions

<ControlsTable section="mission" />

### Camera and views

<ControlsTable section="camera" />

### The shipyard

<ControlsTable section="shipyard" />

### The drydock and the behavior editor

<ControlsTable section="drydock" />

### The voyage map

<ControlsTable section="map" />

### Debug and display

<ControlsTable section="debug" />
