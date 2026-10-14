//! What the toolkit reads of the world camera (ADR-0038): its zoom and where it looks, as [`ViewScale`]. The
//! camera rig (`camera::CameraRig`) writes it every frame in [`ViewSet`], after it has moved; the atlas picks its
//! tier from the zoom and the UI icons are placed through it, so neither needs the rig.

use bevy::prelude::*;

/// The view of the main camera this frame (a resource, written by the camera rig).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewScale {
    /// Logical pixels per metre (current, eased).
    pub zoom: f32,
    /// The zoom it eases towards.
    pub zoom_target: f32,
    /// The world point at the view's centre, and where on screen that centre sits (logical pixels, y down).
    pub pos: Vec2,
    pub center: Vec2,
}

impl ViewScale {
    /// World point under a logical screen position (y down).
    pub fn screen_to_world(&self, screen: Vec2) -> Vec2 {
        let d = screen - self.center;
        self.pos + Vec2::new(d.x, -d.y) / self.zoom
    }
}

/// `PostUpdate`: the camera rig writes [`ViewScale`] here, after it has moved; what places things over the view
/// goes after it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ViewSet;
