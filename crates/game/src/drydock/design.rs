//! The player's design: the blueprint the drydock edits and every flight of the player flies (the sandbox
//! runner, its `?fleet=` wingmen, re-forms and test flights). Raiders stay content ships.

use bevy::prelude::*;
use data::{Blueprint, Content};
use sim::ShipDesign;

use crate::sim_bridge::GameContent;
use crate::ui_kit::Tone;

use super::state::DrydockStatus;

/// The id the player's design flies under. Content ids are snake_case (validation), so a `:` never clashes.
pub const PLAYER_DESIGN_ID: &str = "player:design";
/// The content ship the first design copies.
const STARTER_SHIP: &str = "halvex_runner";
/// The drydock frame until frames are content (frigate 24×24, cruiser 40×40, capital 64×64).
const PLACEHOLDER_FRAME: (u32, u32) = (24, 24);

#[derive(Resource, Clone, Debug)]
pub struct PlayerDesign {
    /// The design in frame coordinates (cells from the frame's top-left corner, y down). `blueprint.size` is
    /// the frame (the schema's "drydock frame"); keep it equal to [`PlayerDesign::frame`].
    pub blueprint: Blueprint,
    /// The drydock frame in cells, `(w, h)`. The one place the frame lives until frames are content.
    pub frame: (u32, u32),
    /// Bumped by every edit (`data::edit` on the editing branch). The drydock view, the design check and the
    /// stats rebuild when it changes.
    pub revision: u64,
}

impl PlayerDesign {
    /// A copy of the starter ship, centred in the placeholder frame.
    pub fn starter(content: &Content) -> Self {
        let frame = PLACEHOLDER_FRAME;
        let mut blueprint = content.ships.get(STARTER_SHIP).cloned().unwrap_or_else(|| Blueprint {
            id: String::new(),
            name: "New design".into(),
            summary: String::new(),
            description: None,
            lore: None,
            tags: Vec::new(),
            faction: content.factions.iter().next().map(|f| f.id.clone()).unwrap_or_default(),
            size: frame,
            parts: Vec::new(),
            behavior: None,
            source: Default::default(),
        });
        if let Some(b) = content.ship_grid(&blueprint).ok().and_then(|g| g.bounds()) {
            let dx = (frame.0 as i32 - b.width()) / 2 - b.min_x;
            let dy = (frame.1 as i32 - b.height()) / 2 - b.min_y;
            for p in &mut blueprint.parts {
                p.x += dx;
                p.y += dy;
            }
        }
        blueprint.id = PLAYER_DESIGN_ID.into();
        blueprint.size = frame;
        Self { blueprint, frame, revision: 0 }
    }

    /// Why the design can't fly, or `None` when it can: it needs parts, a logic core on its main piece (the
    /// piece a ship is flown from) and must compile. A placeholder check until `sim::report` lists the issues.
    pub fn grounded(&self, content: &Content) -> Option<String> {
        let bp = &self.blueprint;
        if bp.parts.is_empty() {
            return Some("empty frame".into());
        }
        let grid = match content.ship_grid(bp) {
            Ok(g) => g,
            Err(e) => return Some(e),
        };
        let core = |i: usize| content.parts.get(&bp.parts[i].part).is_some_and(|p| p.logic.is_some());
        let pieces = grid.pieces();
        let main = grid.main_piece(&pieces, core);
        if !main.is_some_and(|m| pieces.parts[m].iter().any(|&i| core(i))) {
            return Some("no logic core on the main piece".into());
        }
        ShipDesign::compile(content, bp).err()
    }
}

/// The status line and the TEST-FLY switch, whenever the design's revision changes. A placeholder until
/// `sim::report` lists the design's issues (heat, power, arcs) here too.
pub(super) fn check_design(design: Res<PlayerDesign>, content: Res<GameContent>, mut status: ResMut<DrydockStatus>) {
    if status.revision == Some(design.revision) {
        return;
    }
    status.revision = Some(design.revision);
    status.grounded = design.grounded(&content.0);
    (status.line, status.tone) = match &status.grounded {
        None => ("FLIGHT-READY".into(), Tone::Teal),
        Some(why) => (format!("CAN'T FLY  ·  {}", why.to_uppercase()), Tone::Coral),
    };
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_is_centred_in_the_frame_and_can_fly() {
        let content = data::base_content();
        let d = PlayerDesign::starter(&content);
        assert_eq!(d.blueprint.id, PLAYER_DESIGN_ID);
        assert!(!content.ships.contains(PLAYER_DESIGN_ID));
        assert_eq!(d.blueprint.size, d.frame);
        let b = content.ship_grid(&d.blueprint).unwrap().bounds().unwrap();
        // The runner is 12 wide and symmetric: its axis is the frame's centre line.
        assert_eq!(b.min_x + b.max_x, d.frame.0 as i32);
        assert!(b.min_y >= 0 && b.max_y <= d.frame.1 as i32);
        assert_eq!(d.grounded(&content), None);
    }

    #[test]
    fn a_design_without_a_core_is_grounded() {
        let content = data::base_content();
        let mut d = PlayerDesign::starter(&content);
        d.blueprint.parts.retain(|p| content.parts.get(&p.part).is_some_and(|p| p.logic.is_none()));
        assert_eq!(d.grounded(&content).as_deref(), Some("no logic core on the main piece"));
        d.blueprint.parts.clear();
        assert_eq!(d.grounded(&content).as_deref(), Some("empty frame"));
    }
}
