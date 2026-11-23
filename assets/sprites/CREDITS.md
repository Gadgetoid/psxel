# Sprite and texture sources

All sprites and textures are CC0 (see `../LICENSE`). They are converted from source art supplied by the project author with [psxel-asset-tools](../../../psxel-asset-tools) (`sprites/make_sprites.py`). The source art is not shipped.

Variant `_0` comes from the unsuffixed source file, `_1` from `-2`, `_2` from `-3`, `_3` from `-4`.

| Output | Source pattern |
| --- | --- |
| `fir_*` | `douglas_fir_tree_*` |
| `pine_*` | `scott_pine_tree_*` |
| `snow_fir_*` | `snowy_fir_tree_*` |
| `broadleaf_*` | `summer_tree_*` |
| `autumn_*` | `autumn_tree_*` |
| `bare_*` | `half-bare_autumn_tree_*` |
| `joshua_*` | `Joshua_tree_*` |
| `cactus_*` | `cactus_*` |
| `bush_*` | `shrub_*` |
| `scrub_*` | `dry_scrub_*` |
| `rock_*`, `rock_texture` | `rocks_*` |
| `small_rock_*` | `small_rock_*` |
| `snow_rock_*` | `snowy_rock_*` |
| `hay_*` | `hay_bales_*` |
| `road_gravel_*` | `gravel_road_*` |
| `road_sand_*` | `packed_red_sand_road_*` |
| `road_tarmac_*` | `tarmac_with_a_worn_centre_line_*` |
| `road_mud_*` | `muddy_road_with_tyre_ruts_*` |
| `road_snow_*` | `snowy_tarmac_road_with_two_straight_tyre_tracks_*` |
| `road_snow_single` | `snowy_dirt_road_with_two_straight_tyre_tracks_*` |
| `road_single` | `single_track_dirt_road_with_grassy_middle_*` |
| `ground_grass` | `patchy_grass_*` |
| `ground_wheat` | `golden_wheat_stubble_*` |
| `ground_leaves` | `leaf_litter_*` |
| `ground_snow` | `snowy_field_*` |
| `ground_earth` | `desert_grass_*` |
| `ground_riverbed` | `riverbed_*` |
| `verge_dirt` | `grassy_dirt_*` |
| `puddle_mud` | `muddy_puddle_*` |
| `cliff_desert_*` | `horizontally_striated_red_desert_rockscliff_*` |
| `cliff_desert_plain` | `desert_rockscliff_*` |
| `wall_stone` | `drystone_wall_*` |
| `wall_tunnel` | `tunnel_wall_*` |
| `wall_desert` | `desert_wall_*` |
| `wall_concrete` | `weathered_conrete_*` |
| `wall_panels` | `weathered_reinforced_conrete_*` |
| `metal_blue` | `blue_painted_weathered_metal_*` |
| `metal_red` | `red_painted_weathered_metal_*` |
| `barrier` | `race_track_barriers_*` |
| `ice` | `smooth_lake_ice_*` |
| `horizon_forest` | `forested_hills_racing_game_skybox_*` |
| `horizon_farmland` | `rolling_farmland_*` |
| `horizon_mesas` | `red_mesas_*` |
| `horizon_alpine` | `snowy_alpine_peaks_*` |
| `vine_*` | `dense_hanging_jungle_vines_*` |
| `temple_stone_*` | `jungle_temple_stone_*` |
| `jungle_floor_*` | `jungle_floor_*` |
| `bark_*` | `jungle_tree_bark_*` |
| `rockface_*` | `mossy_jungle_rockface_*` |
| `foliage` | `dense_jungle_foliage_*` |
| `carved_face`, `carved_serpent` | panels cropped from `carved_jungle_temple_stone_*` |
| `carved_star` | panel cropped from `carved_jungle_temple_stone_*-2` |
| `carved_sun` | panel cropped from `carved_jungle_temple_stone_*-3` |
| `carved_rosette` | panel cropped from `carved_jungle_temple_stone_*-4` |

These are converted by `jungle/make_rally_jungle.py` in the same tools repo:

| Output | Source pattern |
| --- | --- |
| `treeline_*` | `dense_jungle_trees_full_height_side_on_*` |
| `canopy` | `dense_jungle_canopy_from_below_*` |
| `ground_swamp` | `jungle_swamp_*` |
| `kapok_0` | `jungle_kapok_tree_*` |
| `kapok_1` | `young_jungle_kapok_tree_*` |
| `horizon_jungle` | `misty_karst_peaks_and_jungle_canopy_*` |

`sign_*` are UK traffic sign diagrams (TSRGD 512, 513, 516, 517, 523.1, 524.1, 529.1, 548, 549, 551, 557, 559 and 884) converted from public domain renders on Wikimedia Commons.

Sounds in `../sounds` are rendered with fx-sounds and horn-synthwave by the same tools repo and are also CC0.
