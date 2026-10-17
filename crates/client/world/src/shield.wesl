// Shield bubbles (GDR-0043, ADR-0029). One quad per bubble in one world-space mesh; the vertex data is described
// in `shield.rs`. Output is light only and the pipeline blends with `max`, so overlapping bubbles never add up.
// Units: metres for widths and offsets, `r` / `q` are normalised by the bubble radius.

import bevy_sprite_render::mesh2d::{functions as mesh_functions, view_bindings::globals};
@if(SRGB_OUTPUT)
import bevy_render::color_operations::linear_to_srgb;

struct BubbleStyle {
    color: vec4<f32>,
    core: vec4<f32>,
    // fill at the centre, fill at the rim, rim line, outer glow
    opacity: vec4<f32>,
    // rim width, glow width, shimmer depth, shimmer speed
    shape: vec4<f32>,
    // share of capacity below which the bubble flickers
    low: vec4<f32>,
    // 1: over space, at world z y (under every ship's tiles); 0: over its own piece's tiles, at their depth
    depth: vec4<f32>,
};

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> style: BubbleStyle;

struct Vertex {
    @builtin(instance_index) instance_index: u32,
    @location(0) position: vec3<f32>,
    // offset from the centre (x, y), radius, seed
    @location(1) bubble: vec4<f32>,
    // charge, presence, shatter progress (0 = none)
    @location(2) state: vec4<f32>,
    // direction to the impact (x, y), age 0..1 (1 = none), strength
    @location(3) hit0: vec4<f32>,
    @location(4) hit1: vec4<f32>,
    @location(5) hit2: vec4<f32>,
    @location(6) hit3: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip: vec4<f32>,
    @location(0) offset: vec2<f32>,
    @location(1) @interpolate(flat) bubble: vec2<f32>,
    @location(2) @interpolate(flat) state: vec4<f32>,
    @location(3) @interpolate(flat) hit0: vec4<f32>,
    @location(4) @interpolate(flat) hit1: vec4<f32>,
    @location(5) @interpolate(flat) hit2: vec4<f32>,
    @location(6) @interpolate(flat) hit3: vec4<f32>,
};

@vertex
fn vertex(v: Vertex) -> VertexOutput {
    var out: VertexOutput;
    let world_from_local = mesh_functions::get_world_from_local(v.instance_index);
    var world = mesh_functions::mesh2d_position_local_to_world(world_from_local, vec4<f32>(v.position, 1.0));
    if style.depth.x > 0.5 {
        world.z = style.depth.y;
    }
    out.clip = mesh_functions::mesh2d_position_world_to_clip(world);
    out.offset = v.bubble.xy;
    out.bubble = v.bubble.zw;
    out.state = v.state;
    out.hit0 = v.hit0;
    out.hit1 = v.hit1;
    out.hit2 = v.hit2;
    out.hit3 = v.hit3;
    return out;
}

const TAU: f32 = 6.28318530718;

fn hash11(x: f32) -> f32 {
    var p = fract(x * 0.1031);
    p = p * (p + 33.33);
    p = p * (p + p);
    return fract(p);
}

fn hash12(p: vec2<f32>) -> f32 {
    var p3 = fract(vec3<f32>(p.x, p.y, p.x) * 0.1031);
    p3 = p3 + dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * p3.z);
}

fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash12(i);
    let b = hash12(i + vec2<f32>(1.0, 0.0));
    let c = hash12(i + vec2<f32>(0.0, 1.0));
    let d = hash12(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

// (distance to the nearest cell edge, random id of the cell) of a Voronoi pattern.
fn voronoi(p: vec2<f32>) -> vec2<f32> {
    let i = floor(p);
    let f = fract(p);
    var d1 = 8.0;
    var d2 = 8.0;
    var id = 0.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let g = vec2<f32>(f32(x), f32(y));
            let o = vec2<f32>(hash12(i + g), hash12(i + g + 17.31));
            let rv = g + o - f;
            let dd = dot(rv, rv);
            if (dd < d1) {
                d2 = d1;
                d1 = dd;
                id = hash12(i + g + 3.17);
            } else if (dd < d2) {
                d2 = dd;
            }
        }
    }
    return vec2<f32>(sqrt(d2) - sqrt(d1), id);
}

// One hit at normalised position `q` (`r` = |q|): (ripple ring inside the bubble, flash at the impact, light
// spreading along the rim).
fn hit_light(h: vec4<f32>, q: vec2<f32>, r: f32) -> vec3<f32> {
    if (h.z >= 1.0) {
        return vec3<f32>(0.0);
    }
    let a = h.z;
    let w = h.w;
    let dk = length(q - h.xy);
    let fade = (1.0 - a) * (1.0 - a);
    let front = 2.1 * (1.0 - (1.0 - a) * (1.0 - a));
    let x = (dk - front) / (0.06 + 0.16 * a);
    let ring = exp(-x * x) * fade * w / (1.0 + 0.8 * dk);
    let fx = dk / (0.16 + 0.3 * a);
    let flash = exp(-fx * fx) * exp(-a * 5.0) * w;
    let along = acos(clamp(dot(q / max(r, 1e-4), h.xy), -1.0, 1.0)) / (0.25 + 2.6 * a);
    let rim = exp(-along * along) * fade * w;
    return vec3<f32>(ring, flash, rim);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let radius = in.bubble.x;
    let seed = in.bubble.y;
    let charge = in.state.x;
    let presence = in.state.y;
    let shatter = in.state.z;
    let t = globals.time;

    let d = length(in.offset);
    // Metres per pixel; taken before any branch (derivatives need uniform control flow).
    let aa = max(fwidth(d), 1e-4);
    let r = d / radius;
    let q = in.offset / radius;
    let dir = in.offset / max(d, 1e-4);
    let rim_w = style.shape.x;
    let glow_w = style.shape.y;
    let fill_rim = style.opacity.y;
    let inside = 1.0 - smoothstep(radius - aa, radius + aa, d);

    // Hits: a ripple from the impact, a flash where the shot entered, light running along the rim (it widens
    // and whitens the rim there, which is near full brightness already).
    let hit = hit_light(in.hit0, q, r) + hit_light(in.hit1, q, r) + hit_light(in.hit2, q, r)
        + hit_light(in.hit3, q, r);
    let lit = min(hit.z, 1.5);

    // Rim: a crisp line (energy-conserving when thinner than a pixel) over a soft glow, steeper inwards.
    let line_w = rim_w * (0.7 + 0.3 * charge) * (1.0 + 1.5 * lit);
    let lw = max(line_w, aa);
    let lx = (d - radius) / lw;
    var line = exp(-lx * lx) * (line_w / lw);
    let gd = d - radius;
    let gw = glow_w * (1.0 + 0.8 * lit);
    // (faded out before the edge of the quad, which ends 4 glow widths outside the rim)
    let glow = exp(-max(gd, 0.0) / gw) * exp(-max(-gd, 0.0) / (0.6 * gw))
        * (1.0 - smoothstep(3.0 * glow_w, 4.0 * glow_w, gd));

    // Fill: almost clear in the middle, brighter towards the rim, with a slow shimmer and faint veins.
    var fill = mix(style.opacity.x, fill_rim, pow(min(r, 1.0), 4.0)) * inside;
    let drift = t * style.shape.w;
    let sp = in.offset * 0.9 + vec2<f32>(seed * 61.0, seed * 37.0);
    let n1 = vnoise(sp + vec2<f32>(drift, -0.7 * drift));
    let n2 = vnoise(sp * 1.8 + vec2<f32>(-0.6 * drift, drift) + 4.3);
    let veins = pow(1.0 - abs(n1 + n2 - 1.0), 10.0);
    let sh = style.shape.z;
    fill = fill * (1.0 - 0.5 * sh + sh * n1) + veins * sh * fill_rim * 0.9 * r * r * inside;

    // Low charge: the bubble stutters, wobbles, and its rim breaks into crackling arcs (time-based, so the
    // same at any frame rate).
    let low = 1.0 - smoothstep(0.0, style.low.x, charge);
    let dip = step(hash11(floor(t * 15.0) + seed * 97.0), 0.5 * low);
    let flicker = mix(1.0, 0.85 + 0.15 * sin(t * 43.0 + seed * 30.0), low) * (1.0 - 0.7 * dip);
    let arcs = smoothstep(0.3, 0.65, vnoise(dir * 2.3 + vec2<f32>(t * 5.0, seed * 13.0)));
    line = line * mix(1.0, 0.2 + 0.8 * arcs, low);

    let strength = 0.55 + 0.45 * charge;
    var body = (fill * (0.6 + 0.4 * charge)
        + (style.opacity.w * glow + style.opacity.z * line) * (1.0 + 0.6 * lit) * strength) * flicker;
    body = body + 0.85 * hit.x * inside + hit.y;
    var core = clamp(0.4 * line + 0.6 * lit * line + hit.y + 0.8 * hit.x, 0.0, 1.0);
    var intensity = body * presence;

    // Shatter: a short pop, bright cracks run across the membrane, its fragments drift out and go dark one by
    // one, and the rim breaks into arcs that fly outwards. Faint areas stay small: under max blending a wide
    // faint fill reads as a grey film, thin bright lines read as light.
    if (shatter > 0.0) {
        let c = shatter;
        let ease = 1.0 - (1.0 - c) * (1.0 - c);
        let pop = exp(-c * 14.0);
        let qs = q / (1.0 + 0.3 * ease);
        let rs = length(qs);
        let px = aa / radius;
        let in_s = 1.0 - smoothstep(1.0 - px, 1.0 + px, rs);
        let vor = voronoi(qs * 3.2 + vec2<f32>(seed * 17.0, seed * 29.0));
        let gap = 0.03 + 0.35 * c;
        let out_at = 0.3 + 0.6 * vor.y;
        let cell = smoothstep(gap, gap + 0.04, vor.x) * (1.0 - smoothstep(out_at - 0.2, out_at, c));
        let membrane = cell * in_s * fill_rim * (0.1 + 1.3 * rs * rs * rs * rs) * (1.0 + 2.0 * pop) * (1.0 - c);
        let cw = max(0.035, 1.5 * px * 3.2);
        let crack = (1.0 - smoothstep(0.0, cw, vor.x)) * in_s * (0.5 + 0.5 * rs * rs)
            * smoothstep(0.0, 0.03, c) * exp(-c * 3.0);
        let n = 20.0;
        let u = (atan2(qs.y, qs.x) / TAU + 0.5) * n;
        let lift = hash11(floor(u) * 7.13 + seed * 91.0);
        let half = 0.5 - (0.06 + 0.3 * c);
        let along = 1.0 - smoothstep(half - 0.06, half, abs(fract(u) - 0.5));
        let sw = rim_w * 1.5 / radius;
        let rw = max(sw, px);
        let sd = (rs - (1.0 + 0.3 * lift * ease)) * radius;
        let rx = sd / (rw * radius);
        let halo = exp(-abs(sd) / (0.6 * glow_w)) * style.opacity.w;
        let shards = along * (exp(-rx * rx) * (sw / rw) * min(1.6 * style.opacity.z, 1.0) + halo)
            * pow(1.0 - c, 0.7);
        let fade = 1.0 - smoothstep(0.0, 0.2, c);
        intensity = body * fade + membrane + crack + shards + 0.25 * pop * in_s * (0.3 + 0.7 * rs * rs);
        core = max(core * fade, clamp(crack + 0.6 * shards + pop, 0.0, 1.0));
    }

    intensity = clamp(intensity, 0.0, 1.0);
    var rgb = mix(style.color.rgb, style.core.rgb, core);
    @if(SRGB_OUTPUT)
    rgb = linear_to_srgb(rgb);
    return vec4<f32>(rgb * intensity, intensity);
}
