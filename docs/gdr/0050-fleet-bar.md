# GDR-0050: The fleet bar: a strip at the top centre, shown with two or more vessels or a dormant one; a tap focuses, a tap on a dormant vessel wakes it

- Status: Accepted (provisional, lead)
- Date: 2026-10-12
- Decided by: lead session (default for Niklas to review); look and details by the fleet-bar agent
- Related: [GDR-0013](0013-vessels-and-piloting.md) (focus, fleet bar, camera glide), [GDR-0044](0044-control-cost-priority-idle-dormant-newest.md) (dormant vessels, swap in the fleet bar), [GDR-0008](0008-ui-ship-mind-interface.md) (look), [GDR-0038](0038-logic-cores-control-parts.md) (control capacity), `docs/client.md` "Fleet bar", `docs/style-guide.md` "UI"

## Context

GDR-0013 and GDR-0044 name a fleet bar: the player switches focus there and swaps dormant vessels in. They leave
open what it shows, where it sits, when it appears and what a tap does. The sim exposes every vessel of an owner
with its state (running, dark, dormant), focus, logic cores and control load and capacity, plus the mind load
against the mind capacity (`docs/sim.md` "Vessels"). The flight HUD already uses the top left (hull), top right
(target), left (speed), bottom corners (touch stick and fire button) and the bottom centre (warnings, help).

## Options considered

Where:
- **Top centre, under the scene title** (chosen): free on desktop and phone, reachable by either thumb, reads as
  part of the ship-mind's overview.
- **Top left under the hull block** (where the mockup has the supplies): collides with the speed block on a
  390 px tall phone and with the supplies row later.
- **Bottom centre**: shares the space with the warnings and the help line; between the thumbs on a phone.

When:
- **Only with two or more vessels, or any dormant one** (chosen, as asked): single-ship play stays clean.
- **Always**: one chip for one ship adds nothing.

A tap on a dormant vessel:
- **Wakes it** (chosen): `ShipCommand::Wake`, the swap of GDR-0044; a second tap focuses it.
- **Wakes and focuses it**: one tap, but a swap would always yank the camera.

Hotkeys:
- **None beyond V / Shift+V** (chosen): 1/2/3 are the render toggles; other number schemes can come with a
  key-binding screen.

## Decision

- A strip at the top centre, under the scene title, in the floating-HUD look: no panel, thin lines, Barlow.
- A **mind row**: `MIND 8 / 24` (logic cores running against the mind capacity) and a thin violet bar.
- One **chip** per vessel, in ship-id order (the V order), numbered from 1: one violet pip per logic core, a thin
  bar of its control load against its capacity (teal; gold above 85 %; coral while parts idle), and a state word:
  `PILOT` (the focus, teal corner brackets), `AUTO` (runs its behavior), `DARK` (cores unpowered or overheated,
  coral), `DORMANT` (over the mind capacity, dimmed). Chips are 68 × 50 px (touch targets above 44 px).
- Shown while the player flies a vessel and has two or more vessels or any dormant one; hidden with the HUD (H).
- A click or tap on a running or dark vessel focuses it; on a dormant one wakes it (another goes dormant); on the
  focus does nothing. Clicks on the bar never lock or fire in the world.
- More vessels than fit between the side panels: a window around the focus and `+N`.

## Consequences

- The focus switch through the bar is a sim command like V and clicks; the camera glides over (`docs/client.md`).
- `?mind=N` and a tuning slider set the player's mind capacity, so dormant vessels and the swap can be tried with
  `?fleet=3&mind=4`.
- Open: a long-press or hover detail per vessel (behavior, hull, idle parts); whether a dormant tap should also
  focus; placement once the HUD becomes customisable (GDR-0008).
