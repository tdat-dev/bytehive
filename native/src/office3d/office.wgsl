// The office's light, first cut: a hemisphere (sky above, warm ground below)
// and one sun, on colours kept on the vertices, then ACES into sRGB.

struct Frame {
    view_proj: mat4x4<f32>,
    model: mat4x4<f32>,
    sun: vec4<f32>,
};
@group(0) @binding(0) var<uniform> frame: Frame;

struct Out {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec3<f32>,
};

@vertex
fn vs(@location(0) pos: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) color: vec3<f32>) -> Out {
    var o: Out;
    let world = frame.model * vec4<f32>(pos, 1.0);
    o.clip = frame.view_proj * world;
    o.normal = (frame.model * vec4<f32>(normal, 0.0)).xyz;
    o.color = color;
    return o;
}

fn aces(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

@fragment
fn fs(i: Out) -> @location(0) vec4<f32> {
    let n = normalize(i.normal);
    let sky = mix(vec3<f32>(0.30, 0.26, 0.22), vec3<f32>(0.62, 0.68, 0.80), n.y * 0.5 + 0.5);
    let sun = max(dot(n, normalize(frame.sun.xyz)), 0.0) * vec3<f32>(1.25, 1.12, 0.95);
    let lit = i.color * (sky + sun);
    // The target is sRGB: the hardware encodes on write.
    return vec4<f32>(aces(lit), 1.0);
}
