//! The sim's asteroid rocks (GDR-0104), drawn as they spawn: the outline filled in the kind's tone (`world.rock.*`),
//! a few facet lines from a point off-centre towards the corners, the edge lit on the side the light comes from.
//! A rock never moves (an anchored body): the rocks that spawn together (a field, a piece of a belt) are one mesh in
//! world coordinates, drawn in one call.

use bevy::prelude::*;
use kit::atlas::Atlas;
use kit::meshbuf::MeshBuf;
use kit::style::vcol;

use super::{Scenery, SceneryPart};
use crate::sim_bridge::{SimWorld, v2};

/// Rocks lie under the trails and every ship (z 5 and up).
const Z_ROCKS: f32 = 3.0;
/// The edge line's and the facets' half widths (m).
const EDGE: f32 = 0.22;
const FACET: f32 = 0.12;
/// Where the light comes from (world, unit): the lit edge faces it.
const LIGHT: Vec2 = Vec2::new(-0.6, 0.8);

/// Draws the rocks spawned since the last frame.
pub(super) fn draw_rocks(
    mut commands: Commands,
    mut state: ResMut<Scenery>,
    world: Res<SimWorld>,
    atlas: Res<Atlas>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let Some(v) = world.sim.voyage().filter(|_| state.key.is_some()) else { return };
    let rocks = v.rocks();
    if state.rocks_drawn >= rocks.len() {
        return;
    }
    let white = atlas.region("fx/white");
    let edge = atlas.token_color("world.rock_edge");
    let facet = atlas.token_color("world.rock_facet");
    let mut m = MeshBuf::default();
    for r in &rocks[state.rocks_drawn..] {
        let tone = world.content.rocks.get(&r.kind).map_or("iron", |k| k.tone.as_str());
        let fill = atlas.token_color(&format!("world.rock.{tone}"));
        let (pos, turn) = (v2(r.pos), Vec2::from_angle(r.angle));
        let pts: Vec<Vec2> = r.points.iter().map(|&p| pos + turn.rotate(v2(p))).collect();
        m.polygon(&pts, white.uv_center(), vcol(fill, 1.0));
        // Facets: from a point off the centre (away from the light) to every other corner.
        let core = pos - LIGHT * r.radius * 0.18;
        for p in pts.iter().step_by(2) {
            m.strip(&[core, core.lerp(*p, 0.92)], false, FACET, &white, vcol(facet, 0.9));
        }
        m.strip(&pts, true, EDGE, &white, vcol(edge, 0.55));
        // The lit edge: the sides facing the light, brighter.
        for k in 0..pts.len() {
            let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
            let out = (b - a).perp() * -1.0;
            if out.normalize_or_zero().dot(LIGHT) > 0.3 {
                m.strip(&[a, b], false, EDGE, &white, vcol(edge.mix(&Color::WHITE, 0.25), 0.8));
            }
        }
    }
    commands.spawn((SceneryPart, Mesh2d(meshes.add(m.into_mesh())), MeshMaterial2d(atlas.material.clone()), Transform::from_xyz(0.0, 0.0, Z_ROCKS)));
    state.rocks_drawn = rocks.len();
}
