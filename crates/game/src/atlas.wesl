// Every atlas mesh (ships, effects, backdrop, UI icons): the atlas texture × the vertex colour, like Bevy's
// `ColorMaterial`, but sampled with a mip LOD bias (`atlas.rs`, `AtlasMaterial`). The atlas tier is the next power
// of two above what the screen needs, so the GPU's level of detail lies between 0 and 1 at the tier's own zoom
// range; plain trilinear sampling then blends in up to all of the next level, which has less resolution than the
// screen needs (at 160 px/m on tier 256: 68 % of the 128 px/cell level, edges about twice as soft as geometry).
// The bias keeps the full level until it is minified about 1.4×; past 2× the next level takes over as before.

import bevy_sprite_render::mesh2d::vertex_output::VertexOutput;
@if(TONEMAP_IN_SHADER)
import bevy_sprite_render::mesh2d::view_bindings::view;
@if(TONEMAP_IN_SHADER)
import bevy_core_pipeline::tonemapping;
@if(SRGB_OUTPUT)
import bevy_render::color_operations::linear_to_srgb;
@if(OKLAB_OUTPUT)
import bevy_render::color_operations::linear_rgb_to_oklab;

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var atlas_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var atlas_sampler: sampler;
// A group's fade (`Atlas::group_material`): its alpha scales the colour's.
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var<uniform> tint: vec4<f32>;

const LOD_BIAS: f32 = -0.5;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var color = textureSampleBias(atlas_texture, atlas_sampler, in.uv, LOD_BIAS);
    @if(VERTEX_COLORS)
    color = color * in.color;
    color.a = color.a * tint.a;
    @if(TONEMAP_IN_SHADER)
    color = tonemapping::tone_mapping(color, view.color_grading);
    @if(SRGB_OUTPUT)
    color = vec4(linear_to_srgb(color.rgb), color.a);
    @if(OKLAB_OUTPUT)
    color = vec4(linear_rgb_to_oklab(color.rgb), color.a);
    return color;
}
