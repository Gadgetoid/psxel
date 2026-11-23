struct Params {
    viewport_origin: vec2<f32>,
    viewport_size: vec2<f32>,
    source_size: vec2<f32>,
    _effect: vec2<f32>,
}

@group(0) @binding(0) var source: texture_2d<f32>;
@group(0) @binding(1) var<uniform> params: Params;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let x = f32((index << 1u) & 2u) * 2.0 - 1.0;
    let y = f32(index & 2u) * 2.0 - 1.0;
    return vec4<f32>(x, y, 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let relative = (position.xy - params.viewport_origin) / params.viewport_size;
    let texel = vec2<i32>(clamp(floor(relative * params.source_size), vec2<f32>(0.0), params.source_size - 1.0));
    return textureLoad(source, texel, 0);
}
