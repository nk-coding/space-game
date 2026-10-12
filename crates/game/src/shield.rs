//! Shield bubbles (GDR-0043): one glowing energy bubble per powered emitter (GDR-0021). A translucent fill that
//! brightens towards a soft-glowing rim, a slow shimmer, a ripple from every hit, flicker when low, and a
//! shatter when the bubble collapses. Drawn procedurally by `shield.wesl` (no textures).
//!
//! Batching (ADR-0029): every bubble of every ship is a quad in a world-space mesh with **one** material: two
//! meshes in all, one per layer, so all bubbles are two draw calls. The focused vessel's bubbles draw above it
//! ([`Z_FOCUS_BUBBLES`]), every other bubble on one layer above every other ship and below the focus
//! ([`Z_BUBBLES`]): the focus flying over another ship's bubble stays on top of it. The meshes are rewritten in
//! place every frame (like `fx.rs`); each quad carries its bubble's state in vertex attributes (offset from the
//! centre, radius, seed, charge, presence, shatter progress, up to [`MAX_HITS`] hits). The material only holds
//! the style tokens and never changes; the shader animates from `globals.time`. The bubbles blend with `max`
//! (colour and alpha), so overlapping bubbles of one ship never add up to bright lenses, and drawing order inside
//! a layer does not matter.
//!
//! State lives here, keyed by `(ShipId, emitter part)`, so a piece view rebuilt in the middle of a ripple or a
//! shatter (a part destroyed by the same shot) keeps playing it. Hits arrive from `sim_bridge::apply_events`.

use std::collections::BTreeMap;

use bevy::asset::{AssetPath, RenderAssetUsages, embedded_asset, embedded_path};
use bevy::camera::visibility::NoFrustumCulling;
use bevy::mesh::{
    Indices, MeshVertexAttribute, MeshVertexBufferLayoutRef, PrimitiveTopology, VertexAttributeValues, VertexFormat,
};
use bevy::prelude::*;
use bevy::render::render_resource::{
    AsBindGroup, BlendComponent, BlendFactor, BlendOperation, BlendState, RenderPipelineDescriptor, ShaderType,
    SpecializedMeshPipelineError,
};
use bevy::shader::ShaderRef;
use bevy::sprite_render::{AlphaMode2d, Material2d, Material2dKey, Material2dPipeline, Material2dPlugin};
use sim::{PartIdx, ShipId, Stat};

use crate::RenderToggles;
use crate::scene::PilotFocus;
use crate::ship_view::PieceView;
use crate::sim_bridge::SimWorld;
use crate::style::{ShieldStyle, Style};

/// Every bubble but the focus's: above every other ship (z 10..89), below the focus (`ship_view::Z_FOCUS`).
const Z_BUBBLES: f32 = 95.0;
/// The focused vessel's bubbles: above it, below weapons and sparks (`fx::Z_WEAPONS`).
const Z_FOCUS_BUBBLES: f32 = 200.0;
/// The layers: others, focus.
const LAYERS: usize = 2;
/// Hits a bubble shows at once; a new hit replaces the oldest.
pub const MAX_HITS: usize = 4;
/// A hit this close (seconds, angle) to a younger one only strengthens it: a beam hits every tick.
const MERGE_SECONDS: f32 = 0.16;
const MERGE_COS: f32 = 0.94;
/// Seconds to rise (after a restart) and to fade (power lost, emitter destroyed, piece dark).
const RISE_SECONDS: f32 = 0.35;
const FADE_SECONDS: f32 = 0.12;
/// How far the shards fly out during the shatter, as a share of the radius (see `shield.wesl`).
const SHATTER_REACH: f32 = 0.7;

/// Per bubble: `(offset from the centre x, y in metres, radius in metres, seed 0..1)`.
const ATTR_BUBBLE: MeshVertexAttribute =
    MeshVertexAttribute::new("Bubble", 0x5348_4c44_0001, VertexFormat::Float32x4);
/// Per bubble: `(charge 0..1, presence 0..1, shatter progress 0..1 (0 = none), 0)`.
const ATTR_STATE: MeshVertexAttribute =
    MeshVertexAttribute::new("BubbleState", 0x5348_4c44_0002, VertexFormat::Float32x4);
/// Per hit: `(direction x, y (unit, world), age 0..1 (1 = none), strength 0..1)`.
const ATTR_HITS: [MeshVertexAttribute; MAX_HITS] = [
    MeshVertexAttribute::new("BubbleHit0", 0x5348_4c44_0010, VertexFormat::Float32x4),
    MeshVertexAttribute::new("BubbleHit1", 0x5348_4c44_0011, VertexFormat::Float32x4),
    MeshVertexAttribute::new("BubbleHit2", 0x5348_4c44_0012, VertexFormat::Float32x4),
    MeshVertexAttribute::new("BubbleHit3", 0x5348_4c44_0013, VertexFormat::Float32x4),
];

// ---------------------------------------------------------------------------------------------------------
// Material

/// The style tokens, fixed after start-up (colours in linear RGB).
#[derive(ShaderType, Clone, Copy, Debug)]
struct BubbleUniform {
    color: Vec4,
    core: Vec4,
    /// `(fill at the centre, fill at the rim, rim line, outer glow)`.
    opacity: Vec4,
    /// `(rim width, glow width, shimmer depth, shimmer speed)` in metres and metres per second.
    shape: Vec4,
    /// `(low share, 0, 0, 0)`.
    low: Vec4,
}

#[derive(Asset, TypePath, AsBindGroup, Clone, Debug)]
pub struct BubbleMaterial {
    #[uniform(0)]
    style: BubbleUniform,
}

impl BubbleMaterial {
    fn new(s: &ShieldStyle) -> Self {
        let rgb = |c: Color| {
            let l = c.to_linear();
            Vec4::new(l.red, l.green, l.blue, 1.0)
        };
        Self {
            style: BubbleUniform {
                color: rgb(s.color),
                core: rgb(s.core),
                opacity: Vec4::new(s.fill_center_opacity, s.fill_rim_opacity, s.rim_opacity, s.glow_opacity),
                shape: Vec4::new(s.rim_width, s.glow_width, s.shimmer, s.shimmer_speed),
                low: Vec4::new(s.low_share, 0.0, 0.0, 0.0),
            },
        }
    }
}

fn shader() -> ShaderRef {
    ShaderRef::Path(AssetPath::from_path_buf(embedded_path!("shield.wesl")).with_source("embedded"))
}

impl Material2d for BubbleMaterial {
    fn vertex_shader() -> ShaderRef {
        shader()
    }

    fn fragment_shader() -> ShaderRef {
        shader()
    }

    fn alpha_mode(&self) -> AlphaMode2d {
        AlphaMode2d::Blend
    }

    fn specialize(
        _pipeline: &Material2dPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        layout: &MeshVertexBufferLayoutRef,
        _key: Material2dKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        let mut attrs = vec![
            Mesh::ATTRIBUTE_POSITION.at_shader_location(0),
            ATTR_BUBBLE.at_shader_location(1),
            ATTR_STATE.at_shader_location(2),
        ];
        attrs.extend(ATTR_HITS.iter().enumerate().map(|(k, a)| a.at_shader_location(3 + k as u32)));
        descriptor.vertex.buffers = vec![layout.0.get_layout(&attrs)?];
        // Max blending: the brighter of two overlapping bubbles wins, nothing doubles up (WebGL2 has MIN/MAX).
        let max = BlendComponent {
            src_factor: BlendFactor::One,
            dst_factor: BlendFactor::One,
            operation: BlendOperation::Max,
        };
        let target = descriptor.fragment.as_mut().and_then(|f| f.targets.first_mut()).and_then(Option::as_mut);
        if let Some(target) = target {
            target.blend = Some(BlendState { color: max, alpha: max });
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------------------------
// State

#[derive(Clone, Copy, Debug)]
struct Hit {
    /// From the emitter towards where the shot entered, piece frame (unit).
    dir: Vec2,
    /// Seconds since the hit.
    age: f32,
    /// 0..1.
    power: f32,
}

#[derive(Clone, Debug)]
struct Bubble {
    seed: f32,
    /// Shown charge (eases towards shield / capacity).
    charge: f32,
    /// 0 = down, 1 = up (eases).
    presence: f32,
    hits: [Option<Hit>; MAX_HITS],
    /// Seconds since the bubble collapsed, while the shatter plays.
    shatter: Option<f32>,
    /// Frame this bubble last had a view.
    seen: u32,
}

impl Bubble {
    fn new(ship: ShipId, part: PartIdx) -> Self {
        let h = ship.0.wrapping_mul(0x9E37_79B9) ^ part.wrapping_mul(0x85EB_CA6B);
        let h = (h ^ (h >> 15)).wrapping_mul(0x2C1B_3C6D);
        Self {
            seed: (h >> 8) as f32 / (1u32 << 24) as f32,
            charge: 0.0,
            presence: 0.0,
            hits: [None; MAX_HITS],
            shatter: None,
            seen: 0,
        }
    }
}

/// All shield bubbles on screen and their animations.
#[derive(Resource, Default)]
pub struct ShieldBubbles {
    bubbles: BTreeMap<(ShipId, PartIdx), Bubble>,
    frame: u32,
    /// Per layer: the mesh holds nothing (skip rewriting it while it stays empty).
    empty: [bool; LAYERS],
    scratch: [Scratch; LAYERS],
}

impl ShieldBubbles {
    /// A shot entered the bubble of emitter `part` (`SimEvent::ShieldHit`): a ripple from `dir` (unit, piece frame,
    /// from the emitter), `share` = the share of its capacity the shot took; `depleted` shatters it.
    pub fn hit(&mut self, ship: ShipId, part: PartIdx, dir: Vec2, share: f32, depleted: bool) {
        let b = self.bubbles.entry((ship, part)).or_insert_with(|| Bubble::new(ship, part));
        let power = (0.55 + 2.5 * share).clamp(0.55, 1.0);
        if let Some(h) = b.hits.iter_mut().flatten().find(|h| h.age < MERGE_SECONDS && h.dir.dot(dir) > MERGE_COS) {
            h.power = h.power.max(power);
        } else {
            let age = |k: usize| b.hits[k].map_or(f32::MAX, |h| h.age);
            let slot = (0..MAX_HITS).max_by(|&i, &j| age(i).total_cmp(&age(j))).unwrap_or(0);
            b.hits[slot] = Some(Hit { dir, age: 0.0, power });
        }
        if depleted {
            b.shatter = Some(0.0);
            b.presence = 0.0;
        }
    }

    /// Forgets every bubble (scene reset: ship ids start over).
    pub fn clear(&mut self) {
        self.bubbles.clear();
    }
}

#[derive(Default)]
struct Scratch {
    pos: Vec<[f32; 3]>,
    bubble: Vec<[f32; 4]>,
    state: Vec<[f32; 4]>,
    hits: [Vec<[f32; 4]>; MAX_HITS],
    idx: Vec<u32>,
}

impl Scratch {
    fn clear(&mut self) {
        self.pos.clear();
        self.bubble.clear();
        self.state.clear();
        self.hits.iter_mut().for_each(Vec::clear);
        self.idx.clear();
    }

    /// One quad of half-size `half` around `centre`; every corner carries the bubble's data.
    fn quad(&mut self, centre: Vec2, half: f32, bubble: [f32; 4], state: [f32; 4], hits: [[f32; 4]; MAX_HITS]) {
        let base = self.pos.len() as u32;
        for (sx, sy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
            let off = Vec2::new(sx, sy) * half;
            let p = centre + off;
            self.pos.push([p.x, p.y, 0.0]);
            self.bubble.push([off.x, off.y, bubble[2], bubble[3]]);
            self.state.push(state);
            for (k, h) in hits.iter().enumerate() {
                self.hits[k].push(*h);
            }
        }
        self.idx.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    /// A mesh must keep its layout: an empty frame is one degenerate triangle.
    fn degenerate(&mut self) {
        self.quad(Vec2::ZERO, 0.0, [0.0; 4], [0.0; 4], [[0.0, 0.0, 1.0, 0.0]; MAX_HITS]);
        self.idx.truncate(3);
    }

    /// Copies the buffers into the mesh, reusing its vectors.
    fn write(&self, mesh: &mut Mesh) {
        if let Ok(VertexAttributeValues::Float32x3(v)) = mesh.try_attribute_mut(Mesh::ATTRIBUTE_POSITION) {
            v.clear();
            v.extend_from_slice(&self.pos);
        } else {
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, self.pos.clone());
        }
        put4(mesh, ATTR_BUBBLE, &self.bubble);
        put4(mesh, ATTR_STATE, &self.state);
        for (k, attr) in ATTR_HITS.into_iter().enumerate() {
            put4(mesh, attr, &self.hits[k]);
        }
        match mesh.indices_mut() {
            Some(Indices::U32(v)) => {
                v.clear();
                v.extend_from_slice(&self.idx);
            }
            _ => mesh.insert_indices(Indices::U32(self.idx.clone())),
        }
    }
}

fn put4(mesh: &mut Mesh, attr: MeshVertexAttribute, data: &[[f32; 4]]) {
    if let Ok(VertexAttributeValues::Float32x4(v)) = mesh.try_attribute_mut(attr) {
        v.clear();
        v.extend_from_slice(data);
    } else {
        mesh.insert_attribute(attr, data.to_vec());
    }
}

// ---------------------------------------------------------------------------------------------------------
// Plugin and systems

/// The bubble meshes: others, focus.
#[derive(Resource)]
struct BubbleMeshes([Handle<Mesh>; LAYERS]);

pub struct ShieldPlugin;

impl Plugin for ShieldPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "shield.wesl");
        app.add_plugins(Material2dPlugin::<BubbleMaterial>::default())
            .init_resource::<ShieldBubbles>()
            .add_systems(Startup, setup)
            .add_systems(
                PostUpdate,
                draw_bubbles
                    .after(crate::ship_view::sync_piece_views)
                    .before(bevy::transform::TransformSystems::Propagate),
            );
    }
}

fn setup(
    mut commands: Commands,
    style: Res<Style>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<BubbleMaterial>>,
    mut bubbles: ResMut<ShieldBubbles>,
) {
    let material = materials.add(BubbleMaterial::new(&style.shield));
    let handles = [Z_BUBBLES, Z_FOCUS_BUBBLES].map(|z| {
        let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
        let mut empty = Scratch::default();
        empty.degenerate();
        empty.write(&mut mesh);
        let mesh = meshes.add(mesh);
        commands.spawn((
            Mesh2d(mesh.clone()),
            MeshMaterial2d(material.clone()),
            Transform::from_xyz(0.0, 0.0, z),
            NoFrustumCulling,
        ));
        mesh
    });
    bubbles.empty = [true; LAYERS];
    commands.insert_resource(BubbleMeshes(handles));
}

/// Every frame: follows each emitter's state in the sim, ages the animations and writes the bubble mesh.
#[allow(clippy::too_many_arguments)]
fn draw_bubbles(
    world: Res<SimWorld>,
    focus: Res<PilotFocus>,
    time: Res<Time>,
    toggles: Res<RenderToggles>,
    style: Res<Style>,
    views: Query<(&PieceView, &Transform)>,
    mut bubbles: ResMut<ShieldBubbles>,
    mesh: Res<BubbleMeshes>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let dt = time.delta_secs().min(0.1);
    let s = &style.shield;
    let state = &mut *bubbles;
    state.frame = state.frame.wrapping_add(1);
    let frame = state.frame;
    state.scratch.iter_mut().for_each(Scratch::clear);

    for (view, tf) in &views {
        let Some(ship) = world.sim.ship(view.ship) else { continue };
        let Some(piece) = ship.piece(view.piece) else { continue };
        // The same test as the piece's draw layer (`ship_view::sync_piece_views`).
        let layer = usize::from(focus.0 == Some(view.ship) && piece.controlled);
        let rot = Mat2::from_angle(tf.rotation.to_euler(EulerRot::ZYX).0);
        for p in &view.visual.parts {
            let Some(radius) = p.shield_radius.filter(|r| *r > 0.0) else { continue };
            let st = ship.part(p.index);
            let cap = ship.stats(p.index).get(Stat::ShieldCapacity).max(1e-3);
            // Same test as the sim's bubble list (`weapons::collect_bubbles`).
            let up = piece.active && st.alive && st.work > 0.0 && st.shield > 0.0 && !st.shield_down;
            let b = state.bubbles.entry((view.ship, p.index)).or_insert_with(|| Bubble::new(view.ship, p.index));
            b.seen = frame;
            b.presence =
                if up { (b.presence + dt / RISE_SECONDS).min(1.0) } else { (b.presence - dt / FADE_SECONDS).max(0.0) };
            b.charge += ((st.shield / cap).clamp(0.0, 1.0) - b.charge) * (1.0 - (-dt * 10.0).exp());
            for slot in &mut b.hits {
                if let Some(h) = slot {
                    h.age += dt;
                    if h.age >= s.hit_seconds {
                        *slot = None;
                    }
                }
            }
            if let Some(t) = b.shatter.as_mut() {
                *t += dt;
                if *t >= s.collapse_seconds {
                    b.shatter = None;
                }
            }
            let shatter = b.shatter.map(|t| (t / s.collapse_seconds).max(1e-3)).unwrap_or(0.0);
            if toggles.debug_shapes || (b.presence <= 0.0 && shatter == 0.0) {
                continue;
            }
            // Rising bubbles grow from 85 % of their radius.
            let rise = 1.0 - (1.0 - b.presence).powi(3);
            let r = if shatter > 0.0 { radius } else { radius * (0.85 + 0.15 * rise) };
            let reach = if shatter > 0.0 { r * SHATTER_REACH } else { 0.0 };
            let half = r + reach + 4.0 * s.glow_width + 0.1;
            let centre = tf.transform_point(view.visual.to_local(p.center).extend(0.0)).truncate();
            let mut hits = [[0.0, 0.0, 1.0, 0.0]; MAX_HITS];
            for (k, h) in b.hits.iter().enumerate() {
                if let Some(h) = h {
                    let d = rot * h.dir;
                    hits[k] = [d.x, d.y, (h.age / s.hit_seconds).min(1.0), h.power];
                }
            }
            state.scratch[layer].quad(centre, half, [0.0, 0.0, r, b.seed], [b.charge, b.presence, shatter, 0.0], hits);
        }
    }
    state.bubbles.retain(|_, b| b.seen == frame);

    for (k, handle) in mesh.0.iter().enumerate() {
        let sc = &mut state.scratch[k];
        let empty = sc.idx.is_empty();
        if empty && state.empty[k] {
            continue;
        }
        if empty {
            sc.degenerate();
        }
        state.empty[k] = empty;
        if let Some(mut m) = meshes.get_mut(handle) {
            sc.write(&mut m);
        }
    }
}
