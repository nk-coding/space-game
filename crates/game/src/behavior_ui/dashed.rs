//! A dashed pill outline for bevy_ui nodes (bevy_ui borders are solid): a UI material drawn by `dashed.wesl`.
//! Put [`dashed_outline`] as a child of a pill-shaped node; its border width sets the stroke, the material's
//! colour (an sRGB colour from `style::ui_color`) the colour.

use bevy::asset::{AssetPath, embedded_asset, embedded_path};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;
use bevy::ui_render::prelude::{MaterialNode, UiMaterial, UiMaterialPlugin};

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct DashedOutline {
    #[uniform(0)]
    pub color: LinearRgba,
}

impl UiMaterial for DashedOutline {
    fn fragment_shader() -> ShaderRef {
        ShaderRef::Path(AssetPath::from_path_buf(embedded_path!("dashed.wesl")).with_source("embedded"))
    }
}

pub(super) fn plugin(app: &mut App) {
    embedded_asset!(app, "dashed.wesl");
    app.add_plugins(UiMaterialPlugin::<DashedOutline>::default());
}

/// The outline node: fills its (positioned) parent; `width` is the stroke in logical px.
pub fn dashed_outline(material: Handle<DashedOutline>, width: f32) -> impl Bundle {
    (
        MaterialNode(material),
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(0),
            bottom: px(0),
            border: UiRect::all(px(width)),
            ..default()
        },
    )
}

/// The material's colour from a UI colour (already through `style::ui_color`).
pub fn outline_color(c: Color) -> LinearRgba {
    c.to_linear()
}
