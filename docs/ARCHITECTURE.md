# Architecture

- `psx_math`: fixed-point types and a `Gte` mirroring GTE registers and commands (RTPS/RTPT, NCLIP, AVSZ3/4, NCS/NCCS/NCDS, DPCS, depth cue) with flag and saturation behaviour, and UNR division.
- `psx_prim`: GPU primitive packets, PS1-style whole-polygon culling and ordering tables (LIFO buckets, back to front).
- `psx_vram`: 1024x512 BGR555 VRAM, texture pages, 4/8/15-bit texel fetch, CLUTs, texture windows, allocator.
- `psx_soft`: software rasteriser into VRAM: affine interpolation, modulation, dithering, semi-transparency, mask bit, top-left fill rule.
- `psx_gpu`: `Presenter` converts a BGR555 frame to RGBA8 and presents it with nearest-neighbour integer scaling, 4:3 aspect and letterboxing.
- `psx_engine`: window, input (keyboard, mouse, touch drag, web gamepad), fixed 60 Hz tick, BGR555 framebuffer handed to the app.

## Apps

- `apps/demo`: test pattern.
- `apps/rally`: procedural rally game built on the above.

## Web

`cd web/rally && trunk serve --release` (or `web/demo`). Append `?webgl` to force the WebGL2 backend.

## Golden images

`UPDATE_GOLDEN=1 cargo test -p psx_soft` regenerates `tests/golden/*.png`.

## Rally game

- `track`: procedural stage (segments, pace notes, stage characters, crossing-free generation) and a world height function blending the stage profile into 2D noise.
- `landscape`: camera-centred heightfield grid (8 m and 16 m cells, cached) and scattered props; the road is a ribbon drawn over it.
- `art`/`sprites`: user-supplied photo sprites and textures (assets/sprites, built by the sibling psx-asset-tools repo) packed into 4-bit pages with fogged CLUT bands.
- `sound`: Web Audio mix of looped engine/surface/skid/weather samples (psx-asset-tools/sounds) and SPU-ADPCM music (`psx_spu`, psx-asset-tools/music).
