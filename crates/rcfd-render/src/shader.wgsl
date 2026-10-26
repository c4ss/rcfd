// Render core shaders (ADR 0125). Colours are carried as 0..255 floats and written as round(c) / 255,
// so an Rgba8Unorm target stores exactly round(c).

struct Uniforms {
    view_proj: mat4x4<f32>,
    light: vec4<f32>,   // xyz: unit direction to the light (the camera back vector), w: 0 unlit, 1 flat, 2 smooth
    clip: vec4<f32>,    // keep points with dot(xyz, p) <= w
    params: vec4<f32>,  // x: range lo, y: 1 / span, z: clip on (1) / off (0), w: number of colour-map stops
    extra: vec4<f32>,   // x: field present (1) / absent (0)
    stops: array<vec4<f32>, 16>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
// One entry per part: rgb (0..255) and mode (0 hidden, 1 field colour, 2 solid colour).
@group(0) @binding(1) var<storage, read> parts: array<vec4<f32>>;

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) scalar: f32,
    @location(3) @interpolate(flat) face: u32,
    @location(4) @interpolate(flat) part: u32,
};

@vertex
fn vs(
    @location(0) pos: vec3<f32>,
    @location(1) n_flat: vec3<f32>,
    @location(2) n_smooth: vec3<f32>,
    @location(3) scalar: f32,
    @location(4) face: u32,
    @location(5) part: u32,
) -> VOut {
    var o: VOut;
    o.pos = u.view_proj * vec4<f32>(pos, 1.0);
    o.world = pos;
    o.normal = select(n_flat, n_smooth, u.light.w > 1.5);
    o.scalar = scalar;
    o.face = face;
    o.part = part;
    return o;
}

fn cmap(t: f32) -> vec3<f32> {
    let n = u32(u.params.w);
    let x = clamp(t, 0.0, 1.0) * f32(n - 1u);
    let i = min(u32(floor(x)), n - 2u);
    let f = x - f32(i);
    return u.stops[i].xyz * (1.0 - f) + u.stops[i + 1u].xyz * f;
}

// Discard hidden parts and the far side of the cut plane; return the part style.
fn keep(v: VOut) -> vec4<f32> {
    let style = parts[v.part];
    if style.w < 0.5 || (u.params.z > 0.5 && dot(u.clip.xyz, v.world) > u.clip.w) {
        discard;
    }
    return style;
}

@fragment
fn fs_colour(v: VOut) -> @location(0) vec4<f32> {
    let style = keep(v);
    var c = vec3<f32>(200.0, 200.0, 200.0);
    if style.w > 1.5 {
        c = style.xyz;
    } else if u.extra.x > 0.5 {
        c = round(cmap((v.scalar - u.params.x) * u.params.y));
    }
    if u.light.w > 0.5 {
        c = c * (0.25 + 0.75 * abs(dot(normalize(v.normal), u.light.xyz)));
    }
    return vec4<f32>(round(c) / 255.0, 1.0);
}

struct PickOut {
    @location(0) id: u32,
    @location(1) value: f32,
};

@fragment
fn fs_pick(v: VOut) -> PickOut {
    _ = keep(v);
    return PickOut(v.face + 1u, v.scalar);
}
