//! Built-in animated shaders rendered on the GPU.
//!
//! Each shader is a WGSL body that defines
//! `fn shader_image(frag_coord, resolution, time, colors) -> vec3<f32>` and is
//! wrapped in the shared prelude/epilogue below, which provide a fullscreen
//! triangle and a uniform block with the output size, elapsed time and the
//! configured palette (up to [`coconut_core::MAX_SHADER_COLORS`] slots).
//! Registering a new [`BuiltinShader`] only takes adding its body and a match
//! arm in [`wgsl`].

use coconut_core::BuiltinShader;

const PRELUDE: &str = r#"
struct Globals {
    resolution: vec2<f32>,
    time: f32,
    aspect: f32,
    colors: array<vec4<f32>, 4>,
};

@group(0) @binding(0) var<uniform> globals: Globals;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    return vec4<f32>(positions[index], 0.0, 1.0);
}
"#;

const EPILOGUE: &str = r#"
@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    return vec4<f32>(
        shader_image(position.xy, globals.resolution, globals.time, globals.colors),
        1.0,
    );
}
"#;

/// A layered vortex: every iteration inverts the point around a drifting
/// center and deposits a colored filament where the fold passes the origin.
/// `colors[0]` is the backdrop, `colors[1]`/`colors[2]` the two accents the
/// filaments cycle between.
const SWIRL: &str = r#"
fn swirl_field(p: vec2<f32>, time: f32, colors: array<vec4<f32>, 4>) -> vec3<f32> {
    var z = p;
    var color = colors[0].rgb;
    var strength = 1.0;
    for (var i = 0; i < 40; i = i + 1) {
        let layer = f32(i);
        let seed = fract(sin(layer * 12.9898) * 43758.5453);
        let rate = 0.3 + 0.6 * seed;
        let center = 0.55 * vec2<f32>(
            cos(rate * time + layer * 1.93),
            sin(rate * time + layer * 2.41),
        );
        let d = z - center;
        let r = max(length(d), 0.0001);
        let a = atan2(d.y, d.x) + 0.55 * r;
        z = center + vec2<f32>(cos(a), sin(a)) / r;
        let tint = mix(
            colors[1].rgb,
            colors[2].rgb,
            0.5 + 0.5 * cos(6.2831853 * (layer * 0.09 + time * 0.05)),
        );
        color += strength * exp(-16.0 * dot(z, z)) * tint;
        strength *= 0.94;
    }
    return color;
}

fn shader_image(
    frag_coord: vec2<f32>,
    resolution: vec2<f32>,
    time: f32,
    colors: array<vec4<f32>, 4>,
) -> vec3<f32> {
    let p = (2.0 * frag_coord - resolution) / min(resolution.x, resolution.y);
    var color = swirl_field(p * 1.5, time * 0.06 + 30.0, colors);
    color = 1.0 - exp(-color * 0.85);
    color *= color;
    let q = frag_coord / resolution;
    color *= 0.3 + 0.7 * pow(16.0 * q.x * q.y * (1.0 - q.x) * (1.0 - q.y), 0.25);
    return color;
}
"#;

/// Calm tropical water: domain-warped simplex noise mixed between the
/// configured deep (`colors[0]`), lagoon (`colors[1]`) and shallow
/// (`colors[2]`) tones, with a fresnel rim and thin crest highlights.
const WAVES: &str = r#"
fn mod289_v3(x: vec3<f32>) -> vec3<f32> {
    return x - floor(x * (1.0 / 289.0)) * 289.0;
}

fn mod289_v4(x: vec4<f32>) -> vec4<f32> {
    return x - floor(x * (1.0 / 289.0)) * 289.0;
}

fn permute4(x: vec4<f32>) -> vec4<f32> {
    return mod289_v4((x * 34.0 + vec4<f32>(1.0)) * x);
}

fn taylor_inv_sqrt(r: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(1.79284291400159) - 0.85373472095314 * r;
}

fn snoise(v: vec3<f32>) -> f32 {
    let c = vec2<f32>(1.0 / 6.0, 1.0 / 3.0);
    let d = vec4<f32>(0.0, 0.5, 1.0, 2.0);
    var i = floor(v + vec3<f32>(dot(v, c.yyy)));
    let x0 = v - i + vec3<f32>(dot(i, c.xxx));
    let g = step(x0.yzx, x0.xyz);
    let l = vec3<f32>(1.0) - g;
    let i1 = min(g.xyz, l.zxy);
    let i2 = max(g.xyz, l.zxy);
    let x1 = x0 - i1 + c.xxx;
    let x2 = x0 - i2 + c.yyy;
    let x3 = x0 - d.yyy;
    i = mod289_v3(i);
    let p = permute4(permute4(permute4(
        i.z + vec4<f32>(0.0, i1.z, i2.z, 1.0))
        + i.y + vec4<f32>(0.0, i1.y, i2.y, 1.0))
        + i.x + vec4<f32>(0.0, i1.x, i2.x, 1.0));
    let n_ = 0.142857142857;
    let ns = n_ * d.wyz - d.xzx;
    let j = p - 49.0 * floor(p * ns.z * ns.z);
    let x_ = floor(j * ns.z);
    let y_ = floor(j - 7.0 * x_);
    let x = x_ * ns.x + ns.yyyy;
    let y = y_ * ns.x + ns.yyyy;
    let h = vec4<f32>(1.0) - abs(x) - abs(y);
    let b0 = vec4<f32>(x.xy, y.xy);
    let b1 = vec4<f32>(x.zw, y.zw);
    let s0 = floor(b0) * 2.0 + vec4<f32>(1.0);
    let s1 = floor(b1) * 2.0 + vec4<f32>(1.0);
    let sh = -step(h, vec4<f32>(0.0));
    let a0 = b0.xzyw + s0.xzyw * sh.xxyy;
    let a1 = b1.xzyw + s1.xzyw * sh.zzww;
    var p0 = vec3<f32>(a0.xy, h.x);
    var p1 = vec3<f32>(a0.zw, h.y);
    var p2 = vec3<f32>(a1.xy, h.z);
    var p3 = vec3<f32>(a1.zw, h.w);
    let norm = taylor_inv_sqrt(vec4<f32>(dot(p0, p0), dot(p1, p1), dot(p2, p2), dot(p3, p3)));
    p0 = p0 * norm.x;
    p1 = p1 * norm.y;
    p2 = p2 * norm.z;
    p3 = p3 * norm.w;
    var m = max(
        vec4<f32>(0.6) - vec4<f32>(dot(x0, x0), dot(x1, x1), dot(x2, x2), dot(x3, x3)),
        vec4<f32>(0.0),
    );
    m = m * m;
    return 42.0 * dot(m * m, vec4<f32>(dot(p0, x0), dot(p1, x1), dot(p2, x2), dot(p3, x3)));
}

fn fbm(p: vec3<f32>) -> f32 {
    var value = 0.0;
    var amp = 0.5;
    var freq = 1.0;
    for (var i = 0; i < 4; i = i + 1) {
        value += amp * snoise(p * freq);
        freq *= 2.0;
        amp *= 0.5;
    }
    return value * 0.5 + 0.5;
}

fn warp(p: vec2<f32>, off: vec2<f32>, t: f32) -> f32 {
    return fbm(vec3<f32>(p + off, t));
}

struct WaterPattern {
    color: vec3<f32>,
    field: f32,
    grad: vec2<f32>,
};

fn water_pattern(p: vec2<f32>, t: f32, colors: array<vec4<f32>, 4>) -> WaterPattern {
    let s = 4.5;
    let q = vec2<f32>(
        warp(p, vec2<f32>(0.0, 0.0), t),
        warp(p, vec2<f32>(5.2, 1.3), t),
    );
    let r = vec2<f32>(
        warp(p, vec2<f32>(s * q.x + 1.7, s * q.y + 9.2), t),
        warp(p, vec2<f32>(s * q.x + 8.3, s * q.y + 2.8), t),
    );
    let result = warp(p, vec2<f32>(s * r.x, s * r.y), t * 1.2 + 10.0);
    let m1 = length(q);
    let m2 = length(r);
    let deep_sea = colors[0].rgb;
    let lagoon = colors[1].rgb;
    let shallows = colors[2].rgb;
    let water_col = mix(
        lagoon,
        shallows,
        0.5 + 0.5 * cos(6.2831853 * (result * 0.6 + m1 * 0.25 + m2 * 0.15)),
    );
    let depth_mix = smoothstep(0.25, 0.75, result * (m1 + m2) * 0.85);
    var col = mix(deep_sea, lagoon, depth_mix);
    col = mix(col, shallows, smoothstep(0.7, 0.95, result * m2) * 0.6);
    col = mix(col, water_col, 0.35);
    return WaterPattern(clamp(col, vec3<f32>(0.0), vec3<f32>(1.0)), result, r);
}

fn shader_image(
    frag_coord: vec2<f32>,
    resolution: vec2<f32>,
    time: f32,
    colors: array<vec4<f32>, 4>,
) -> vec3<f32> {
    let uv = frag_coord / resolution.y;
    var p = uv * (resolution.y / 560.0);
    let t = time * 0.012;
    p.x += t * 0.5;
    let pat = water_pattern(p, t, colors);
    var col = pat.color;
    let slope = clamp(length(pat.grad), 0.0, 1.0);
    let fresnel = pow(slope, 1.6);
    col += vec3<f32>(0.55, 0.85, 0.95) * fresnel * 0.18;
    let refr_field = fbm(vec3<f32>((p + pat.grad * 0.04) * 1.0, t * 0.8));
    col = mix(col, col * (0.85 + refr_field * 0.5), 0.25);
    let crest = smoothstep(0.62, 0.98, pat.field * length(pat.grad));
    col += vec3<f32>(0.6, 0.9, 0.95) * crest * 0.12;
    col = pow(col, vec3<f32>(0.92));
    col = mix(col, col * vec3<f32>(0.96, 1.04, 1.08), 0.5);
    return clamp(col, vec3<f32>(0.0), vec3<f32>(1.0));
}
"#;

/// The complete WGSL module for `shader`, ready for
/// [`wgpu::Device::create_shader_module`].
pub fn wgsl(shader: BuiltinShader) -> String {
    let body = match shader {
        BuiltinShader::Swirl => SWIRL,
        BuiltinShader::Waves => WAVES,
    };
    format!("{PRELUDE}{body}{EPILOGUE}")
}
