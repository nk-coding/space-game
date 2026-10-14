//! The UI's fonts: Barlow Regular and Medium (OFL, `assets/fonts/`), embedded and loaded once at `Startup`
//! ([`load_fonts`]); every screen, the flight HUD and the kit's widgets read them from [`HudFonts`].

use bevy::prelude::*;

pub static BARLOW_REGULAR: &[u8] = include_bytes!("../../assets/fonts/Barlow-Regular.ttf");
static BARLOW_MEDIUM: &[u8] = include_bytes!("../../assets/fonts/Barlow-Medium.ttf");

#[derive(Resource, Clone)]
pub struct HudFonts {
    pub regular: Handle<Font>,
    pub medium: Handle<Font>,
}

/// `Startup`: the fonts as assets, in [`HudFonts`]. What spawns text goes after it.
pub fn load_fonts(mut commands: Commands, mut fonts_assets: ResMut<Assets<Font>>) {
    commands.insert_resource(HudFonts {
        regular: fonts_assets.add(Font::from_bytes(BARLOW_REGULAR.to_vec())),
        medium: fonts_assets.add(Font::from_bytes(BARLOW_MEDIUM.to_vec())),
    });
}
