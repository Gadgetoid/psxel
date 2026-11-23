# Deviations

Where this engine knowingly differs from PS1 hardware, or from PLAN.md where psx-spx disagrees with it.

| Area | Deviation | Reason |
|---|---|---|
| Rasteriser interpolation | Colour and UV gradients use 12-bit fixed point plane equations. Colours get a half-step rounding bias, UVs truncate. Not verified bit-exact against hardware. | Exact Mednafen/DuckStation stepping not yet reimplemented from psx-spx. |
| Dithering | Applied to Gouraud and modulated textured polygons and Gouraud lines. Never to sprites or raw-textured polygons. | psx-spx describes the rule loosely; this matches common emulator behaviour. |
| Quads | Split into triangles 0-1-2 and 1-2-3, each checked against the 1023x511 size limit separately. | Matches emulator behaviour; whole-quad rejection not confirmed. |
| GPU rasteriser (M4) | Not implemented yet. Apps render with `psxel_soft` and present via `psxel_gpu::Presenter`. | Software path was needed first for exactness and the rally game. |
| Sine table | `sin12` is generated at startup with f64 sin, rounded to 4.12. | Equivalent to a precomputed table; not in the raster path. |
| Texture window | `Texture::window` lets a primitive carry its own texture window, overriding `DrawEnv::window`. | Equivalent to inserting a GP0(E2) command before that primitive in the OT; avoids modelling state packets. |
| Photo sprites | Billboards use 8-bit CLUT textures in pages 8+ with per-distance fogged CLUT copies. | CLUT-based fog, a period technique, because texture modulation cannot brighten towards fog colour. |
