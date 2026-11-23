import argparse
import glob
import os
import sys

import numpy as np
from PIL import Image, ImageEnhance, ImageFilter
from scipy import ndimage

OUTPUT_DIR = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "assets", "sprites"))

TREE = dict(base="trunk", fringe_level=215, fringe_passes=2, hole_level=236, hole_chroma=14, hole_min_size=4)
SOLID = dict(base="shadow", fringe_level=200, fringe_passes=40, hole_level=236, hole_chroma=14, hole_min_size=4)
CACTUS = dict(base="trunk", fringe_level=190, fringe_passes=40, hole_level=236, hole_chroma=14, hole_min_size=4)
SNOWY = dict(base="trunk", fringe_level=235, fringe_passes=1, hole_level=248, hole_chroma=5, hole_min_size=40)
SNOWY_ROCK = dict(base="pale", fringe_level=235, fringe_passes=1, hole_level=248, hole_chroma=5, hole_min_size=40)

SPRITES = [
    ("broadleaf", "summer_tree_*", (64, 128), TREE, None),
    ("autumn", "autumn_tree_*", (64, 128), TREE, None),
    ("bare", "half-bare_autumn_tree_*", (64, 128), TREE, None),
    ("fir", "douglas_fir_tree_*", (64, 128), TREE, None),
    ("pine", "scott_pine_tree_*", (64, 128), TREE, None),
    ("snow_fir", "snowy_fir_tree_*", (64, 128), SNOWY, None),
    ("joshua", "Joshua_tree_*", (64, 128), TREE, None),
    ("cactus", "cactus_*", (32, 64), CACTUS, None),
    ("bush", "shrub_*", (64, 48), SOLID, None),
    ("scrub", "dry_scrub_*", (64, 48), SOLID, None),
    ("rock", "rocks_*", (64, 64), SOLID, None),
    ("small_rock", "small_rock_*", (32, 32), SOLID, None),
    ("snow_rock", "snowy_rock_*", (64, 64), SNOWY_ROCK, None),
]

ROAD_WIDTH_METRES = 9.0
ROAD_LENGTH_METRES = 8.0
GROUND_METRES = 8.0
WHEEL_TRACK_OFFSETS_METRES = (-2.95, -1.45, 1.45, 2.95)

ROADS = [
    ("road_gravel", "gravel_road_*", dict(kind="surface", source_metres=1.6, patch_metres=0.5, tracks=0.45, detail=1.0, macro=0.05)),
    ("road_sand", "packed_red_sand_road_*", dict(kind="surface", source_metres=2.0, patch_metres=0.8, tracks=0.3, detail=0.8, macro=0.07)),
    ("road_tarmac", "tarmac_with_a_worn_centre_line_*", dict(kind="tarmac", source_metres=6.4, patch_metres=1.0, detail=0.6, macro=0.04)),
]

SNOW_BASE_SOURCE = "snowy_field_*"
SNOW_BASE_METRES = 2.0
RUT_DEPTH = 0.22
RUT_CORE_METRES = 0.11

GROUNDS = [
    ("ground_grass", "patchy_grass_*", 2, dict(source_metres=4.0, patch_metres=1.2, rotate=True, detail=0.8, macro=0.1)),
    ("ground_wheat", "golden_wheat_stubble_*", 0, dict(source_metres=1.0, patch_metres=0.6, rotate=False, detail=0.8, macro=0.07)),
    ("ground_leaves", "leaf_litter_*", 0, dict(source_metres=0.6, patch_metres=0.35, rotate=True, detail=0.9, macro=0.07)),
    ("ground_earth", "desert_grass_*", 0, dict(source_metres=3.0, patch_metres=1.0, rotate=True, detail=0.8, macro=0.07)),
    ("ground_snow", "snowy_field_*", 0, dict(source_metres=2.0, patch_metres=1.0, rotate=True, detail=0.6, macro=0.03)),
]

LATE_SPRITES = [
    ("hay", "hay_bales_*", (64, 48), SOLID, None),
]

LATE_TEXTURES = [
    ("wall_stone", "drystone_wall_*", 1, dict(kind="direct", overlap=0.1, detail=0.8)),
    ("wall_tunnel", "tunnel_wall_*", 2, dict(kind="direct", overlap=0.1, detail=0.8)),
    ("ground_riverbed", "riverbed_*", 2, dict(kind="direct", overlap=0.1, detail=0.8)),
    ("wall_desert", "desert_wall_*", 1, dict(kind="direct", overlap=0.1, detail=0.8)),
    ("wall_concrete", "weathered_conrete_*", 0, dict(kind="direct", overlap=0.1, detail=0.8)),
    ("wall_panels", "weathered_reinforced_conrete_*", 0, dict(kind="direct", overlap=0.1, detail=0.8)),
    ("metal_blue", "blue_painted_weathered_metal_*", 2, dict(kind="direct", overlap=0.1, detail=0.8)),
    ("metal_red", "red_painted_weathered_metal_*", 2, dict(kind="direct", overlap=0.1, detail=0.8)),
    ("road_single", "single_track_dirt_road_*", 0, dict(kind="direct", overlap=0.1, detail=0.8)),
    ("verge_dirt", "grassy_dirt_*", 1, dict(kind="direct", overlap=0.1, detail=0.8)),
    ("puddle_mud", "muddy_puddle_*", 0, dict(kind="direct", overlap=0.1, detail=0.8)),
    ("road_snow_single", "snowy_dirt_road_with_two_straight_tyre_tracks_*", 0, dict(kind="direct", overlap=0.1, detail=0.8, crop=(0.2, 0.0, 0.8, 1.0))),
    ("road_snow_0", "snowy_tarmac_road_with_two_straight_tyre_tracks_*", 0, dict(kind="direct", overlap=0.1, detail=0.7)),
    ("road_snow_1", "snowy_tarmac_road_with_two_straight_tyre_tracks_*", 1, dict(kind="direct", overlap=0.1, detail=0.7, straighten=True)),
    ("barrier", "race_track_barriers_*", 0, dict(kind="direct", overlap=0.0, detail=0.9, crop=(0.0, 0.0, 1.0, 0.25), size=(64, 16))),
]

LATE_ROADS = [
    ("road_mud", "muddy_road_with_tyre_ruts_*", dict(kind="direct", overlap=0.15, detail=0.8)),
]

ROCK_TEXTURE_SOURCE = "rocks_*"
ROCK_TEXTURE_VARIANT = 0
ROCK_TEXTURE_BOX = (0.36, 0.42, 0.66, 0.62)
ROCK_TEXTURE_SETTINGS = dict(source_metres=3.5, patch_metres=0.45, rotate=True, detail=0.7)
ROCK_TEXTURE_METRES = 2.0

CLIFFS = [
    ("cliff_desert", "horizontally_striated_red_desert_rockscliff_*", dict(repeats=1, overlap=0.2, detail=0.8)),
]
CLIFF_PLAIN = ("cliff_desert_plain", "desert_rockscliff_*", dict(source_metres=3.0, patch_metres=1.0, rotate=True, detail=0.8, macro=0.05))

HORIZONS = [
    ("horizon_forest", "forested_hills_racing_game_skybox_*", (200, 335, 1020, 540), None),
    ("horizon_mesas", "red_mesas_*", (140, 365, 1040, 590), dict(sky_columns=(780, 830), sky_bottom=415, threshold=30)),
    ("horizon_farmland", "rolling_farmland_*", (120, 375, 1120, 625), dict(sky_columns=(600, 700), sky_bottom=405, threshold=34)),
    ("horizon_alpine", "snowy_alpine_peaks_*", (90, 258, 1050, 498), dict(sky_columns=(330, 380), sky_bottom=380, threshold=30, cloud_texture=7)),
]
HORIZON_SIZE = (256, 64)
HORIZON_COLOURS = 15
HORIZON_FADE_COLUMNS = 6

WORK_SCALE = 8
TEXTURE_COLOURS = 16
TEXTURE_SEED = 1997
MACRO_CYCLES = 3.0

BACKGROUND_MIN_LEVEL = 244
BACKGROUND_MAX_CHROMA = 8
FRINGE_MAX_CHROMA = 24
ISLAND_MIN_FRACTION = 0.004
MOUND_WIDTH_FACTOR = 1.6
MOUND_KEEP_PIXELS = 2
EDGE_INSET = 4
PALE_BASE_LEVEL = 170
PALE_BASE_CHROMA = 40
PALE_BASE_FRACTION = 0.4
CONTRAST = 1.12
SATURATION = 1.18
SPRITE_COLOURS = 15
SPRITE_DITHER = Image.Dither.NONE
TEXTURE_CONTRAST = 1.25
TEXTURE_SATURATION = 1.15


def sort_key(path):
    stem = os.path.splitext(os.path.basename(path))[0]
    suffix = stem.rsplit("-", 1)[-1]
    return int(suffix) if suffix.isdigit() else 1


def find_sources(source_dir, pattern, variants):
    sources = sorted(glob.glob(os.path.join(source_dir, pattern + ".jpg")), key=sort_key)
    if variants is not None:
        sources = [sources[index] for index in variants]
    return sources


def is_pale(rgb, min_level, max_chroma):
    channels = rgb.astype(np.int16)
    lowest = channels.min(axis=2)
    chroma = channels.max(axis=2) - lowest
    return (lowest >= min_level) & (chroma <= max_chroma)


def flood_from_border(candidate):
    labels, _ = ndimage.label(candidate)
    border = np.concatenate([labels[0], labels[-1], labels[:, 0], labels[:, -1]])
    border_labels = np.unique(border[border > 0])
    return np.isin(labels, border_labels)


def grow_into(background, allowed, passes):
    for _ in range(passes):
        grown = ndimage.binary_dilation(background) & allowed
        if (grown | background).sum() == background.sum():
            break
        background = background | grown
    return background


def drop_islands(mask):
    labels, count = ndimage.label(mask)
    if count == 0:
        return mask
    sizes = np.bincount(labels.ravel())
    sizes[0] = 0
    keep = sizes >= sizes.max() * ISLAND_MIN_FRACTION
    keep[0] = False
    return keep[labels]


def fill_holes_smaller_than(mask, limit):
    holes = ndimage.binary_fill_holes(mask) & ~mask
    labels, count = ndimage.label(holes)
    if count == 0:
        return mask
    sizes = np.bincount(labels.ravel())
    small = sizes < limit
    small[0] = False
    return mask | small[labels]


def key_white_background(rgb, settings):
    candidate = is_pale(rgb, BACKGROUND_MIN_LEVEL, BACKGROUND_MAX_CHROMA)
    background = flood_from_border(candidate)
    holes = is_pale(rgb, settings["hole_level"], settings["hole_chroma"]) & ~background
    labels, count = ndimage.label(holes)
    if count:
        sizes = np.bincount(labels.ravel())
        large = sizes >= settings["hole_min_size"]
        large[0] = False
        background = background | large[labels]
    pale = is_pale(rgb, settings["fringe_level"], FRINGE_MAX_CHROMA)
    background = grow_into(background, pale, settings["fringe_passes"])
    return drop_islands(~background)


def row_widths(mask):
    widths = np.zeros(mask.shape[0], dtype=int)
    for y in np.flatnonzero(mask.any(axis=1)):
        columns = np.flatnonzero(mask[y])
        widths[y] = columns[-1] - columns[0] + 1
    return widths


def ground_row(mask):
    rows = np.flatnonzero(mask.any(axis=1))
    top, bottom = rows[0], rows[-1]
    widths = row_widths(mask)
    search_limit = top + int((bottom - top) * 0.5)
    base_peak = 0
    trunk_row = bottom
    for y in range(bottom, search_limit, -1):
        if widths[y] > base_peak:
            if widths[trunk_row] < base_peak * 0.8 or (widths[y] > base_peak * 1.5 and base_peak > widths.max() * 0.2):
                break
            base_peak = widths[y]
            trunk_row = y
        if widths[y] < widths[trunk_row]:
            trunk_row = y
    trunk_width = widths[trunk_row]
    if trunk_width >= base_peak * 0.8:
        return bottom
    limit = min(trunk_width * MOUND_WIDTH_FACTOR, trunk_width + (base_peak - trunk_width) * 0.5)
    for y in range(trunk_row, bottom + 1):
        if widths[y] > limit:
            return y
    return bottom


def trim_mound(rgb, mask, keep_rows):
    trimmed = mask.copy()
    ground = ground_row(mask)
    trimmed[ground + keep_rows + 1:, :] = False
    pale = is_pale(rgb, PALE_BASE_LEVEL, PALE_BASE_CHROMA)
    trimmed[ground:, :] &= ~pale[ground:, :]
    return drop_islands(trimmed)


def trim_pale_base(rgb, mask):
    trimmed = mask.copy()
    pale = is_pale(rgb, PALE_BASE_LEVEL, PALE_BASE_CHROMA)
    for y in np.flatnonzero(mask.any(axis=1))[::-1]:
        if pale[y][mask[y]].mean() < PALE_BASE_FRACTION:
            break
        trimmed[y] = False
    return drop_islands(trimmed)


def grade(rgb, contrast, saturation):
    image = Image.fromarray(rgb)
    image = ImageEnhance.Contrast(image).enhance(contrast)
    return np.asarray(ImageEnhance.Color(image).enhance(saturation))


def premultiplied_resize(rgb, mask, size):
    alpha = mask.astype(np.float32)
    interior = ndimage.binary_erosion(mask, iterations=EDGE_INSET)
    interior_weight = np.where(interior, 1.0, 0.02).astype(np.float32) * alpha
    weighted = rgb.astype(np.float32) * interior_weight[..., None]
    channels = [np.asarray(Image.fromarray(weighted[..., index], "F").resize(size, Image.BOX)) for index in range(3)]
    small_alpha = np.asarray(Image.fromarray(alpha, "F").resize(size, Image.BOX))
    small_weight = np.asarray(Image.fromarray(interior_weight, "F").resize(size, Image.BOX))
    small = np.stack(channels, axis=-1)
    colour = np.clip(small / np.maximum(small_weight, 1e-4)[..., None], 0, 255).astype(np.uint8)
    return colour, small_alpha >= 0.5


def fit_to_canvas(rgb, mask, canvas_size):
    rows = np.flatnonzero(mask.any(axis=1))
    columns = np.flatnonzero(mask.any(axis=0))
    rgb = rgb[rows[0]:rows[-1] + 1, columns[0]:columns[-1] + 1]
    mask = mask[rows[0]:rows[-1] + 1, columns[0]:columns[-1] + 1]
    canvas_width, canvas_height = canvas_size
    scale = min((canvas_width - 2) / rgb.shape[1], (canvas_height - 1) / rgb.shape[0])
    width = max(1, round(rgb.shape[1] * scale))
    height = max(1, round(rgb.shape[0] * scale))
    colour, small_mask = premultiplied_resize(rgb, mask, (width, height))
    small_mask = drop_islands(small_mask)
    bottom = np.flatnonzero(small_mask.any(axis=1))[-1]
    full_colour = np.zeros((canvas_height, canvas_width, 3), dtype=np.uint8)
    full_mask = np.zeros((canvas_height, canvas_width), dtype=bool)
    left = (canvas_width - width) // 2
    top = canvas_height - 1 - bottom
    full_colour[top:top + height, left:left + width] = colour[:canvas_height - top]
    full_mask[top:top + height, left:left + width] = small_mask[:canvas_height - top]
    return full_colour, full_mask


def sharpen(colour):
    return np.asarray(Image.fromarray(colour).filter(ImageFilter.UnsharpMask(radius=1, percent=60, threshold=2)))


def build_palette(pixels, colours):
    strip = Image.fromarray(pixels.reshape(-1, 1, 3))
    quantised = strip.quantize(colors=colours, method=Image.Quantize.MEDIANCUT, dither=Image.Dither.NONE)
    used = len(quantised.getcolors(maxcolors=256))
    return quantised.getpalette()[:used * 3], used


def map_to_palette(colour, palette, used, dither=Image.Dither.NONE):
    reference = Image.new("P", (1, 1))
    reference.putpalette(palette + palette[:3] * (256 - used))
    return np.minimum(np.asarray(Image.fromarray(colour).quantize(palette=reference, dither=dither)), used - 1)


def fill_transparent(colour, mask):
    _, (rows, columns) = ndimage.distance_transform_edt(~mask, return_indices=True)
    return colour[rows, columns]


def indexed_with_transparency(colour, mask):
    palette, used = build_palette(colour[mask], SPRITE_COLOURS)
    mapped = map_to_palette(fill_transparent(colour, mask), palette, used, SPRITE_DITHER)
    present = np.unique(mapped[mask])
    remap = np.zeros(used, dtype=np.uint8)
    remap[present] = np.arange(1, len(present) + 1)
    indices = np.where(mask, remap[mapped], 0).astype(np.uint8)
    trimmed = [value for index in present for value in palette[index * 3:index * 3 + 3]]
    result = Image.fromarray(indices, "P")
    result.putpalette([0, 0, 0] + trimmed, rawmode="RGB")
    result.info["transparency"] = 0
    return result


def indexed_opaque(colour, colours, reserved=()):
    palette, used = build_palette(colour.reshape(-1, 3), colours - len(reserved))
    for reserved_colour in reserved:
        palette = palette + [int(round(channel)) for channel in reserved_colour]
        used += 1
    result = Image.fromarray(map_to_palette(colour, palette, used).astype(np.uint8), "P")
    result.putpalette(palette)
    return result


def save(image, name):
    path = os.path.join(OUTPUT_DIR, name + ".png")
    options = {}
    if len(image.getpalette()) <= 16 * 3:
        options["bits"] = 4
    if "transparency" in image.info:
        options["transparency"] = image.info["transparency"]
    image.save(path, **options)
    return path


def load_rgb(path):
    with Image.open(path) as image:
        return np.asarray(image.convert("RGB"))


def keyed_sprite(source_path, canvas_size, settings):
    rgb = load_rgb(source_path)
    mask = key_white_background(rgb, settings)
    mask = fill_holes_smaller_than(mask, 64)
    if settings["base"] == "trunk":
        rows = np.flatnonzero(mask.any(axis=1))
        scale = (canvas_size[1] - 1) / (rows[-1] - rows[0] + 1)
        mask = trim_mound(rgb, mask, int(round(MOUND_KEEP_PIXELS / scale)))
    mask = trim_pale_base(rgb, mask)
    colour, small_mask = fit_to_canvas(grade(rgb, CONTRAST, SATURATION), mask, canvas_size)
    return indexed_with_transparency(sharpen(colour), small_mask)


def raised_cosine(length):
    positions = (np.arange(length, dtype=np.float32) + 0.5) / length
    return np.sin(np.pi * positions) ** 2


def stamp_window(width, height):
    return (np.outer(raised_cosine(height), raised_cosine(width)) ** 2).astype(np.float32)


def random_patch(source, size, rng, rotate, box=None):
    height, width = source.shape[:2]
    left, top, right, bottom = box if box else (0, 0, width, height)
    x = int(rng.integers(left, max(left + 1, right - size)))
    y = int(rng.integers(top, max(top + 1, bottom - size)))
    patch = source[y:y + size, x:x + size]
    if rotate:
        patch = np.rot90(patch, int(rng.integers(0, 4)))
    if rng.integers(0, 2):
        patch = patch[:, ::-1]
    return patch


def synthesise(source, settings, area_metres, work_size, wrap_x, rng, box=None):
    work_width, work_height = work_size
    source_pixels_per_metre = source.shape[1] / settings["source_metres"]
    crop = int(settings["patch_metres"] * source_pixels_per_metre)
    stamp_width = max(4, round(settings["patch_metres"] * work_width / area_metres[0]))
    stamp_height = max(4, round(settings["patch_metres"] * work_height / area_metres[1]))
    window = stamp_window(stamp_width, stamp_height)
    accumulated = np.zeros((work_height, work_width, 3), dtype=np.float32)
    weights = np.zeros((work_height, work_width), dtype=np.float32)
    step_x, step_y = max(1, stamp_width // 3), max(1, stamp_height // 3)
    start_x = 0 if wrap_x else -stamp_width
    for grid_y in range(0, work_height, step_y):
        for grid_x in range(start_x, work_width, step_x):
            y = grid_y + int(rng.integers(-step_y // 2, step_y // 2 + 1))
            x = grid_x + int(rng.integers(-step_x // 2, step_x // 2 + 1))
            patch = random_patch(source, crop, rng, settings.get("rotate", True), box)
            patch = np.asarray(Image.fromarray(np.ascontiguousarray(patch)).resize((stamp_width, stamp_height), Image.BOX), dtype=np.float32)
            rows = (y + np.arange(stamp_height)) % work_height
            columns = x + np.arange(stamp_width)
            if wrap_x:
                columns = columns % work_width
                inside = np.ones(stamp_width, dtype=bool)
            else:
                inside = (columns >= 0) & (columns < work_width)
            columns = columns[inside]
            accumulated[np.ix_(rows, columns)] += patch[:, inside] * window[:, inside, None]
            weights[np.ix_(rows, columns)] += window[:, inside]
    result = accumulated / np.maximum(weights, 1e-6)[..., None]
    return result * (1 + settings.get("macro", 0.0) * periodic_noise(work_size, rng))[..., None]


def periodic_noise(size, rng):
    width, height = size
    white = rng.standard_normal((height, width))
    frequency_y = np.fft.fftfreq(height)[:, None] * height
    frequency_x = np.fft.fftfreq(width)[None, :] * width
    low_pass = np.exp(-(frequency_x ** 2 + frequency_y ** 2) / (2 * MACRO_CYCLES ** 2))
    noise = np.real(np.fft.ifft2(np.fft.fft2(white) * low_pass)).astype(np.float32)
    return np.clip(noise / (2.5 * noise.std()), -1, 1)


def box_down(work, factor):
    height, width = work.shape[:2]
    return work.reshape(height // factor, factor, width // factor, factor, 3).mean(axis=(1, 3))


def wrapped_filter(colour, wrap_x, image_filter):
    pad = 4
    mode_x = "wrap" if wrap_x else "edge"
    padded = np.pad(colour, ((pad, pad), (0, 0), (0, 0)), mode="wrap")
    padded = np.pad(padded, ((0, 0), (pad, pad), (0, 0)), mode=mode_x)
    filtered = np.asarray(Image.fromarray(padded).filter(image_filter))
    return filtered[pad:-pad, pad:-pad]


def point_down(work, factor):
    centre = factor // 2
    return (work[centre - 1::factor, centre - 1::factor] + work[centre::factor, centre::factor] + work[centre - 1::factor, centre::factor] + work[centre::factor, centre - 1::factor]) / 4


def finish_texture(work, wrap_x, detail, contrast=TEXTURE_CONTRAST):
    smooth = box_down(work, WORK_SCALE)
    small = smooth + (point_down(work, WORK_SCALE) - smooth) * detail
    small = np.clip(small, 0, 255).astype(np.uint8)
    small = wrapped_filter(small, wrap_x, ImageFilter.UnsharpMask(radius=1, percent=70, threshold=1))
    return grade(small, contrast, TEXTURE_SATURATION)


def wheel_track_profile(work_width, sigma_metres):
    metres = (np.arange(work_width) + 0.5) / work_width * ROAD_WIDTH_METRES - ROAD_WIDTH_METRES / 2
    profile = np.zeros(work_width, dtype=np.float32)
    for offset in WHEEL_TRACK_OFFSETS_METRES:
        profile = np.maximum(profile, np.exp(-0.5 * ((metres - offset) / sigma_metres) ** 2))
    return profile


def blur_vertically_wrapped(work, radius):
    return ndimage.uniform_filter1d(ndimage.uniform_filter1d(work, radius, axis=0, mode="wrap"), radius, axis=1, mode="nearest")


def add_wheel_tracks(work, strength):
    if strength <= 0:
        return work
    profile = wheel_track_profile(work.shape[1], 0.16)[None, :, None] * strength
    smoothed = blur_vertically_wrapped(work, WORK_SCALE * 2) * 0.9
    return work * (1 - profile) + smoothed * profile


def yellowness(rgb):
    channels = rgb.astype(np.int16)
    return (channels[..., 0] + channels[..., 1]) // 2 - channels[..., 2]


def line_box(rgb):
    profile = (yellowness(rgb) > 40).sum(axis=0)
    columns = np.flatnonzero(profile > profile.max() * 0.2)
    return columns[0], columns[-1] + 1


def tarmac_texture(rgb, settings, rng):
    work_size = (64 * WORK_SCALE, 64 * WORK_SCALE)
    area = (ROAD_WIDTH_METRES, ROAD_LENGTH_METRES)
    line_left, line_right = line_box(rgb)
    margin = rgb.shape[1] // 10
    if line_left > rgb.shape[1] - line_right:
        box = (0, 0, line_left - margin, rgb.shape[0])
    else:
        box = (line_right + margin, 0, rgb.shape[1], rgb.shape[0])
    work = synthesise(rgb, settings, area, work_size, False, rng, box)
    profile = wheel_track_profile(work_size[0], 0.2)[None, :, None]
    work = work * (1 - 0.16 * profile)
    small = finish_texture(work, False, settings["detail"]).astype(np.float32)
    line = rgb[:, line_left:line_right]
    line_yellowness = yellowness(line)
    paint = line[line_yellowness > np.percentile(line_yellowness, 97)]
    paint_colour = paint.astype(np.float32).mean(axis=0)
    wear = np.clip(0.8 + 0.25 * rng.standard_normal(32), 0.35, 1.0)
    wear = np.round(wear * 3) / 3
    column = 32
    for row in range(32):
        small[row, column] = small[row, column] * (1 - wear[row]) + paint_colour * wear[row]
    return np.clip(small, 0, 255).astype(np.uint8), paint_colour


def rut_strips(rgb, source_metres):
    pixels_per_metre = rgb.shape[1] / source_metres
    brightness = ndimage.uniform_filter1d(rgb.astype(np.float32).mean(axis=(0, 2)), int(pixels_per_metre * 0.1))
    half_width = int(pixels_per_metre * 0.3)
    strips = []
    order = np.argsort(brightness)
    for column in order:
        if column < half_width or column > rgb.shape[1] - half_width:
            continue
        if all(abs(column - other) > half_width * 2 for other in strips):
            strips.append(int(column))
        if len(strips) == 2:
            break
    return [rgb[:, column - half_width:column + half_width] for column in strips]


def vertically_periodic(strip, overlap):
    height = strip.shape[0] - overlap
    tile = strip[:height].astype(np.float32)
    continuation = strip[height:].astype(np.float32)
    ramp = raised_cosine(overlap * 2)[:overlap][:, None, None]
    tile[:overlap] = continuation * (1 - ramp) + tile[:overlap] * ramp
    return tile


def snow_texture(rgb, settings, rng, snow_sources):
    work_width, work_height = 64 * WORK_SCALE, 64 * WORK_SCALE
    area = (ROAD_WIDTH_METRES, ROAD_LENGTH_METRES)
    base_settings = dict(source_metres=SNOW_BASE_METRES, patch_metres=settings["patch_metres"], rotate=True, macro=settings["macro"])
    base_source = snow_sources[int(rng.integers(0, len(snow_sources)))]
    work = synthesise(base_source, base_settings, area, (work_width, work_height), False, rng)
    road_snow = rgb.reshape(-1, 3).astype(np.float32)
    bright = road_snow[road_snow.mean(axis=1) > np.percentile(road_snow.mean(axis=1), 60)]
    work *= (np.median(bright, axis=0) / work.reshape(-1, 3).mean(axis=0))[None, None, :]
    pixels_per_metre_x = work_width / ROAD_WIDTH_METRES
    pixels_per_metre_y = work_height / ROAD_LENGTH_METRES
    source_pixels_per_metre = rgb.shape[1] / settings["source_metres"]
    strips = rut_strips(rgb, settings["source_metres"])
    for index, offset in enumerate(WHEEL_TRACK_OFFSETS_METRES):
        strip = strips[index % len(strips)].astype(np.float32)
        overlap = strip.shape[0] // 4
        tile = vertically_periodic(strip, overlap)
        tile_metres = tile.shape[0] / source_pixels_per_metre
        repeats = max(1, round(ROAD_LENGTH_METRES / tile_metres))
        strip_width = round(strip.shape[1] / source_pixels_per_metre * pixels_per_metre_x)
        tile = np.asarray(Image.fromarray(np.clip(tile, 0, 255).astype(np.uint8)).resize((strip_width, work_height // repeats), Image.BOX), dtype=np.float32)
        tile = np.tile(tile, (repeats, 1, 1))
        tile = np.roll(tile, int(rng.integers(0, work_height)), axis=0)
        margins = np.concatenate([tile[:, :strip_width // 6], tile[:, -strip_width // 6:]], axis=1).mean(axis=1, keepdims=True)
        left = round((ROAD_WIDTH_METRES / 2 + offset) * pixels_per_metre_x) - strip_width // 2
        region = work[:, left:left + strip_width]
        matched = np.minimum(tile * (region.mean(axis=1, keepdims=True) / np.maximum(margins, 1)), region)
        offsets = (np.arange(strip_width) - (strip_width - 1) / 2) / pixels_per_metre_x
        core = 1 - RUT_DEPTH * np.exp(-0.5 * (offsets / RUT_CORE_METRES) ** 2)
        blend = raised_cosine(strip_width)[None, :, None]
        work[:, left:left + strip_width] = (region * (1 - blend) + matched * blend) * core[None, :, None]
    return finish_texture(work, False, settings["detail"])


def surface_texture(rgb, settings, rng):
    work_size = (64 * WORK_SCALE, 64 * WORK_SCALE)
    work = synthesise(rgb, settings, (ROAD_WIDTH_METRES, ROAD_LENGTH_METRES), work_size, False, rng)
    return finish_texture(add_wheel_tracks(work, settings["tracks"]), False, settings["detail"])


def track_columns(rgb, rows):
    darkness = 255 - rgb[rows].astype(np.float32).mean(axis=(0, 2))
    darkness = ndimage.uniform_filter1d(darkness, max(3, rgb.shape[1] // 24))
    half = rgb.shape[1] // 2
    return int(np.argmax(darkness[:half])), half + int(np.argmax(darkness[half:]))


def straighten_tracks(rgb):
    height, width = rgb.shape[:2]
    band = max(4, height // 8)
    top_left, top_right = track_columns(rgb, slice(0, band))
    bottom_left, bottom_right = track_columns(rgb, slice(height - band, height))
    left, right = (top_left + bottom_left) / 2, (top_right + bottom_right) / 2
    rows = np.arange(height, dtype=np.float32)[:, None]
    blend = rows / (height - 1)
    source_left = top_left + (bottom_left - top_left) * blend
    source_right = top_right + (bottom_right - top_right) * blend
    columns = np.arange(width, dtype=np.float32)[None, :]
    source_x = source_left + (columns - left) * (source_right - source_left) / (right - left)
    source_y = np.broadcast_to(rows, (height, width))
    channels = [
        ndimage.map_coordinates(rgb[..., channel].astype(np.float32), [source_y, source_x], order=1, mode="reflect")
        for channel in range(3)
    ]
    return np.stack(channels, axis=-1)


def direct_texture(rgb, settings):
    if settings.get("straighten"):
        rgb = straighten_tracks(rgb)
    if "crop" in settings:
        left, top, right, bottom = settings["crop"]
        height, width = rgb.shape[:2]
        rgb = rgb[int(top * height):int(bottom * height), int(left * width):int(right * width)]
    width, height = settings.get("size", (64, 64))
    tile = rgb.astype(np.float32)
    if settings["overlap"] > 0:
        tile = vertically_periodic(tile, int(tile.shape[0] * settings["overlap"]))
    image = Image.fromarray(np.clip(tile, 0, 255).astype(np.uint8))
    work = np.asarray(image.resize((width * WORK_SCALE, height * WORK_SCALE), Image.BOX), dtype=np.float32)
    return finish_texture(work, False, settings["detail"])


def road_texture(source_path, settings, rng, snow_sources):
    rgb = load_rgb(source_path)
    reserved = ()
    if settings["kind"] == "direct":
        small = direct_texture(rgb, settings)
    elif settings["kind"] == "tarmac":
        small, paint_colour = tarmac_texture(rgb, settings, rng)
        reserved = (paint_colour,)
    elif settings["kind"] == "snow":
        small = snow_texture(rgb, settings, rng, snow_sources)
    else:
        small = surface_texture(rgb, settings, rng)
    return indexed_opaque(small, TEXTURE_COLOURS, reserved)


def ground_texture(source_path, settings, rng, metres=GROUND_METRES, box=None):
    rgb = load_rgb(source_path)
    work_size = (64 * WORK_SCALE, 64 * WORK_SCALE)
    work = synthesise(rgb, settings, (metres, metres), work_size, True, rng, box)
    return indexed_opaque(finish_texture(work, True, settings["detail"]), TEXTURE_COLOURS)


def rock_texture(source_path, rng):
    rgb = load_rgb(source_path)
    height, width = rgb.shape[:2]
    left, top, right, bottom = ROCK_TEXTURE_BOX
    box = (int(left * width), int(top * height), int(right * width), int(bottom * height))
    return ground_texture(source_path, ROCK_TEXTURE_SETTINGS, rng, ROCK_TEXTURE_METRES, box)


def best_period(values, minimum, maximum, overlap):
    head = values[:overlap]
    errors = [np.mean((values[period:period + overlap] - head) ** 2) for period in range(minimum, maximum + 1)]
    return minimum + int(np.argmin(errors))


def periodic_both_ways(rgb, overlap_fraction):
    tile = rgb.astype(np.float32)
    for _ in range(2):
        size = tile.shape[0]
        overlap = int(size * overlap_fraction)
        period = best_period(tile, int(size * 0.7), size - overlap, overlap)
        tile = vertically_periodic(tile[:period + overlap], overlap)
        tile = tile.transpose(1, 0, 2)
    return tile


def cliff_texture(source_path, settings):
    tile = periodic_both_ways(load_rgb(source_path), settings["overlap"])
    work_size = 64 * WORK_SCALE
    repeats = settings["repeats"]
    tile_size = work_size // repeats
    small = np.asarray(Image.fromarray(np.clip(tile, 0, 255).astype(np.uint8)).resize((tile_size, tile_size), Image.BOX), dtype=np.float32)
    work = np.tile(small, (repeats, repeats, 1))
    return indexed_opaque(finish_texture(work, True, settings["detail"]), TEXTURE_COLOURS)


def haze_land(rgb):
    channels = rgb.astype(np.int16)
    blueness = channels[..., 2] - channels[..., 0]
    luminance = channels.mean(axis=2)
    return ((blueness > 15) & (luminance < 175)) | (luminance < 150)


def land_from_sky(crop, sky, settings):
    distance = np.sqrt(((crop - sky) ** 2).sum(axis=2))
    land = distance > settings["threshold"]
    if "cloud_texture" in settings:
        luminance = crop.mean(axis=2)
        local_mean = ndimage.uniform_filter(luminance, 7)
        local_std = np.sqrt(np.maximum(ndimage.uniform_filter(luminance ** 2, 7) - local_mean ** 2, 0))
        darker = luminance < sky.mean(axis=2) - 10
        land &= darker | (local_std > settings["cloud_texture"])
    return land


def fitted_sky(crop, sky_mask):
    height, width = sky_mask.shape
    rows, columns = np.mgrid[0:height, 0:width]
    y = rows / height
    x = columns / width
    terms = np.stack([np.ones_like(x), x, y, x * x, x * y, y * y], axis=-1)
    coefficients, *_ = np.linalg.lstsq(terms[sky_mask], crop[sky_mask], rcond=None)
    sky_rows = np.where(sky_mask.any(axis=0), height - 1 - np.argmax(sky_mask[::-1], axis=0), 0)
    clamped = terms.copy()
    limit = np.minimum(rows, sky_rows[None, :]) / height
    clamped[..., 2] = limit
    clamped[..., 4] = x * limit
    clamped[..., 5] = limit * limit
    return clamped @ coefficients


def sky_reference_land(rgb, box, settings):
    left, top, right, bottom = box
    first, last = settings["sky_columns"]
    reference = np.median(rgb[:, first:last].astype(np.float32), axis=1)
    reference[settings["sky_bottom"]:] = reference[settings["sky_bottom"]]
    crop = rgb[top:bottom, left:right].astype(np.float32)
    land = land_from_sky(crop, np.broadcast_to(reference[top:bottom, None, :], crop.shape), settings)
    ridge = ridge_line(land)
    sky_mask = np.arange(crop.shape[0])[:, None] < ridge[None, :] - 3
    return land_from_sky(crop, fitted_sky(crop, sky_mask), settings)


def connected_to_bottom(land):
    labels, _ = ndimage.label(ndimage.binary_opening(land, iterations=1))
    bottom_labels = np.unique(labels[-1][labels[-1] > 0])
    return np.isin(labels, bottom_labels)


def ridge_line(land):
    run = 4
    land = connected_to_bottom(land)
    solid = ndimage.minimum_filter1d(land.astype(np.uint8), run, axis=0, origin=-(run // 2)).astype(bool)
    ridge = np.argmax(solid, axis=0)
    ridge[~solid.any(axis=0)] = land.shape[0] - 1
    return ndimage.median_filter(ridge, size=9, mode="nearest")


def flatten_ends(ridge, columns):
    ridge = ridge.astype(np.float32)
    ramp = raised_cosine(columns * 2)[:columns]
    for end in (slice(0, columns), slice(len(ridge) - columns, len(ridge))):
        level = ridge[end].mean()
        weights = ramp[::-1] if end.start == 0 else ramp
        ridge[end] = ridge[end] * (1 - weights) + level * weights
    return np.round(ridge).astype(int)


def horizon_texture(source_path, box, sky_settings):
    full = load_rgb(source_path)
    left, top, right, bottom = box
    rgb = full[top:bottom, left:right]
    land = haze_land(rgb) if sky_settings is None else sky_reference_land(full, box, sky_settings)
    width, height = HORIZON_SIZE
    fade_columns = HORIZON_FADE_COLUMNS * rgb.shape[1] // width
    ridge = flatten_ends(ridge_line(land), fade_columns)
    mask = np.arange(rgb.shape[0])[:, None] >= ridge[None, :]
    colour, small_mask = premultiplied_resize(rgb, mask, (width, height))
    colour = grade(colour, 1.05, 0.9)
    palette, used = build_palette(colour[small_mask], HORIZON_COLOURS)
    mapped = map_to_palette(colour, palette, used)
    indices = np.where(small_mask, mapped + 1, 0).astype(np.uint8)
    result = Image.fromarray(indices, "P")
    result.putpalette([0, 0, 0] + palette)
    result.info["transparency"] = 0
    return result


def build_all(source_dir):
    written = []
    for prefix, pattern, size, settings, variants in SPRITES:
        for index, source in enumerate(find_sources(source_dir, pattern, variants)):
            written.append(save(keyed_sprite(source, size, settings), "%s_%d" % (prefix, index)))
    snow_sources = [load_rgb(path) for path in find_sources(source_dir, SNOW_BASE_SOURCE, None)]
    for prefix, pattern, settings in ROADS:
        for index, source in enumerate(find_sources(source_dir, pattern, None)):
            rng = np.random.default_rng([TEXTURE_SEED, len(written)])
            written.append(save(road_texture(source, settings, rng, snow_sources), "%s_%d" % (prefix, index)))
    for name, pattern, variant, settings in GROUNDS:
        rng = np.random.default_rng([TEXTURE_SEED, len(written)])
        written.append(save(ground_texture(find_sources(source_dir, pattern, None)[variant], settings, rng), name))
    rng = np.random.default_rng([TEXTURE_SEED, len(written)])
    rock_source = find_sources(source_dir, ROCK_TEXTURE_SOURCE, None)[ROCK_TEXTURE_VARIANT]
    written.append(save(rock_texture(rock_source, rng), "rock_texture"))
    for prefix, pattern, settings in CLIFFS:
        for index, source in enumerate(find_sources(source_dir, pattern, None)):
            written.append(save(cliff_texture(source, settings), "%s_%d" % (prefix, index)))
    name, pattern, settings = CLIFF_PLAIN
    rng = np.random.default_rng([TEXTURE_SEED, len(written)])
    written.append(save(ground_texture(find_sources(source_dir, pattern, None)[0], settings, rng), name))
    for name, pattern, box, sky_settings in HORIZONS:
        horizon_source = glob.glob(os.path.join(source_dir, pattern + ".jpg"))[0]
        written.append(save(horizon_texture(horizon_source, box, sky_settings), name))
    for prefix, pattern, size, settings, variants in LATE_SPRITES:
        for index, source in enumerate(find_sources(source_dir, pattern, variants)):
            written.append(save(keyed_sprite(source, size, settings), "%s_%d" % (prefix, index)))
    for prefix, pattern, settings in LATE_ROADS:
        for index, source in enumerate(find_sources(source_dir, pattern, None)):
            written.append(save(road_texture(source, settings, None, None), "%s_%d" % (prefix, index)))
    for name, pattern, variant, settings in LATE_TEXTURES:
        source = find_sources(source_dir, pattern, None)[variant]
        written.append(save(road_texture(source, settings, None, None), name))
    return written


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("source_dir")
    arguments = parser.parse_args()
    os.makedirs(OUTPUT_DIR, exist_ok=True)
    for path in build_all(arguments.source_dir):
        print(path)


if __name__ == "__main__":
    sys.exit(main())
