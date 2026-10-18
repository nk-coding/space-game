//! What fills a cluster between its sites (GDR-0104): the kinds of asteroid rock (`rocks.ron`), the landmarks seen
//! far off (`landmarks.ron`), and a cluster type's or a voyage's [`Scenery`], which places them and sets how thick
//! the dust, debris and drifting derelicts are and how often a fight flashes in the distance.
//!
//! Rocks are real bodies in the simulation (`sim::voyage::rocks`): they block ships, shots and plumes. Everything
//! else here is drawn by the client only, seeded per cluster, and never touches the simulation.

use super::*;

/// `rocks.ron`: a kind of asteroid rock. Each rock of a field is rolled from it: a convex hull of `corners`
/// corners round a `size` radius, pulled in by up to `roughness` of it and stretched along one axis by `stretch`, then
/// dented along its edges by up to `dent` (the outline the sim collides and the client draws, `sim::voyage::rocks`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RockKind {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lore: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// The material it is made of (a material id): what mining it will give (GDR-0031; no mining yet).
    pub material: String,
    /// Radius `(min, max)` in metres, rolled per rock.
    pub size: (f32, f32),
    /// Corners of its outline `(min, max)`, rolled per rock.
    pub corners: (u32, u32),
    /// How far a corner may sit inside the radius, as a share of it (0: a regular outline).
    pub roughness: f32,
    /// Stretch along its long axis `(min, max)` (1: round), rolled per rock.
    pub stretch: (f32, f32),
    /// How far the middle of an edge may dent in, as a share of its distance from the centre (0: no dents). A rock
    /// dented deeply enough collides as two or three convex pieces.
    pub dent: f32,
    /// The share of a ram's damage (GDR-0047) a ship takes against it: an anchored rock gives nothing, so a ram on
    /// it would hurt as much as one on a ship of infinite mass; like a ram prow's factors (GDR-0102).
    pub ram: f32,
    /// Its colours: a key into `art/tokens.json` `world.rock` ([`crate::content::art_keys::ROCK_TONES`]).
    pub tone: String,
    /// Its marks, as the client draws them (the colours are its tone's).
    pub look: RockLook,
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(RockKind, Kind::Rock);

/// How a rock kind's rocks are marked (drawn only, `world::scenery::rocks`; GDR-0104's status line of 18 Oct 2026).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RockLook {
    /// One crater per this many metres of radius (at least one, at most seven).
    pub crater_every: f32,
    /// A ridge across it.
    pub ridge: bool,
    /// A metal glint on its lit side: how strong (0: none).
    pub glint: f32,
    /// Bright fractures across it.
    pub cracks: u32,
    /// Small gas pits over it.
    pub pits: u32,
    /// Ember veins on its shadow side.
    pub embers: u32,
}

/// `landmarks.ron`: something big seen far off in a cluster, drawn behind the play (no body, no effect): a derelict
/// station, a beacon, a ring of junk, a hot star's corona.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Landmark {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lore: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// How the client draws it.
    pub form: LandmarkForm,
    /// Its size `(min, max)` in metres of its own layer (at the camera's starting zoom; GDR-0107), rolled per cluster.
    pub size: (f32, f32),
    /// The share of the camera's motion its layer follows (0: fixed to the sky; below 1: behind the ships' plane); its
    /// size on screen goes with the zoom to this power (GDR-0107).
    pub parallax: f32,
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(Landmark, Kind::Landmark);

/// How a landmark is drawn (the client's vector shapes).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum LandmarkForm {
    /// A broken station: a hub, spokes and a torn ring, dark windows.
    Station,
    /// A beacon mast with a slow light.
    Beacon,
    /// A ring of junk round a dead hub.
    Ring,
    /// A hot star's corona: arcs of light at the edge of the view towards the star. Only where there is a star.
    Corona,
}

/// What fills a cluster type's or a voyage's space between its sites (GDR-0104). Everything defaults to nothing,
/// so a cluster without `scenery` is as empty as before.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenery {
    /// Asteroid rock: real bodies, placed when the fleet warps in.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rocks: Vec<RockGroup>,
    /// How thick the drifting dust and glints are (1: the base amount; 0 none).
    #[serde(default)]
    pub dust: f32,
    /// How thick the bits of hull drifting about are (1: the base amount; thicker near sites).
    #[serde(default)]
    pub debris: f32,
    /// Derelict shells drifting along the way, `(min, max)` per cluster (drawn only).
    #[serde(default)]
    pub derelicts: (u32, u32),
    /// The landmarks that may be seen there, each with its chance per cluster.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub landmarks: Vec<LandmarkPlace>,
}

impl Scenery {
    /// Nothing at all: no rocks, dust, debris, derelicts, or landmarks.
    pub fn is_empty(&self) -> bool {
        *self == Scenery::default()
    }
}

/// Rock of one kind placed in a layout: `count` groups (rolled per cluster), each of `rocks` rocks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RockGroup {
    /// The rock kind (id).
    pub rock: String,
    pub layout: RockLayout,
    /// How many fields, belts or scatterings `(min, max)`.
    pub count: (u32, u32),
    /// Rocks in each `(min, max)`.
    pub rocks: (u32, u32),
}

/// How a group's rocks lie in the cluster.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum RockLayout {
    /// A field: a disc of `radius` metres somewhere in the cluster.
    Field { radius: f32 },
    /// A belt: an arc round the cluster's centre, `width` metres across.
    Belt { width: f32 },
    /// Rocks on their own all over the cluster.
    Scatter,
}

/// A landmark that may be seen in a cluster: `chance` (0..1) per cluster.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LandmarkPlace {
    pub landmark: String,
    pub chance: f32,
}
