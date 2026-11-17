// The whole office, as three.js r170 draws scene.ts: one frame of lights and
// camera, then per draw its place and its look. Lit draws are the clay
// (MeshStandardMaterial, roughness 0.82, with clay.ts's room glow, edge light
// and storey foot) or the glass; unlit draws hold their own colour (no tone
// mapping), laid on as is, added as light, or over everything. focus.ts's
// band dims what it covers outside one band of heights. The sRGB target
// encodes on write.

struct Frame {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    light: mat4x4<f32>,
    // Towards the sun, w unused; the sun's colour times its intensity.
    sun: vec4<f32>,
    sun_color: vec4<f32>,
    // The hemisphere's sky and ground colours times its intensity.
    sky: vec4<f32>,
    ground: vec4<f32>,
    // The camera; w = tone mapping exposure.
    eye: vec4<f32>,
    // (bias, normal bias, intensity, 1 when shadows are on).
    shadow: vec4<f32>,
    // focus.ts: (lowest, highest, how far the rest falls back, unused).
    band: vec4<f32>,
    // (width, height in pixels, a star's half size in pixels, unused).
    viewport: vec4<f32>,
};

struct Draw {
    model: mat4x4<f32>,
    // Clay: the room's glow (rgb) and the foot; the edge light (rgb) and the roughness.
    room: vec4<f32>,
    rim: vec4<f32>,
    // The colour the look multiplies by (rgb) and its opacity.
    tint: vec4<f32>,
    // (1 when the band dims it, 1 when tint replaces the vertex colour, 1 for round marks, unused).
    flags: vec4<f32>,
};

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(1) var shadow_map: texture_depth_2d;
@group(0) @binding(2) var shadow_cmp: sampler_comparison;
@group(1) @binding(0) var<uniform> draw: Draw;
@group(2) @binding(0) var picture: texture_2d<f32>;
@group(2) @binding(1) var picture_at: sampler;

struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) world: vec3<f32>,
    @location(3) shadow_pos: vec4<f32>,
    @location(4) uv: vec2<f32>,
};

// Every textured thing is a PlaneGeometry of one quad, unrolled: its corners'
// uv in that order.
fn plane_uv(i: u32) -> vec2<f32> {
    switch (i % 6u) {
        case 0u: { return vec2<f32>(0.0, 1.0); }
        case 1u: { return vec2<f32>(0.0, 0.0); }
        case 2u: { return vec2<f32>(1.0, 1.0); }
        case 3u: { return vec2<f32>(0.0, 0.0); }
        case 4u: { return vec2<f32>(1.0, 0.0); }
        default: { return vec2<f32>(1.0, 1.0); }
    }
}

@vertex
fn vs(@builtin(vertex_index) i: u32, @location(0) pos: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) color: vec3<f32>) -> Out {
    var o: Out;
    let world = draw.model * vec4<f32>(pos, 1.0);
    o.clip = frame.proj * frame.view * world;
    o.normal = (draw.model * vec4<f32>(normal, 0.0)).xyz;
    o.color = color;
    o.world = world.xyz;
    o.shadow_pos = frame.light * vec4<f32>(world.xyz + normalize(o.normal) * frame.shadow.y, 1.0);
    // uv with three's flipY: the picture's top row at v = 1.
    let uv = plane_uv(i);
    o.uv = vec2<f32>(uv.x, 1.0 - uv.y);
    return o;
}

// A sprite: `pos` its middle, `normal` (corner x, corner y, half size). Stars
// are sized in pixels, the rising marks in the world, both facing the camera.
@vertex
fn vs_star(@location(0) pos: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) color: vec3<f32>) -> Out {
    var o: Out;
    let world = draw.model * vec4<f32>(pos, 1.0);
    var clip = frame.proj * frame.view * world;
    clip = vec4<f32>(clip.xy + normal.xy * frame.viewport.z * 2.0 / frame.viewport.xy * clip.w, clip.zw);
    o.clip = clip;
    o.color = color;
    o.uv = normal.xy;
    return o;
}

@vertex
fn vs_mark(@location(0) pos: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) color: vec3<f32>) -> Out {
    var o: Out;
    var at = frame.view * draw.model * vec4<f32>(pos, 1.0);
    at = vec4<f32>(at.xy + normal.xy * normal.z, at.zw);
    o.clip = frame.proj * at;
    o.color = color;
    o.uv = normal.xy;
    return o;
}

@vertex
fn vs_shadow(@location(0) pos: vec3<f32>) -> @builtin(position) vec4<f32> {
    return frame.light * draw.model * vec4<f32>(pos, 1.0);
}

fn lit_at(uv: vec2<f32>, z: f32) -> f32 {
    return textureSampleCompareLevel(shadow_map, shadow_cmp, uv, z);
}

// getShadow with PCFSoftShadowMap: a 3 x 3 compare, the outer taps blended by
// where the point sits between texels; then shadow.intensity.
fn shadow_at(sp: vec4<f32>) -> f32 {
    if (frame.shadow.w == 0.0) {
        return 1.0;
    }
    let ndc = sp.xyz / sp.w;
    let coord = vec2<f32>(ndc.x * 0.5 + 0.5, -ndc.y * 0.5 + 0.5);
    let z = ndc.z + frame.shadow.x;
    if (any(coord < vec2<f32>(0.0)) || any(coord > vec2<f32>(1.0)) || z > 1.0) {
        return 1.0;
    }
    let size = vec2<f32>(textureDimensions(shadow_map));
    let t = 1.0 / size;
    let dx = t.x;
    let dy = t.y;
    let f = fract(coord * size + 0.5);
    let uv = coord - f * t;
    let s = lit_at(uv, z) + lit_at(uv + vec2<f32>(dx, 0.0), z) + lit_at(uv + vec2<f32>(0.0, dy), z) + lit_at(uv + t, z)
        + mix(lit_at(uv + vec2<f32>(-dx, 0.0), z), lit_at(uv + vec2<f32>(2.0 * dx, 0.0), z), f.x)
        + mix(lit_at(uv + vec2<f32>(-dx, dy), z), lit_at(uv + vec2<f32>(2.0 * dx, dy), z), f.x)
        + mix(lit_at(uv + vec2<f32>(0.0, -dy), z), lit_at(uv + vec2<f32>(0.0, 2.0 * dy), z), f.y)
        + mix(lit_at(uv + vec2<f32>(dx, -dy), z), lit_at(uv + vec2<f32>(dx, 2.0 * dy), z), f.y)
        + mix(mix(lit_at(uv + vec2<f32>(-dx, -dy), z), lit_at(uv + vec2<f32>(2.0 * dx, -dy), z), f.x), mix(lit_at(uv + vec2<f32>(-dx, 2.0 * dy), z), lit_at(uv + vec2<f32>(2.0 * dx, 2.0 * dy), z), f.x), f.y);
    return mix(1.0, s * (1.0 / 9.0), frame.shadow.z);
}

const RECIPROCAL_PI: f32 = 0.3183098861837907;
// layout.ts: GROUND, STOREY, and a room's height.
const GROUND: f32 = 0.5;
const STOREY: f32 = 4.0;
const ROOM_H: f32 = 3.44;

fn f_schlick(f0: vec3<f32>, f90: f32, dot_vh: f32) -> vec3<f32> {
    let fresnel = exp2((-5.55473 * dot_vh - 6.98316) * dot_vh);
    return f0 * (1.0 - fresnel) + vec3<f32>(f90 * fresnel);
}

fn v_ggx_smith(alpha: f32, dot_nl: f32, dot_nv: f32) -> f32 {
    let a2 = alpha * alpha;
    let gv = dot_nl * sqrt(a2 + (1.0 - a2) * dot_nv * dot_nv);
    let gl = dot_nv * sqrt(a2 + (1.0 - a2) * dot_nl * dot_nl);
    return 0.5 / max(gv + gl, 1e-6);
}

fn d_ggx(alpha: f32, dot_nh: f32) -> f32 {
    let a2 = alpha * alpha;
    let denom = dot_nh * dot_nh * (a2 - 1.0) + 1.0;
    return RECIPROCAL_PI * a2 / (denom * denom);
}

fn brdf_ggx(l: vec3<f32>, v: vec3<f32>, n: vec3<f32>, roughness: f32) -> vec3<f32> {
    let alpha = roughness * roughness;
    let h = normalize(l + v);
    let dot_nl = saturate(dot(n, l));
    let dot_nv = saturate(dot(n, v));
    let dot_nh = saturate(dot(n, h));
    let dot_vh = saturate(dot(v, h));
    return f_schlick(vec3<f32>(0.04), 1.0, dot_vh) * (v_ggx_smith(alpha, dot_nl, dot_nv) * d_ggx(alpha, dot_nh));
}

fn rrt_and_odt(v: vec3<f32>) -> vec3<f32> {
    let a = v * (v + 0.0245786) - 0.000090537;
    let b = v * (0.983729 * v + 0.4329510) + 0.238081;
    return a / b;
}

// three's ACESFilmicToneMapping (the fitted RRT + ODT, columns as in GLSL).
fn aces(color_in: vec3<f32>, exposure: f32) -> vec3<f32> {
    let input = mat3x3<f32>(vec3<f32>(0.59719, 0.07600, 0.02840), vec3<f32>(0.35458, 0.90834, 0.13383), vec3<f32>(0.04823, 0.01566, 0.83777));
    let output = mat3x3<f32>(vec3<f32>(1.60475, -0.10208, -0.00327), vec3<f32>(-0.53108, 1.10813, -0.07276), vec3<f32>(-0.07367, -0.00605, 1.07602));
    var c = color_in * (exposure / 0.6);
    c = input * c;
    c = rrt_and_odt(c);
    c = output * c;
    return saturate(c);
}

// focus.ts: outside the band, dimmer and cooler.
fn fall_back(c: vec3<f32>, y: f32) -> vec3<f32> {
    if (draw.flags.x == 0.0) {
        return c;
    }
    let b = frame.band;
    let inside = smoothstep(b.x - 0.22, b.x, y) * (1.0 - smoothstep(b.y, b.y + 0.22, y));
    return c * mix(vec3<f32>(1.0), vec3<f32>(0.5, 0.54, 0.68), b.z * (1.0 - inside));
}

fn base_of(i: Out) -> vec3<f32> {
    return select(i.color * draw.tint.rgb, draw.tint.rgb, draw.flags.y != 0.0);
}

// The clay, or the glass: lit by the sun and the sky, tone mapped.
@fragment
fn fs_lit(i: Out) -> @location(0) vec4<f32> {
    let n = normalize(i.normal);
    let v = normalize(frame.eye.xyz - i.world);
    let up = (i.world.y - GROUND) - STOREY * floor((i.world.y - GROUND) / STOREY);
    let low = 1.0 - draw.room.w * (1.0 - smoothstep(0.0, 0.85, up));
    let diffuse = base_of(i) * low;
    let l = normalize(frame.sun.xyz);
    let irradiance = saturate(dot(n, l)) * frame.sun_color.rgb * shadow_at(i.shadow_pos);
    let direct = irradiance * RECIPROCAL_PI * diffuse + irradiance * brdf_ggx(l, v, n, draw.rim.w);
    let hemi = mix(frame.ground.rgb, frame.sky.rgb, 0.5 * n.y + 0.5);
    let indirect = hemi * RECIPROCAL_PI * diffuse;
    let flat_ = abs(n.y);
    let edge = pow(1.0 - saturate(dot(n, v)), 2.6) * (1.0 - flat_ * flat_);
    let glow = diffuse * draw.room.rgb * (0.72 + 0.28 * smoothstep(0.0, ROOM_H, up)) + draw.rim.rgb * edge;
    let c = fall_back(aces(direct + indirect + glow, frame.eye.w), i.world.y);
    return vec4<f32>(c * draw.tint.a, draw.tint.a);
}

// What holds its own light, as it is.
@fragment
fn fs_flat(i: Out) -> @location(0) vec4<f32> {
    let c = fall_back(base_of(i), i.world.y);
    return vec4<f32>(c * draw.tint.a, draw.tint.a);
}

// A picture: the sky, the sign's logo, a badge. Premultiplied for blending.
@fragment
fn fs_picture(i: Out) -> @location(0) vec4<f32> {
    let p = textureSample(picture, picture_at, i.uv);
    let a = p.a * draw.tint.a;
    let c = fall_back(p.rgb * draw.tint.rgb, i.world.y);
    return vec4<f32>(c * a, a);
}

// Light thrown: the soft round pool, added.
@fragment
fn fs_light(i: Out) -> @location(0) vec4<f32> {
    let p = textureSample(picture, picture_at, i.uv);
    return vec4<f32>(i.color * p.rgb * p.a * draw.tint.a, 0.0);
}

// A star: a square of light, added.
@fragment
fn fs_sprite(i: Out) -> @location(0) vec4<f32> {
    return vec4<f32>(i.color * draw.tint.a, 0.0);
}

// A rising mark, added: its colour carries the fade and the strength; steam
// (flags.z) is a soft round puff, a byte a square.
@fragment
fn fs_mark(i: Out) -> @location(0) vec4<f32> {
    let round = 1.0 - smoothstep(0.1, 1.0, length(i.uv));
    return vec4<f32>(i.color * select(1.0, round, draw.flags.z != 0.0), 0.0);
}
