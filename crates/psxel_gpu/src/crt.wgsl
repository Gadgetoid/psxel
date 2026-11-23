struct Params {
    viewport_origin: vec2<f32>,
    viewport_size: vec2<f32>,
    source_size: vec2<f32>,
    effect: vec2<f32>,
}

@group(0) @binding(0) var primary: texture_2d<f32>;
@group(0) @binding(1) var<uniform> params: Params;
@group(0) @binding(2) var smooth_sampler: sampler;
@group(0) @binding(3) var secondary: texture_2d<f32>;

const CRT_GAMMA: f32 = 2.4;
const DISPLAY_GAMMA: f32 = 2.2;
const STORE_GAMMA: f32 = 2.0;
const CONVERGENCE: vec3<f32> = vec3<f32>(0.12, 0.0, -0.12);
const BEAM_MIN: vec3<f32> = vec3<f32>(0.30, 0.28, 0.32);
const BEAM_MAX: vec3<f32> = vec3<f32>(0.62, 0.58, 0.66);
const HALATION: f32 = 0.11;
const DIFFUSION: f32 = 0.06;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let x = f32((index << 1u) & 2u) * 2.0 - 1.0;
    let y = f32(index & 2u) * 2.0 - 1.0;
    return vec4<f32>(x, y, 0.0, 1.0);
}

fn source_linear(texel: vec2<f32>) -> vec3<f32> {
    let clamped = vec2<i32>(clamp(texel, vec2<f32>(0.0), params.source_size - 1.0));
    return pow(textureLoad(primary, clamped, 0).rgb, vec3<f32>(CRT_GAMMA));
}

fn store(color: vec3<f32>) -> vec4<f32> {
    return vec4<f32>(pow(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(1.0 / STORE_GAMMA)), 1.0);
}

fn load_stored(color: vec4<f32>) -> vec3<f32> {
    return pow(color.rgb, vec3<f32>(STORE_GAMMA));
}

fn row_color(row: f32, x: f32) -> vec3<f32> {
    var color = vec3<f32>(0.0);
    for (var channel = 0; channel < 3; channel = channel + 1) {
        let position = x + CONVERGENCE[channel];
        let base = floor(position);
        var sum = 0.0;
        var total = 0.0;
        for (var offset = -1; offset <= 2; offset = offset + 1) {
            let distance = base + f32(offset) - position;
            let weight = exp(-distance * distance / 0.5);
            sum = sum + source_linear(vec2<f32>(base + f32(offset), row))[channel] * weight;
            total = total + weight;
        }
        color[channel] = sum / total;
    }
    return color;
}

fn beam(color: vec3<f32>, distance: f32) -> vec3<f32> {
    let width = mix(BEAM_MIN, BEAM_MAX, pow(color, vec3<f32>(0.6)));
    let spread = vec3<f32>(distance) / width;
    return color * exp(-spread * spread) * (0.55 / width);
}

@fragment
fn fs_beam(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let relative = position.xy / params.viewport_size;
    let coordinate = relative * params.source_size - vec2<f32>(0.0, 0.5);
    let row = floor(coordinate.y);
    let fraction = coordinate.y - row;
    let x = coordinate.x - 0.5;
    var color = vec3<f32>(0.0);
    for (var offset = -1; offset <= 2; offset = offset + 1) {
        let line = row + f32(offset);
        color = color + beam(row_color(line, x), f32(offset) - fraction);
    }
    return store(color * 0.62);
}

fn blur(position: vec2<f32>, direction: vec2<f32>, linear_input: bool) -> vec4<f32> {
    var sum = vec3<f32>(0.0);
    var total = 0.0;
    for (var offset = -6; offset <= 6; offset = offset + 1) {
        let weight = exp(-f32(offset * offset) / 18.0);
        let texel = floor(position) + direction * f32(offset);
        var color: vec3<f32>;
        if (linear_input) {
            color = source_linear(texel);
        } else {
            let clamped = vec2<i32>(clamp(texel, vec2<f32>(0.0), params.source_size - 1.0));
            color = load_stored(textureLoad(primary, clamped, 0));
        }
        sum = sum + color * weight;
        total = total + weight;
    }
    return store(sum / total);
}

@fragment
fn fs_blur_horizontal(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    return blur(position.xy, vec2<f32>(1.0, 0.0), true);
}

@fragment
fn fs_blur_vertical(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    return blur(position.xy, vec2<f32>(0.0, 1.0), false);
}

fn warp(relative: vec2<f32>) -> vec2<f32> {
    let centered = relative * 2.0 - 1.0;
    let radius = dot(centered, centered);
    let distortion = 1.0 + radius * (0.045 + radius * 0.012);
    let scale = 1.0 + 0.045 + 0.012 * 4.0;
    return centered * distortion / scale;
}

fn screen_edge(curved: vec2<f32>, pixel: vec2<f32>) -> f32 {
    let corner = 0.06;
    let inner = abs(curved) - (vec2<f32>(1.0) - corner);
    let distance = length(max(inner, vec2<f32>(0.0))) + min(max(inner.x, inner.y), 0.0) - corner;
    let soft = max(pixel.x, pixel.y) * 1.5;
    return 1.0 - smoothstep(-soft, soft, distance);
}

fn phosphor_mask(position: vec2<f32>, scale: f32) -> vec3<f32> {
    if (scale < 3.0) {
        return vec3<f32>(1.0);
    }
    let column = u32(position.x);
    let triad = column / 3u;
    let lit = column % 3u;
    var mask = vec3<f32>(0.32);
    mask[lit] = 1.0;
    if (scale >= 5.0) {
        let row = (u32(position.y) + (triad % 2u) * 2u) % 4u;
        if (row == 3u) {
            mask = mask * 0.45;
        }
    }
    return mask;
}

fn mask_average(scale: f32) -> f32 {
    if (scale < 3.0) {
        return 1.0;
    }
    let grille = (1.0 + 0.32 * 2.0) / 3.0;
    if (scale >= 5.0) {
        return grille * (3.0 + 0.45) / 4.0;
    }
    return grille;
}

@fragment
fn fs_composite(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let relative = (position.xy - params.viewport_origin) / params.viewport_size;
    let curved = warp(relative);
    let pixel = 2.0 / params.viewport_size;
    let edge = screen_edge(curved, pixel);
    if (edge <= 0.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }
    let uv = curved * 0.5 + 0.5;
    let scanlines = load_stored(textureSampleLevel(primary, smooth_sampler, uv, 0.0));
    let glow = load_stored(textureSampleLevel(secondary, smooth_sampler, uv, 0.0));
    let scale = params.viewport_size.y / params.source_size.y;
    let average = mask_average(scale);
    let masked = scanlines * phosphor_mask(position.xy, scale) / average;
    var color = mix(masked, glow, DIFFUSION) + glow * HALATION;
    let vignette = 1.0 - dot(curved * curved, vec2<f32>(0.08, 0.12));
    color = color * vignette * edge;
    let encoded = pow(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(1.0 / DISPLAY_GAMMA));
    return vec4<f32>(encoded, 1.0);
}
