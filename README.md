# psxel

A PlayStation-style renderer in Rust, and a procedurally generated rally game built on it.

The renderer models the PS1 pipeline rather than imitating its look: a fixed-point GTE (4.12 maths, RTPS/RTPT, NCLIP, depth cue), ordering tables, 1 MB of VRAM with 4-bit and 8-bit CLUT textures, and a software rasteriser with affine texturing, the top-left fill rule, dithering, semi-transparency modes and the mask bit. Frames are presented with wgpu, on desktop or in the browser via WebGPU or WebGL. Known departures from hardware are listed in [docs/DEVIATIONS.md](docs/DEVIATIONS.md).

## Demos

The GitHub Pages site builds every web target on each push to `main`:

- `rally/`: PSX Rally, an endless stage with a co-driver, five biomes, day and night, weather, fords, ravines, tunnels, bridges and twelve period liveries.
- `temple/`: a looping fly-through of a jungle temple with a waterfall, pool, a secret crystal cave behind the falls, the cave the falls pour from, with an indoor pool fed by falls from the ceiling, sunken treasure and ruins, and a passage to a jade idol ruin in a sinkhole, a blocked jungle tunnel at the end of the causeway, a torch-lit idol chamber with a flooded crypt beneath, and a shrine on top lit by braziers; a sunken treasure waits in the pool. Space pauses, C toggles a free camera (WASD, arrows, Q/E), V toggles the CRT filter and G the short draw-distance fog (both on by default), F toggles a period 20 fps frame limit, M mutes. `?crt=0`, `?fog=0` and `?fps=20` (or `--crt=0`, `--fog=0`, `--fps=20` natively) set them at start.
- `demo/`: the presenter test card.

Useful URL options for the rally: `?seed=N` picks a stage, `?at=N` starts at a track index, `?stats=1` shows frame timing and location, `?webgl` forces the WebGL backend.

## Building

The toolchain is pinned in `rust-toolchain.toml`.

```sh
cargo run --release -p rally          # native
cargo test --workspace
cd web/rally && trunk build --release  # web build into dist/rally
cargo run --release -p temple -- --shots out  # temple tour screenshots
```

## Layout

- `crates/psxel_math`: fixed-point maths and the GTE.
- `crates/psxel_prim`: primitives and ordering tables.
- `crates/psxel_vram`: VRAM, texture pages, CLUTs and texture windows.
- `crates/psxel_soft`: the software rasteriser.
- `crates/psxel_gpu`: wgpu presentation.
- `crates/psxel_spu`: SPU-ADPCM encoding and decoding.
- `crates/psxel_engine`: windowing, input, gamepad and Web Audio.
- `apps/rally`: the rally game. `apps/temple`: the jungle temple fly-through. `apps/demo`: the test card.
- `assets`: converted sprites, textures and sounds.

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for more.

## Licence

The code is MIT (see [LICENSE](LICENSE)). Everything in `assets/` is CC0 (see [assets/LICENSE](assets/LICENSE)). The assets are produced from source art and synth patches by the separate psxel-asset-tools repository; only the converted results live here.
