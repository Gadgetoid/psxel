# psx

A PlayStation-style renderer in Rust, and a procedurally generated rally game built on it.

The renderer models the PS1 pipeline rather than imitating its look: a fixed-point GTE (4.12 maths, RTPS/RTPT, NCLIP, depth cue), ordering tables, 1 MB of VRAM with 4-bit and 8-bit CLUT textures, and a software rasteriser with affine texturing, the top-left fill rule, dithering, semi-transparency modes and the mask bit. Frames are presented with wgpu, on desktop or in the browser via WebGPU or WebGL. Known departures from hardware are listed in [docs/DEVIATIONS.md](docs/DEVIATIONS.md).

## Demos

The GitHub Pages site builds both web targets on every push to `main`:

- `rally/`: PSX Rally, an endless stage with a co-driver, five biomes, day and night, weather, fords, ravines, tunnels, bridges and twelve period liveries.
- `demo/`: the presenter test card.

Useful URL options for the rally: `?seed=N` picks a stage, `?at=N` starts at a track index, `?stats=1` shows frame timing and location, `?webgl` forces the WebGL backend.

## Building

The toolchain is pinned in `rust-toolchain.toml`.

```sh
cargo run --release -p rally          # native
cargo test --workspace
cd web/rally && trunk build --release  # web build into dist/rally
```

## Layout

- `crates/psx_math`: fixed-point maths and the GTE.
- `crates/psx_prim`: primitives and ordering tables.
- `crates/psx_vram`: VRAM, texture pages, CLUTs and texture windows.
- `crates/psx_soft`: the software rasteriser.
- `crates/psx_gpu`: wgpu presentation.
- `crates/psx_spu`: SPU-ADPCM encoding and decoding.
- `crates/psx_engine`: windowing, input, gamepad and Web Audio.
- `apps/rally`: the rally game. `apps/demo`: the test card.
- `assets`: converted sprites, textures and sounds.

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for more.

## Licence

The code is MIT (see [LICENSE](LICENSE)). Everything in `assets/` is CC0 (see [assets/LICENSE](assets/LICENSE)). The assets are produced from source art and synth patches by the separate psx-asset-tools repository; only the converted results live here.
