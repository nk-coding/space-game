//! Validation of parts (part of [`Validator`]'s impl): every component and its numbers, and the parts and materials
//! that must stay unlocked.

use super::*;

impl<'a> Validator<'a> {
    pub(super) fn part(&mut self, p: &'a Part) {
        if !self.c.categories.contains(&p.category) {
            self.err(p, "category", format!("unknown category `{}`", p.category));
        }
        for (k, old) in p.replaces.iter().enumerate() {
            let field = format!("replaces[{k}]");
            if *old == p.id {
                self.err(p, field, "a part cannot stand in for itself");
            } else if let Some(other) = self.c.parts.iter().find(|q| q.id < p.id && q.replaces.contains(old)) {
                self.err(p, field, format!("`{}` stands in for `{old}` already", other.id));
            } else if let Some(q) = self.c.parts.get(old).filter(|q| q.size != p.size || q.shape != p.shape || q.hull != p.hull) {
                self.err(p, field, format!("`{}` has another footprint: a design refitted with this part would change shape", q.id));
            }
        }
        if let Some(icon) = &p.icon {
            self.icon(p, "icon", icon);
        }
        if !p.hull {
            self.ship_drawing(p);
        }
        if let Some(col) = &p.color {
            self.key_in(p, "color", col, SYSTEM_COLORS, "system colour");
        }
        if let Some(b) = &p.barrel {
            if p.weapon.is_none() {
                self.err(p, "barrel", "only guns (parts with `weapon`) have a barrel");
            }
            self.number(p, "barrel.width", b.width, 0.0, true);
            self.number(p, "barrel.length", b.length, 0.0, false);
            self.number(p, "barrel.muzzle", b.muzzle, 0.0, true);
        }
        let (w, h) = p.size;
        if w == 0 || h == 0 {
            self.err(p, "size", "footprint must be at least 1×1");
        } else if w > MAX_PART_SIDE || h > MAX_PART_SIDE {
            self.err(
                p,
                "size",
                format!("each side of a footprint is at most {MAX_PART_SIDE} cells; got {w}×{h}"),
            );
        }
        if p.shape == Shape::Slope {
            if h != 1 {
                self.err(p, "size", format!("slopes must be one cell high (w × 1); got {w}×{h}. Steep slopes come from rotating"));
            }
            if !p.hull {
                self.err(p, "shape", "slopes must be hull parts (hull: true)");
            }
        }
        self.number(p, "build_points", p.build_points, 0.0, false);
        if p.hull {
            if p.mass.is_some() {
                self.err(
                    p,
                    "mass",
                    "hull parts take their mass from the material; remove `mass`",
                );
            }
            if p.hp.is_some() {
                self.err(
                    p,
                    "hp",
                    "hull parts take their hit points from the material; remove `hp`",
                );
            }
        } else {
            match p.mass {
                Some(m) => self.number(p, "mass", m, 0.0, true),
                None => self.err(p, "mass", "non-hull parts need `mass`"),
            }
            match p.hp {
                Some(x) => self.number(p, "hp", x, 0.0, true),
                None => self.err(p, "hp", "non-hull parts need `hp`"),
            }
        }
        if let Some(s) = &p.salvage {
            if p.hull {
                self.err(
                    p,
                    "salvage",
                    "hull parts are salvaged per cell by their material's `salvage`; remove `salvage`",
                );
            }
            self.family_ref(p, "salvage.family", &s.family);
            self.research_data(p, "salvage.data", s.data, false);
            self.material_amounts(p, "salvage.fragment", &s.fragment);
        }
        if let Some(r) = &p.recipe {
            if p.hull {
                self.err(p, "recipe", "hull parts are not fabricated (their plating is not counted); remove `recipe`");
            }
            if r.materials.is_empty() {
                self.err(p, "recipe.materials", "a recipe takes at least one material");
            }
            self.material_amounts(p, "recipe.materials", &r.materials);
            if let Some(n) = &r.needs {
                match self.c.facilities.get(n) {
                    None => self.err(p, "recipe.needs", format!("unknown facility `{n}`")),
                    Some(f) if f.within.is_none() => {
                        self.err(p, "recipe.needs", format!("`{n}` is no workshop upgrade (it has no `within`)"))
                    }
                    Some(_) => {}
                }
            }
        }
        if let Some(x) = &p.power {
            self.number(p, "power.output", x.output, 0.0, false);
            self.number(p, "power.draw", x.draw, 0.0, false);
            self.power_weight(p, "power.weight", x.weight);
            if !(x.min_level.is_finite() && (0.0..=1.0).contains(&x.min_level)) {
                self.err(
                    p,
                    "power.min_level",
                    format!("a share of the demand, 0 to 1, got {}", x.min_level),
                );
            }
            let allocation_set =
                x.tier != PowerTier::default() || x.weight != 1.0 || x.min_level != 0.0;
            if x.draw <= 0.0 && allocation_set {
                self.err(
                    p,
                    "power",
                    "`tier`, `weight` and `min_level` are for parts that draw power",
                );
            }
            if let Some(words) = &x.intermittent {
                if words.trim().is_empty() {
                    self.err(p, "power.intermittent", "say when it draws, in a few words (\"while scanning\")");
                }
                if p.scanner.is_none() || x.draw <= 0.0 {
                    self.err(p, "power.intermittent", "only a scanner's draw comes in bursts (while a scan charges)");
                }
            }
        }
        if let Some(x) = &p.heat {
            self.number(p, "heat.output", x.output, 0.0, false);
            if let Some(l) = x.limit {
                self.number(p, "heat.limit", l, 0.0, true);
            }
        }
        if let Some(x) = &p.radiator {
            self.number(p, "radiator.vent", x.vent, 0.0, true);
        }
        if let Some(x) = &p.weapon {
            self.number(p, "weapon.damage", x.damage, 0.0, false);
            self.number(p, "weapon.range", x.range, 0.0, true);
            match x.delivery {
                crate::content::model::Delivery::Beam => {}
                crate::content::model::Delivery::Pulse { rate } => self.shot_rate(p, rate),
                crate::content::model::Delivery::Projectile {
                    rate,
                    speed,
                    penetration,
                    recoil,
                } => {
                    self.shot_rate(p, rate);
                    self.number(p, "weapon.delivery.speed", speed, 0.0, true);
                    self.number(p, "weapon.delivery.recoil", recoil, 0.0, false);
                    if penetration == 0 {
                        self.err(p, "weapon.delivery.penetration", "must be at least 1");
                    }
                    self.count(p, "weapon.delivery.penetration", penetration, MAX_COUNT);
                }
            }
        }
        if let Some(x) = &p.engine {
            self.number(p, "engine.thrust", x.thrust, 0.0, true);
            self.number(p, "engine.rated_speed", x.rated_speed, 0.0, true);
            self.number(p, "engine.gimbal", x.gimbal, 0.0, false);
            if x.gimbal > MAX_GIMBAL {
                self.err(
                    p,
                    "engine.gimbal",
                    format!("at most {MAX_GIMBAL} degrees, got {}", x.gimbal),
                );
            }
            self.number(p, "engine.plume.length", x.plume.length, 0.0, false);
            self.number(p, "engine.plume.damage", x.plume.damage, 0.0, false);
            self.number(p, "engine.plume.heat", x.plume.heat, 0.0, false);
            if x.plume.length > MAX_PLUME_LENGTH {
                self.err(
                    p,
                    "engine.plume.length",
                    format!("at most {MAX_PLUME_LENGTH} m, got {}", x.plume.length),
                );
            }
        }
        if let Some(x) = &p.explosion {
            self.number(p, "explosion.damage", x.damage, 0.0, true);
            self.number(p, "explosion.radius", x.radius, 0.0, true);
            if x.radius > MAX_BLAST_RADIUS {
                self.err(
                    p,
                    "explosion.radius",
                    format!("at most {MAX_BLAST_RADIUS} m, got {}", x.radius),
                );
            }
        }
        if let Some(x) = &p.heart {
            self.number(p, "heart.interval", x.interval, 0.0, true);
            if !(x.hp > 0.0 && x.hp <= 1.0) {
                self.err(p, "heart.hp", format!("a share of the full hit points, above 0 and at most 1, got {}", x.hp));
            }
            if p.hull {
                self.err(p, "heart", "hull parts cannot be hearts");
            }
        }
        if let Some(x) = p.heal {
            self.number(p, "heal", x, 0.0, true);
            if p.hull {
                self.err(p, "heal", "hull parts heal by their armour's `heal`; remove `heal`");
            }
        }
        if let Some(x) = &p.ram {
            self.number(p, "ram.dealt", x.dealt, 0.0, false);
            self.number(p, "ram.taken", x.taken, 0.0, false);
        }
        if let Some(x) = &p.gyro {
            self.number(p, "gyro.torque", x.torque, 0.0, true);
        }
        if let Some(x) = &p.rcs {
            self.number(p, "rcs.thrust", x.thrust, 0.0, true);
            self.number(p, "rcs.rated_speed", x.rated_speed, 0.0, true);
            if p.hull {
                self.err(p, "rcs", "hull parts cannot carry RCS nozzles");
            }
        }
        if let Some(x) = &p.shield {
            self.number(p, "shield.radius", x.radius, 0.0, true);
            self.number(p, "shield.capacity", x.capacity, 0.0, false);
            self.number(p, "shield.regen", x.regen, 0.0, false);
            for (f, &a) in &x.absorb {
                self.number(p, &format!("shield.absorb.{f:?}"), a, 0.0, false);
                if a > 1.0 {
                    self.err(p, format!("shield.absorb.{f:?}"), "a share, at most 1");
                }
            }
        }
        if let Some(x) = &p.cargo {
            self.number(p, "cargo.capacity", x.capacity, 0.0, true);
        }
        if let Some(x) = &p.sensor {
            self.reach(p, "sensor.range", x.range);
        }
        if let Some(x) = &p.scanner {
            self.reach(p, "scanner.range", x.range);
            self.reach(p, "scanner.wake", x.wake);
            self.number(p, "scanner.charge", x.charge, 0.0, true);
            self.number(p, "scanner.cooldown", x.cooldown, 0.0, false);
            if !p.draws_power() {
                self.err(p, "scanner", "a scanner draws power while it charges (`power.draw`)");
            }
        }
        if let Some(c) = p.control_cost {
            if c > MAX_CONTROL_COST {
                self.err(
                    p,
                    "control_cost",
                    format!("at most {MAX_CONTROL_COST}, got {c}"),
                );
            } else if c > 0 && !p.has_power_role() {
                self.warn(
                    p,
                    "control_cost",
                    "the part draws and makes no power, so it can never go idle; it only uses up control capacity",
                );
            }
        }
        if let Some(l) = &p.logic {
            self.count(p, "logic.rule_slots", l.rule_slots, crate::behavior::MAX_RULES as u32);
            self.count(p, "logic.control_capacity", l.control_capacity, MAX_COUNT);
            if l.control_capacity == 0 {
                self.warn(
                    p,
                    "logic.control_capacity",
                    "a core with no control capacity runs no parts",
                );
            }
        }
        for (k, b) in p.bonuses.iter().enumerate() {
            let f = format!("bonuses[{k}]");
            match &b.target {
                Selector::Part(id) if !self.c.parts.contains(id) => {
                    self.err(p, format!("{f}.target"), format!("unknown part `{id}`"))
                }
                Selector::Category(id) if !self.c.categories.contains(id) => {
                    self.err(p, format!("{f}.target"), format!("unknown category `{id}`"))
                }
                Selector::Tag(t) if !self.c.parts.iter().any(|q| q.has_tag(t)) => self.err(
                    p,
                    format!("{f}.target"),
                    format!("no part has the tag `{t}`"),
                ),
                _ => {}
            }
            if !self.c.stats.contains(&b.stat) {
                self.err(p, format!("{f}.stat"), format!("unknown stat `{}`", b.stat));
            } else if !BONUS_STATS.contains(&b.stat.as_str()) {
                self.err(
                    p,
                    format!("{f}.stat"),
                    format!(
                        "bonuses cannot change `{}`; they change {}",
                        b.stat,
                        BONUS_STATS.join(", ")
                    ),
                );
            }
            match b.effect {
                Effect::Add(x) if !(x.is_finite() && x.abs() <= MAX_NUMBER) => self.err(
                    p,
                    format!("{f}.effect"),
                    format!("must be a finite number from -{MAX_NUMBER} to {MAX_NUMBER}, got {x}"),
                ),
                Effect::Percent(x) if !(x.is_finite() && x > -100.0 && x <= MAX_BONUS_PERCENT) => {
                    self.err(
                        p,
                        format!("{f}.effect"),
                        format!(
                            "a percentage above -100 and at most {MAX_BONUS_PERCENT}, got {x}"
                        ),
                    )
                }
                _ => {}
            }
            match b.max_stacks {
                Some(0) => self.err(
                    p,
                    format!("{f}.max_stacks"),
                    "must be at least 1 (or omitted for no limit)",
                ),
                Some(n) => self.count(p, &format!("{f}.max_stacks"), n, MAX_COUNT),
                None => {}
            }
        }
    }

    /// Locked parts and materials are hidden in the drydock until unlocked, so the parts the drydock paints,
    /// fills and bevels with must stay available, and so must at least one armour material.
    pub(super) fn locked(&mut self) {
        let c = self.c;
        for (shape, what) in [
            (Shape::Rect, "paints and fills with"),
            (Shape::Slope, "bevels with"),
        ] {
            if let Some(p) = c.unit_hull_part(shape).filter(|p| p.locked) {
                self.err(
                    p,
                    "locked",
                    format!("the drydock {what} the first 1×1 hull part of this shape, so it can't be locked"),
                );
            }
        }
        let mut armor = c.materials.iter().filter(|m| m.armor.is_some());
        if let Some(first) = armor.clone().next()
            && armor.all(|m| m.locked)
        {
            self.err(
                first,
                "locked",
                "every armour material is locked: the drydock has nothing to build hull parts in",
            );
        }
        if let Some(first) = c.frames.iter().next()
            && c.frames.iter().all(|f| f.locked)
        {
            self.err(
                first,
                "locked",
                "every frame is locked: a new game has no frame to build in",
            );
        }
    }
}
