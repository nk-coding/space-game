// A dashed pill outline for bevy_ui nodes (the behavior editor's Rhai nodes and its "+ RULE" pill): the stroke of
// a stadium as wide as the node's left border, dashes spaced so they close evenly around it. The colour comes in
// as sRGB numbers (like every UI colour here, `style::ui_color`), so it is returned as is.
import bevy_ui_render::ui_vertex_output::UiVertexOutput;

struct DashedOutline {
    color: vec4<f32>,
}

@group(1) @binding(0)
var<uniform> material: DashedOutline;

const PI: f32 = 3.14159265;
// Dash and gap lengths in stroke widths.
const DASH: f32 = 4.5;
const GAP: f32 = 3.5;

@fragment
fn fragment(in: UiVertexOutput) -> @location(0) vec4<f32> {
    let size = in.size;
    let w = max(in.border_widths.x * size.x, 1.0);
    // Pixels from the centre, y down.
    let p = (in.uv - vec2<f32>(0.5)) * size;
    let rc = max(size.y * 0.5 - w * 0.5, 0.5);
    let a = max(size.x * 0.5 - size.y * 0.5, 0.0);
    // Distance to the stroke's centre line; coverage of the stroke.
    let q = vec2<f32>(max(abs(p.x) - a, 0.0), p.y);
    let d = abs(length(q) - rc) - w * 0.5;
    let aa = max(fwidth(d), 1e-4);
    let stroke = 1.0 - smoothstep(-aa, aa, d);

    // Arc length along the centre line, clockwise from the left end of the top edge.
    let top = p.x + a;
    let right = 2.0 * a + rc * atan2(p.x - a, -p.y);
    let bottom = 2.0 * a + PI * rc + (a - p.x);
    let left = 4.0 * a + PI * rc + rc * atan2(-(p.x + a), p.y);
    let straight = select(bottom, top, p.y < 0.0);
    let cap = select(left, right, p.x > 0.0);
    let s = select(cap, straight, abs(p.x) <= a);

    let length_ = 4.0 * a + 2.0 * PI * rc;
    let n = max(round(length_ / ((DASH + GAP) * w)), 1.0);
    let period = length_ / n;
    let on = select(0.0, 1.0, fract(s / period) < DASH / (DASH + GAP));
    return vec4<f32>(material.color.rgb, material.color.a * stroke * on);
}
