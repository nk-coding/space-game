//! The sky's pages (GDR-0105): planet and moon kinds (with their colour sets drawn as swatches), and "The sky"
//! section of cluster types and bands.

use super::*;
use data::{Atmosphere, BodyClass, PlanetForm, PlanetKind, Sky};

/// A planet form in words.
pub(crate) fn planet_form_word(f: PlanetForm) -> &'static str {
    match f {
        PlanetForm::Jovian => "banded giant with a great storm",
        PlanetForm::Ringed => "pale banded giant",
        PlanetForm::IceGiant => "hazy ice giant",
        PlanetForm::HotGiant => "hot giant",
        PlanetForm::Toxic => "smog world",
        PlanetForm::Living => "seas, land and clouds",
        PlanetForm::Garden => "seas and glowing flora",
        PlanetForm::Ocean => "global sea",
        PlanetForm::Eyeball => "tidally locked ice and sea",
        PlanetForm::Desert => "dunes and canyons",
        PlanetForm::Ice => "cracked ice",
        PlanetForm::Lava => "crust and glowing rifts",
        PlanetForm::Rock => "cratered rock",
        PlanetForm::Geyser => "ice with plumes",
        PlanetForm::Sulfur => "volcanic sulfur",
        PlanetForm::Rubble => "irregular rubble",
    }
}

fn air_word(a: Atmosphere) -> &'static str {
    match a {
        Atmosphere::None => "none",
        Atmosphere::Thin => "thin",
        Atmosphere::Moderate => "moderate",
        Atmosphere::Thick => "thick",
    }
}

fn body_word(b: BodyClass) -> &'static str {
    match b {
        BodyClass::Planet => "planet",
        BodyClass::Moon => "moon",
    }
}

impl Wiki<'_> {
    /// The kind's colour sets, a row of swatches each (vector).
    fn palette_figure(p: &PlanetKind) -> String {
        let cols = p.palettes.iter().map(|pal| pal.colors.len() + usize::from(pal.air.is_some())).max().unwrap_or(1);
        let (w, h) = (cols as f32 * 18.0 + 4.0, p.palettes.len() as f32 * 18.0 + 4.0);
        let mut svg = format!("<svg class=\"rocks\" viewBox=\"0 0 {w} {h}\" role=\"img\" aria-label=\"{} colours\">", esc(&p.name));
        for (row, pal) in p.palettes.iter().enumerate() {
            let y = 11.0 + row as f32 * 18.0;
            // The surface's colours, then the air's with a dashed edge.
            let air = pal.air.iter().map(|c| (c, " stroke-dasharray=\"2 2\""));
            for (k, (c, edge)) in pal.colors.iter().map(|c| (c, "")).chain(air).enumerate() {
                write!(svg, "<circle cx=\"{}\" cy=\"{y}\" r=\"7\" fill=\"{}\" stroke=\"#ffffff33\"{edge}/>", 11.0 + k as f32 * 18.0, esc(c)).unwrap();
            }
        }
        svg.push_str("</svg>");
        svg
    }

    /// Who weights a kind in their sky: cluster types and bands, with its share there.
    fn sky_users(&self, root: &str, p: &PlanetKind) -> String {
        let share = |s: &Sky| -> Option<String> {
            let list = if p.body == BodyClass::Planet { &s.planets } else { &s.moons };
            if list.is_empty() && p.body == BodyClass::Planet {
                let n = self.c.planets.iter().filter(|k| k.body == BodyClass::Planet).count().max(1);
                return Some(format!("{} % (any planet alike)", format_number(100.0 / n as f32, 0)));
            }
            let total: u32 = list.iter().map(|w| w.weight).sum();
            let w = list.iter().find(|w| w.kind == p.id)?;
            Some(format!("{} %", format_number(100.0 * w.weight as f32 / total.max(1) as f32, 0)))
        };
        let mut rows: Vec<String> = Vec::new();
        for t in &self.c.cluster_types {
            if let Some(w) = t.sky.as_ref().and_then(share) {
                rows.push(format!("{}: {w}", self.a(root, Kind::ClusterType, &t.id)));
            }
        }
        for b in &self.c.bands {
            if let Some(w) = b.sky.as_ref().and_then(share) {
                rows.push(format!("{} (its voyages and cluster types without a sky of their own): {w}", self.a(root, Kind::Band, &b.id)));
            }
        }
        list_or_none(&rows)
    }

    pub(crate) fn planet_page(&self, p: &PlanetKind) -> String {
        let root = UP;
        let mut facts = vec![
            ("Body".to_string(), body_word(p.body).into()),
            ("Look".into(), planet_form_word(p.form).into()),
            ("Air".into(), air_word(p.atmosphere).into()),
            ("Colour sets".into(), p.palettes.len().to_string()),
        ];
        if let Some(r) = &p.rings {
            facts.push(("Rings".into(), format!("{} % of them{}", format_number(100.0 * r.chance, 0), if r.bright { ", wide and bright" } else { "" })));
        }
        let what = "<p>Seen far off in a cluster's sky, drawn only: it is no body, nothing can reach it, and it changes nothing in a fight. Each cluster rolls its planet and moons from its seed, so a cluster keeps its sky.</p>\n".to_string();
        let seen = self.sky_users(root, p);
        self.entry(
            p,
            EntryParts {
                figure: Some(Self::palette_figure(p)),
                facts,
                sections: vec![("What it is".to_string(), what), ("Seen in".into(), seen)],
                ..Default::default()
            },
        )
    }

    /// "The sky" section of a cluster type's or a band's page.
    pub(crate) fn sky_section(&self, root: &str, s: &Sky) -> (String, String) {
        let mut out = String::new();
        let list = |ws: &[data::SkyWeight]| -> String {
            let total: u32 = ws.iter().map(|w| w.weight).sum();
            ws.iter()
                .map(|w| format!("{} {} %", self.a(root, Kind::Planet, &w.kind), format_number(100.0 * w.weight as f32 / total.max(1) as f32, 0)))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let planets = if s.planets.is_empty() { "any kind alike".to_string() } else { list(&s.planets) };
        let mut facts = vec![format!("Planet: {planets}"), format!("Moons (each kind once at most): {}", list(&s.moons))];
        if s.backlit {
            facts.push("The planet stands against its light: a thin lit crescent, its limb glowing".into());
        }
        facts.push(format!("A nebula at {} × the base strength, a far galaxy at {} ×", format_number(s.nebula_strength, 2), format_number(s.galaxy, 2)));
        out.push_str("<p class=\"note\">Drawn only, far behind the play:</p>\n");
        out.push_str(&ul(&facts));
        ("The sky".into(), out)
    }
}
