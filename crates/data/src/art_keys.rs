//! Keys that content uses to refer to art. They must exist in `art/tokens.json`; content never holds
//! colours itself, so the client (and the wiki) map these keys to the style-2a palette.

/// `ship.system_colors` in `art/tokens.json`: icon colours of the systems.
pub const SYSTEM_COLORS: &[&str] = &[
    "power",
    "weapons",
    "shields",
    "heat",
    "propulsion",
    "logic",
    "cargo",
    "enemy",
];

/// `ship.armor` in `art/tokens.json`: armour tile tones.
pub const ARMOR_TONES: &[&str] = &["steel", "composite", "reactive", "enemy"];

/// Hull glow colours: "player" = `ship.hull_glow.color` (teal), "enemy" = `ship.hull_glow.enemy_color` (coral).
pub const GLOWS: &[&str] = &["player", "enemy"];

/// Icon keys are file stems in `art/icons` (`<key>.svg`, `<key>.icon-only.svg`): lowercase kebab-case.
pub fn is_icon_key(key: &str) -> bool {
    !key.is_empty()
        && key.split('-').all(|w| {
            !w.is_empty()
                && w.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
}
