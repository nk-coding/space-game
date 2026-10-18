//! The running game (ADR-0038): the sim inside Bevy, the world it draws, flight and voyages, and what the player
//! owns. Built on `kit`; the screens (`game`) build on it. What two screens share lives here.
//!
//! - `mode`: the app mode (home, flight, drydock, the BEHAVIOR screen, research, the map) and what runs in which.
//! - `career`: the player's career save (`saves/<slot>`), a new game, what the career unlocks for the drydock.
//! - `evolution`: the evolution lab's runs (`lab/<id>`): made from the setup, computed in the background within a
//!   budget per frame (paused in flight), saved after every generation; SAVE AS DESIGN (GDR-0092).
//! - `player`: what the player owns: the design every flight flies, the player's behaviors, the saved designs.
//! - `sim_bridge`: owns the `sim::Sim`, steps it at 30 Hz, turns events into view changes and effects.
//! - `scene`: sandbox (player + raider waves; home's TEST RANGE), test flight, showcase, load test; writes the
//!   player's pilot input.
//! - `replay`: the fight viewer (`?scene=fight&fight=<code>`): an arena fight replayed from its code, its playback
//!   and the check against its record (GDR-0092).
//! - `test_range`: TEST IN RANGE for behaviors (watch, fly alongside), its request and the live behavior view.
//! - `voyage`: launching a voyage (the career's seed and save), its run (recall, losing it), its end and its
//!   debrief's data; `warp_fx`: the voyage's transitions (the warp-out, the arrival, the dip to dark and the lift
//!   at the next screen).
//! - `storage`: the player's documents (saves, designs, behaviors, settings), per platform.
//! - `settings`: the action map's bindings (every screen reads its inputs through `settings::Act`) and the display
//!   settings, saved on this device (GDR-0090).
//! - `ship_model` / `ship_view`: content + sim piece → style-2a meshes, posed from `render_pose` every frame;
//!   `part_icons`: part and ship icons in the UI, drawn the same way; `part_card`: a part's information, its stat
//!   block and the hover card beside a part list (the drydock's palette, the workshop).
//! - `fx`: immediate-mode sprites (plumes, trails, beams, tracers, sparks, flashes, blasts).
//! - `shield`: shield bubbles (one batched mesh, each quad at its ship's depth against the opaque ship tiles,
//!   procedural shader `shield.wesl`).
//! - `controls`, `camera`: keyboard / mouse / touch input, follow / frame / free camera.
//! - `flight_hud`, `fleet_bar`, `hud`: flight HUD, fleet bar (the player's vessels), debug overlay and
//!   toggles.
//! - `backdrop`: gradient, parallax dust, range rings.
//! - `scenery`: what fills a cluster between its sites (GDR-0104): the sim's asteroid rocks, glints, drifting debris
//!   and derelicts, landmarks far off, a hot star's corona.

// The screens read the arena's runs and fights (the evolution lab's pages) through the world, which holds them.
pub use arena;

pub mod backdrop;
pub mod camera;
pub mod career;
pub mod controls;
pub mod evolution;
pub mod fleet_bar;
pub mod flight_hud;
pub mod fx;
pub mod hud;
pub mod mode;
pub mod part_card;
pub mod part_icons;
pub mod player;
pub mod replay;
pub mod scene;
pub mod scenery;
pub mod settings;
pub mod shield;
pub mod ship_model;
pub mod ship_view;
pub mod sim_bridge;
pub mod storage;
pub mod test_range;
pub mod voyage;
pub mod warp_fx;
