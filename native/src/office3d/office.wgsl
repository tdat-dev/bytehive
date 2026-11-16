// The office's one matte surface, as three.js r170 draws its clay (clay.ts):
// MeshStandardMaterial (roughness 0.82, metalness 0) lit by a hemisphere and
// one sun, with clay's three additions: a glow of the room's own colour, a
// soft edge light, and a darkening toward each storey's floor. Then three's
// ACES Filmic curve at the scene's exposure; the sRGB target encodes on write.

struct Frame {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    // Towards the sun, w unused; the sun's colour times its intensity.
    sun: vec4<f32>,
    sun_color: vec4<f32>,
    // The hemisphere's sky and ground colours times its intensity.
    sky: vec4<f32>,
    ground: vec4<f32>,
    // Where the camera stands; w = tone mapping exposure.
    eye: vec4<f32>,
    // Clay: the room's glow (rgb), the edge light (rgb), then
    // (foot, GROUND, STOREY, STOREY - SLAB).
    room: vec4<f32>,
    rim: vec4<f32>,
    storey: vec4<f32>,
};
@group(0) @binding(0) var<uniform> frame: Frame;

struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) world: vec3<f32>,
};

@vertex
fn vs(@location(0) pos: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) color: vec3<f32>) -> Out {
    var o: Out;
    let world = frame.model * vec4<f32>(pos, 1.0);
    o.clip = frame.view_proj * world;
    o.normal = (frame.model * vec4<f32>(normal, 0.0)).xyz;
    o.color = color;
    o.world = world.xyz;
    return o;
}

const RECIPROCAL_PI: f32 = 0.3183098861837907;
const ROUGHNESS: f32 = 0.82;

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

fn brdf_ggx(l: vec3<f32>, v: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    let alpha = ROUGHNESS * ROUGHNESS;
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

@fragment
fn fs(i: Out) -> @location(0) vec4<f32> {
    let n = normalize(i.normal);
    let v = normalize(frame.eye.xyz - i.world);
    // Clay: darker toward the foot of each storey (free occlusion for every leg).
    let up = (i.world.y - frame.storey.y) - frame.storey.z * floor((i.world.y - frame.storey.y) / frame.storey.z);
    let low = 1.0 - frame.storey.x * (1.0 - smoothstep(0.0, 0.85, up));
    let diffuse = i.color * low;
    // The sun, then the hemisphere, as RE_Direct and RE_IndirectDiffuse.
    let l = normalize(frame.sun.xyz);
    let irradiance = saturate(dot(n, l)) * frame.sun_color.rgb;
    let direct = irradiance * RECIPROCAL_PI * diffuse + irradiance * brdf_ggx(l, v, n);
    let hemi = mix(frame.ground.rgb, frame.sky.rgb, 0.5 * n.y + 0.5);
    let indirect = hemi * RECIPROCAL_PI * diffuse;
    // Clay's own light and the edge light, added as emission.
    let flat_ = abs(n.y);
    let edge = pow(1.0 - saturate(dot(n, v)), 2.6) * (1.0 - flat_ * flat_);
    let glow = diffuse * frame.room.rgb * (0.72 + 0.28 * smoothstep(0.0, frame.storey.w, up)) + frame.rim.rgb * edge;
    return vec4<f32>(aces(direct + indirect + glow, frame.eye.w), 1.0);
}
