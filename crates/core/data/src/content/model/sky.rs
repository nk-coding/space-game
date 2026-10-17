//! The sky behind a cluster (GDR-0105): the kinds of planet and moon seen far off (`planets.ron`), and a cluster
//! type's or a band's [`Sky`], which weights them and colours the nebula and the far galaxy.
//!
//! Drawn by the client only (`world::scenery::sky`), rolled from the cluster's seed: nothing here touches the
//! simulation, and none of it is in the rules' fingerprint.

use super::*;

/// `planets.ron`: a kind of planet or moon in the sky. The client paints it with the generator its `form` names,
/// in one of its `palettes` (rolled per body; what each colour is for depends on the form, [`PlanetForm`]).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanetKind {
    pub id: String,
    pub name: String,
    pub summary: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub lore: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// A planet (the sky's big body) or a moon (beside it).
    pub body: BodyClass,
    /// How the client paints it.
    pub form: PlanetForm,
    /// Its colour sets, one rolled per body (`#rrggbb`, as many as the form reads, [`PlanetForm::colors`]).
    pub palettes: Vec<Palette>,
    /// How much air it has: none, or a glow round its limb from thin to thick.
    #[serde(default)]
    pub atmosphere: Atmosphere,
    /// Its rings, if it may have any.
    #[serde(default)]
    pub rings: Option<Rings>,
    #[serde(skip_deserializing)]
    pub source: Source,
}
entry!(PlanetKind, Kind::Planet);

/// A planet kind's colour set: the surface's colours (`#rrggbb`) and the air's (with an atmosphere).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Palette {
    pub colors: Vec<String>,
    #[serde(default)]
    pub air: Option<String>,
}

/// A planet kind's rings: the chance a body has them, their colour (`#rrggbb`) and whether they are the wide bright
/// kind (else a few faint bands).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rings {
    pub chance: f32,
    pub color: String,
    #[serde(default)]
    pub bright: bool,
}

/// Planet or moon.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum BodyClass {
    Planet,
    Moon,
}

/// How much air a body has.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Atmosphere {
    #[default]
    None,
    Thin,
    Moderate,
    Thick,
}

/// The generator that paints a planet kind. Each reads its palette's colours in this order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum PlanetForm {
    /// Warm bands and a great storm: storm, small storms, then the band colours.
    Jovian,
    /// Pale bands under wide rings: small storms, then the band colours.
    Ringed,
    /// Deep haze over faint bands, sometimes a dark spot: spot, then the band colours.
    IceGiant,
    /// Dark red bands, a glowing night side: storm, night glow, then the band colours.
    HotGiant,
    /// Featureless smog under a thick glowing limb: the band colours (the first the base).
    Toxic,
    /// Oceans, land, clouds and ice caps: sea, shallows, sand, lowland, highland.
    Living,
    /// Seas and flora glowing on the night side: sea, sand, flora, its glow.
    Garden,
    /// A global sea with islands and storm spirals: sea, shallows, sand, land.
    Ocean,
    /// Tidally locked, a sea facing its star: ice, the outer ring, land, shallows, the deep.
    Eyeball,
    /// Dunes, canyons, ice caps: base, dark, light.
    Desert,
    /// A white crust with cracks: crust, patches, cracks.
    Ice,
    /// A black crust split by glowing rifts: crust, rift glow, rift core, then patch colours.
    Lava,
    /// Cratered and airless: base, maria, light.
    Rock,
    /// Ice with plumes at a pole: ice, cracks.
    Geyser,
    /// Yellow and volcanic: base, then patch colours.
    Sulfur,
    /// Faceted, catching the light: light, dark.
    Crystal,
    /// An irregular captured rock: base, maria, light.
    Rubble,
}

impl PlanetForm {
    /// How many colours a palette of this form needs at least.
    pub fn colors(self) -> usize {
        match self {
            PlanetForm::Jovian | PlanetForm::HotGiant => 4,
            PlanetForm::Ringed | PlanetForm::IceGiant => 3,
            PlanetForm::Toxic => 2,
            PlanetForm::Living | PlanetForm::Eyeball => 5,
            PlanetForm::Garden | PlanetForm::Ocean | PlanetForm::Lava => 4,
            PlanetForm::Desert | PlanetForm::Ice | PlanetForm::Rock | PlanetForm::Rubble => 3,
            PlanetForm::Geyser | PlanetForm::Sulfur | PlanetForm::Crystal => 2,
        }
    }
}

/// The sky of a cluster type or a band (a band's for its content voyages): which planet and moon kinds come up, and
/// the nebula's and the far galaxy's colours and strength.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sky {
    /// Planet kinds (`body: Planet`) with weights; empty: any planet kind alike.
    #[serde(default)]
    pub planets: Vec<SkyWeight>,
    /// Moon kinds (`body: Moon`) with weights; a cluster shows each kind once at most.
    pub moons: Vec<SkyWeight>,
    /// The nebula's two colours along its length and the colour of its bright cores (`#rrggbb`).
    pub nebula: (String, String),
    pub nebula_core: String,
    /// How strong the nebula shows (1: the base).
    pub nebula_strength: f32,
    /// How strong the far galaxy shows (1: the base).
    pub galaxy: f32,
    /// The planet stands against its light (a thin lit crescent and a glowing limb).
    #[serde(default)]
    pub backlit: bool,
}

/// A planet or moon kind (id) and its weight in a [`Sky`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkyWeight {
    pub kind: String,
    pub weight: u32,
}

/// `#rrggbb` as its three bytes.
pub fn hex_rgb(s: &str) -> Option<[u8; 3]> {
    let h = s.strip_prefix('#').filter(|h| h.len() == 6)?;
    let n = u32::from_str_radix(h, 16).ok()?;
    Some([(n >> 16) as u8, (n >> 8) as u8, n as u8])
}
