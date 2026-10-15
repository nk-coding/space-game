//! Validation of ships (part of [`Validator`]'s impl): content ships and drydock designs against the parts, the
//! frames, the factions and their behaviors.

use super::*;

impl<'a> Validator<'a> {
    pub(super) fn ship(&mut self, s: &'a Blueprint) {
        if !self.c.factions.contains(&s.faction) {
            self.err(s, "faction", format!("unknown faction `{}`", s.faction));
        }
        if let Some(b) = &s.behavior {
            self.behavior_ref(s, b);
        }
        let (sw, sh) = s.size;
        let size_ok = if sw == 0 || sh == 0 {
            self.err(s, "size", "size must be at least 1×1");
            false
        } else if sw > MAX_FRAME_SIZE || sh > MAX_FRAME_SIZE {
            self.err(
                s,
                "size",
                format!("each side must be at most {MAX_FRAME_SIZE} cells; got {sw}×{sh}"),
            );
            false
        } else {
            true
        };
        if s.parts.is_empty() {
            self.err(s, "parts", "a ship needs at least one part");
            return;
        }
        // The placements are checked against the frame, so a frame out of range ends the check here.
        if !size_ok {
            return;
        }
        let errors_before = self.out.iter().filter(|p| p.is_error()).count();
        let mut occupied: BTreeMap<(i32, i32), usize> = BTreeMap::new();
        for (k, pl) in s.parts.iter().enumerate() {
            let f = format!("parts[{k}]");
            if pl.rot > 3 {
                self.err(
                    s,
                    format!("{f}.rot"),
                    format!("rot is a number of quarter turns, 0 to 3; got {}", pl.rot),
                );
            }
            let Some(part) = self.c.parts.get(&pl.part) else {
                self.err(
                    s,
                    format!("{f}.part"),
                    format!("unknown part `{}`", pl.part),
                );
                continue;
            };
            match (&pl.material, part.hull) {
                (None, true) => self.err(
                    s,
                    format!("{f}.material"),
                    format!("hull part `{}` needs a material", pl.part),
                ),
                (Some(m), true) => match self.c.materials.get(m) {
                    None => self.err(
                        s,
                        format!("{f}.material"),
                        format!("unknown material `{m}`"),
                    ),
                    Some(mat) if mat.armor.is_none() => self.err(
                        s,
                        format!("{f}.material"),
                        format!("material `{m}` is not an armour material"),
                    ),
                    _ => {}
                },
                (Some(m), false) => self.err(
                    s,
                    format!("{f}.material"),
                    format!("`{}` is not a hull part; remove material `{m}`", pl.part),
                ),
                (None, false) => {}
            }
            if let Some(o) = &pl.power {
                if !part.draws_power() {
                    self.err(
                        s,
                        format!("{f}.power"),
                        format!("`{}` draws no power; remove `power`", pl.part),
                    );
                }
                if let Some(w) = o.weight {
                    self.power_weight(s, &format!("{f}.power.weight"), w);
                }
                if o.tier.is_none() && o.weight.is_none() {
                    self.warn(
                        s,
                        format!("{f}.power"),
                        "an empty power override changes nothing",
                    );
                }
            }
            if let Some(g) = pl.group {
                if part.weapon.is_none() {
                    self.err(s, format!("{f}.group"), format!("`{}` is not a weapon; remove `group`", pl.part));
                } else if !(1..=MAX_WEAPON_GROUP).contains(&g) {
                    self.err(s, format!("{f}.group"), format!("weapon group {g}: groups are 1 to {MAX_WEAPON_GROUP}"));
                }
            }
            let shape = self.c.placed_shape(pl).unwrap();
            let (w, h) = shape.dims();
            let (x, y) = (pl.x as i64, pl.y as i64);
            if x < 0 || y < 0 || x + w as i64 > sw as i64 || y + h as i64 > sh as i64 {
                self.err(
                    s,
                    &f,
                    format!(
                        "`{}` at ({}, {}) with footprint {w}×{h} does not fit the {sw}×{sh} frame",
                        pl.part, pl.x, pl.y
                    ),
                );
                // Its cells are not looked at: they may be far out (or very many, for a bad part size).
                continue;
            }
            for cell in shape.cells() {
                if let Some(other) = occupied.insert(cell, k) {
                    self.err(
                        s,
                        &f,
                        format!(
                            "`{}` overlaps parts[{other}] (`{}`) at cell {:?}",
                            pl.part, s.parts[other].part, cell
                        ),
                    );
                    break;
                }
            }
        }
        if self.out.iter().filter(|p| p.is_error()).count() > errors_before {
            return;
        }
        // The behavior it flies under must fit its rule slots (a ship without cores cannot think at all).
        if let Some(b) = self.c.ship_behavior(s) {
            let slots = self.c.ship_rule_slots(s);
            if slots > 0 && b.rules.len() > slots as usize {
                let (field, whose) = match &s.behavior {
                    Some(_) => ("behavior", "its behavior".to_string()),
                    None => ("faction", "its faction's default behavior".to_string()),
                };
                self.err(
                    s,
                    field,
                    format!(
                        "{whose} `{}` has {} rules, but the ship's logic cores give only {slots} rule slots",
                        b.id,
                        b.rules.len()
                    ),
                );
            }
        }
        // Design warnings (valid content, but probably not intended).
        let (load, capacity) = self.c.ship_control(s);
        if capacity > 0 && load > capacity {
            self.warn(
                s,
                "parts",
                format!(
                    "control load {load} exceeds the logic cores' control capacity {capacity}: the lowest-priority parts start idle"
                ),
            );
        }
        // Clean placements inside a frame of at most MAX_FRAME_SIZE cells always make a grid.
        let Ok(grid) = self.c.ship_grid(s) else {
            return;
        };
        let pieces = grid.pieces();
        if pieces.len() > 1 {
            let sizes: Vec<usize> = pieces.parts.iter().map(Vec::len).collect();
            self.warn(
                s,
                "parts",
                format!(
                    "the ship falls apart into {} pieces (parts per piece: {sizes:?})",
                    pieces.len()
                ),
            );
        }
        for (k, pl) in s.parts.iter().enumerate() {
            let part = self.c.parts.get(&pl.part).unwrap();
            if part.weapon.is_some() && !grid.line_of_fire_clear(k) {
                self.warn(
                    s,
                    format!("parts[{k}]"),
                    format!(
                        "`{}` at ({}, {}) has no clear line of fire",
                        pl.part, pl.x, pl.y
                    ),
                );
            }
            if part.rcs.is_some() && grid.open_sides(k).is_empty() {
                self.warn(
                    s,
                    format!("parts[{k}]"),
                    format!(
                        "`{}` at ({}, {}) has no side open to space, so no RCS nozzle",
                        pl.part, pl.x, pl.y
                    ),
                );
            }
            if part.radiator.is_some() && !grid.is_exposed(k) {
                self.warn(
                    s,
                    format!("parts[{k}]"),
                    format!(
                        "`{}` at ({}, {}) is boxed in and cannot vent",
                        pl.part, pl.x, pl.y
                    ),
                );
            }
        }
    }
}
