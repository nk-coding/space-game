//! The play-test kits (`playtest.ron`): each is for a band a galaxy lists past its first (the first is open from the
//! start, its kit would never be given); its research nodes exist, none is listed twice in the kits, and each node's
//! `requires` come before it in its kit or in the kit of a band nearer the rim; its materials and research families
//! exist, with amounts above 0. Problems name the field as `playtest[<band>].research[2]` and the like (the kits are
//! not an entry kind).

use super::*;
use crate::content::model::Source;

impl<'a> Validator<'a> {
    pub(super) fn playtest(&mut self) {
        let c = self.c;
        // A band's place in the first galaxy that lists it: 0 the rim.
        let depth = |band: &str| c.galaxies.iter().find_map(|g| g.bands.iter().position(|b| b == band));
        let mut out = Vec::new();
        let mut bad = |src: &Source, field: String, msg: String| out.push(Problem::error(msg).at(src).field(field));
        let mut seen: BTreeSet<&str> = BTreeSet::new();
        for kit in &c.playtest {
            let at = format!("playtest[{}]", kit.band);
            let d = depth(&kit.band);
            match d {
                None => bad(&kit.source, format!("{at}.band"), format!("`{}` is no band of a galaxy", kit.band)),
                Some(0) => bad(&kit.source, format!("{at}.band"), format!("`{}` is a galaxy's first band, open from the start: its kit is never given", kit.band)),
                Some(_) => {}
            }
            let before = |id: &str| c.playtest.iter().any(|k| depth(&k.band).zip(d).is_some_and(|(x, d)| x < d) && k.research.iter().any(|r| r == id));
            for (k, id) in kit.research.iter().enumerate() {
                let field = format!("{at}.research[{k}]");
                let Some(node) = c.research.get(id) else {
                    bad(&kit.source, field, format!("unknown research `{id}`"));
                    continue;
                };
                if !seen.insert(id) {
                    bad(&kit.source, field.clone(), format!("`{id}` is listed twice in the kits"));
                }
                let missing: Vec<&str> = node.requires.iter().filter(|r| !kit.research[..k].contains(r) && !before(r)).map(String::as_str).collect();
                if !missing.is_empty() {
                    bad(&kit.source, field, format!("`{id}` requires {}: list it before, here or in a band nearer the rim", missing.join(", ")));
                }
            }
            for (m, &n) in &kit.materials {
                if !c.materials.contains(m) {
                    bad(&kit.source, format!("{at}.materials"), format!("unknown material `{m}`"));
                } else if n == 0 {
                    bad(&kit.source, format!("{at}.materials"), format!("`{m}`: an amount above 0"));
                }
            }
            for (f, &x) in &kit.data {
                if !c.families.contains(f) {
                    bad(&kit.source, format!("{at}.data"), format!("unknown research family `{f}`"));
                } else if !(x.is_finite() && x > 0.0) {
                    bad(&kit.source, format!("{at}.data"), format!("`{f}`: an amount above 0, got {x}"));
                }
            }
        }
        self.out.extend(out);
    }
}
