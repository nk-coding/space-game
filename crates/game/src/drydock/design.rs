//! The player's design: the blueprint the drydock edits and every flight of the player flies (the sandbox
//! runner, its `?fleet=` wingmen, re-forms and test flights). Raiders stay content ships.
//!
//! The design lives in a [`data::edit::Drydock`] (the editing model: legality, tools, mirror, undo). Every edit
//! goes through [`PlayerDesign::edit`], which keeps the frame-coordinate blueprint the view, the report and the
//! flights read in step and bumps [`PlayerDesign::revision`].

use bevy::prelude::*;
use data::edit::{Drydock, OpenError};
use data::{Blueprint, Content};

/// The id the player's design flies under. Content ids are snake_case (validation), so a `:` never clashes.
pub const PLAYER_DESIGN_ID: &str = "player:design";

#[derive(Resource, Clone, Debug)]
pub struct PlayerDesign {
    /// The editing model, in frame coordinates.
    dock: Drydock,
    /// The design in frame coordinates (cells from the frame's top-left corner, y down), `size` = the frame and
    /// `id` = [`PLAYER_DESIGN_ID`]: what the view draws and flights fly. Rebuilt after every change.
    blueprint: Blueprint,
    /// `dock.revision()` when `blueprint` was built.
    seen: u64,
    /// Goes up with every change, including opening another design; never goes back.
    revision: u64,
}

impl PlayerDesign {
    /// Opens a blueprint in a frame of `frame = (w, h)` cells, centred. The blueprint's header (id, name,
    /// summary, faction, behavior) is kept for [`PlayerDesign::export`].
    pub fn open(content: &Content, blueprint: &Blueprint, frame: (u32, u32)) -> Result<Self, OpenError> {
        let dock = Drydock::open(content, blueprint, frame)?;
        let blueprint = Blueprint { parts: Vec::new(), ..dock.header().clone() };
        let mut design = Self { dock, blueprint, seen: 0, revision: 0 };
        design.rebuild();
        Ok(design)
    }

    /// A copy of the content ship `ship` in the first content frame it fits, mirror on; an empty design when the
    /// content has no such ship. A new game's first design is its start's ship (`career::PlayerCareer::start_ship`).
    pub fn starter(content: &Content, ship: &str) -> Self {
        let empty = || Blueprint {
            id: String::new(),
            name: "New design".into(),
            summary: String::new(),
            description: None,
            lore: None,
            tags: Vec::new(),
            faction: content.factions.iter().next().map(|f| f.id.clone()).unwrap_or_default(),
            size: (0, 0),
            parts: Vec::new(),
            behavior: None,
            source: Default::default(),
        };
        let ship = content.ships.get(ship).cloned().unwrap_or_else(empty);
        let frames = content.frames.iter().map(|f| f.size).chain([ship.size]);
        let mut design = frames
            .filter_map(|frame| Self::open(content, &ship, frame).ok())
            .next()
            .unwrap_or_else(|| Self::open(content, &empty(), (24, 24)).expect("an empty design opens in any frame"));
        design.dock.mirror = true;
        design
    }

    /// Replaces this design with another one (opened, loaded or a new one); the revision keeps going up and the
    /// mirror and bevel modes stay as they are (they are the drydock's, not the design's).
    pub fn replace(&mut self, mut next: PlayerDesign) {
        next.revision = self.revision + 1;
        next.dock.mirror = self.dock.mirror;
        next.dock.bevel = self.dock.bevel;
        *self = next;
    }

    /// The design in frame coordinates, `size` = the frame, `id` = [`PLAYER_DESIGN_ID`].
    pub fn blueprint(&self) -> &Blueprint {
        &self.blueprint
    }

    /// The editing model, read-only. Edit through [`PlayerDesign::edit`].
    pub fn dock(&self) -> &Drydock {
        &self.dock
    }

    /// Runs an edit on the editing model, then brings the blueprint and the revision up to date when the
    /// design changed. Mode switches (`mirror`, `bevel`) change nothing.
    pub fn edit<R>(&mut self, f: impl FnOnce(&mut Drydock) -> R) -> R {
        let r = f(&mut self.dock);
        if self.dock.revision() != self.seen {
            self.rebuild();
        }
        r
    }

    /// Renames or re-describes the design (name, summary, description, tags, faction, behavior): not an undo
    /// step, but the revision goes up.
    pub fn edit_header(&mut self, f: impl FnOnce(&mut Blueprint)) {
        f(self.dock.header_mut());
        self.rebuild();
    }

    /// The frame `(w, h)` in cells.
    pub fn frame(&self) -> (u32, u32) {
        self.dock.frame()
    }

    /// Bumped by every change; the drydock view, the checks and the stats rebuild when it changes.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// The design as a blueprint to save or share: trimmed to its parts, the header as opened (or renamed).
    pub fn export(&self) -> Blueprint {
        self.dock.export()
    }

    fn rebuild(&mut self) {
        self.seen = self.dock.revision();
        self.revision += 1;
        self.blueprint = Blueprint {
            id: PLAYER_DESIGN_ID.into(),
            size: self.dock.frame(),
            parts: self.dock.parts().to_vec(),
            ..self.dock.header().clone()
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starter_is_centred_in_the_frame_and_can_fly() {
        let content = data::base_content();
        for ship in ["rim_skiff", "halvex_runner"] {
            let d = PlayerDesign::starter(&content, ship);
            assert_eq!(d.blueprint().id, PLAYER_DESIGN_ID);
            assert!(!content.ships.contains(PLAYER_DESIGN_ID));
            assert_eq!(d.blueprint().size, d.frame());
            assert_eq!(d.frame(), content.frames.iter().next().unwrap().size);
            let b = content.ship_grid(d.blueprint()).unwrap().bounds().unwrap();
            // Both are symmetric: the axis is the frame's centre line.
            assert_eq!(b.min_x + b.max_x, d.frame().0 as i32, "{ship}");
            assert!(b.min_y >= 0 && b.max_y <= d.frame().1 as i32);
            assert!(sim::design_report(&content, d.blueprint(), &sim::SimConfig::default()).unwrap().flight_ready());
            // Exporting gives the content ship back (opening a design drops where it was loaded from).
            let bp = content.ships.get(ship).unwrap();
            assert_eq!(d.export(), Blueprint { source: Default::default(), ..bp.clone() });
        }
        // A ship the content lacks: an empty design.
        assert!(PlayerDesign::starter(&content, "nowhere").blueprint().parts.is_empty());
    }

    #[test]
    fn edits_bump_the_revision_and_reach_the_blueprint() {
        let content = data::base_content();
        let mut d = PlayerDesign::starter(&content, "halvex_runner");
        let (r0, n0) = (d.revision(), d.blueprint().parts.len());
        let out = d.edit(|dock| dock.paint((0, 0), "steel"));
        assert!(out.unwrap().changed);
        // Mirror is on: the block and its twin.
        assert_eq!(d.blueprint().parts.len(), n0 + 2);
        assert!(d.revision() > r0);
        let r1 = d.revision();
        d.edit(|dock| dock.undo());
        assert_eq!(d.blueprint().parts.len(), n0);
        assert!(d.revision() > r1);
        // A no-op leaves the revision alone.
        let r2 = d.revision();
        d.edit(|_| ());
        assert_eq!(d.revision(), r2);
        // Renaming reaches the blueprint and the export.
        d.edit_header(|h| h.name = "Renamed".into());
        assert_eq!(d.blueprint().name, "Renamed");
        assert_eq!(d.export().name, "Renamed");
        assert!(d.revision() > r2);
        // Replacing keeps the revision going up.
        let r3 = d.revision();
        let next = PlayerDesign::starter(&content, "halvex_runner");
        d.replace(next);
        assert!(d.revision() > r3);
    }
}
