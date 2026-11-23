# Architecture

- `psxel_math`: fixed-point types and a `Gte` mirroring GTE registers and commands (RTPS/RTPT, NCLIP, AVSZ3/4, NCS/NCCS/NCDS, DPCS, depth cue) with flag and saturation behaviour, and UNR division.
- `psxel_prim`: GPU primitive packets, PS1-style whole-polygon culling and ordering tables (LIFO buckets, back to front).
- `psxel_vram`: 1024x512 BGR555 VRAM, texture pages, 4/8/15-bit texel fetch, CLUTs, texture windows, allocator.
- `psxel_soft`: software rasteriser into VRAM: affine interpolation, modulation, dithering, semi-transparency, mask bit, top-left fill rule.
- `psxel_gpu`: `Presenter` converts a BGR555 frame to RGBA8 and presents it with nearest-neighbour integer scaling, 4:3 aspect and letterboxing.
- `psxel_engine`: window, input (keyboard, mouse, touch drag, web gamepad), fixed 60 Hz tick, BGR555 framebuffer handed to the app. `Game::display` can request the CRT filter (beam, halation and composite passes in `psxel_gpu::crt`) and a render frame limit (simulation stays at 60 Hz); `option` reads URL query or `--name=value` arguments.

## Apps

- `apps/demo`: test pattern.
- `apps/rally`: procedural rally game built on the above.
- `apps/temple`: jungle temple fly-through. Reuses rally's `render`, `rng` and `sprites` modules via `#[path]`; static geometry is chunked with baked per-vertex lighting and per-torch flicker; water, mist, flames, crystals and light shafts are procedural. `cave` holds the hidden cave behind the waterfall (jittered rock surfaces in their own lighting zone, crystal and shimmer lights); `grotto` is the waterfall's source cave inside a hill on the plateau: indoor pool, ceiling cascades, sunken treasure, carved columns, and a stepped passage through a square doorway to the sinkhole; `ruins` holds the causeway (ending in a blocked jungle tunnel), stelae, walls and fallen blocks (rotated boxes); `crypt` is the flooded vault under the chamber; `secrets` holds the sunken pool treasure and the sinkhole ruin. Billboards sort as one unit at their centre (`force_otz`) and overlays carry a material depth bias, to avoid painter's-order flicker. `--views` renders screenshots from arbitrary cameras.

## Web

`cd web/rally && trunk serve --release` (or `web/demo`). Append `?webgl` to force the WebGL2 backend.

## Golden images

`UPDATE_GOLDEN=1 cargo test -p psxel_soft` regenerates `tests/golden/*.png`.

## Rally game

- `track`: procedural stage (segments, pace notes, stage characters, crossing-free generation) and a world height function blending the stage profile into 2D noise.
- `landscape`: camera-centred heightfield grid (8 m and 16 m cells, cached) and scattered props; the road is a ribbon drawn over it.
- `art`/`sprites`: user-supplied photo sprites and textures (assets/sprites, built by the sibling psxel-asset-tools repo) packed into 4-bit pages with fogged CLUT bands.
- `sound`: Web Audio mix of looped engine/surface/skid/weather samples (psxel-asset-tools/sounds) and SPU-ADPCM music (`psxel_spu`, psxel-asset-tools/music).
