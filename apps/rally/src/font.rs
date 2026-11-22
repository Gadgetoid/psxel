pub const GLYPH_SIZE: usize = 8;
pub const COLUMNS: usize = 16;

const GLYPHS: &[(char, [u8; 7])] = &[
    ('0', [0x0e, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0e]),
    ('1', [0x04, 0x0c, 0x04, 0x04, 0x04, 0x04, 0x0e]),
    ('2', [0x0e, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1f]),
    ('3', [0x1f, 0x02, 0x04, 0x02, 0x01, 0x11, 0x0e]),
    ('4', [0x02, 0x06, 0x0a, 0x12, 0x1f, 0x02, 0x02]),
    ('5', [0x1f, 0x10, 0x1e, 0x01, 0x01, 0x11, 0x0e]),
    ('6', [0x06, 0x08, 0x10, 0x1e, 0x11, 0x11, 0x0e]),
    ('7', [0x1f, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08]),
    ('8', [0x0e, 0x11, 0x11, 0x0e, 0x11, 0x11, 0x0e]),
    ('9', [0x0e, 0x11, 0x11, 0x0f, 0x01, 0x02, 0x0c]),
    ('A', [0x0e, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11]),
    ('B', [0x1e, 0x11, 0x11, 0x1e, 0x11, 0x11, 0x1e]),
    ('C', [0x0e, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0e]),
    ('D', [0x1c, 0x12, 0x11, 0x11, 0x11, 0x12, 0x1c]),
    ('E', [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x1f]),
    ('F', [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x10]),
    ('G', [0x0e, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0f]),
    ('H', [0x11, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11]),
    ('I', [0x0e, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0e]),
    ('J', [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0c]),
    ('K', [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11]),
    ('L', [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1f]),
    ('M', [0x11, 0x1b, 0x15, 0x15, 0x11, 0x11, 0x11]),
    ('N', [0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11]),
    ('O', [0x0e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e]),
    ('P', [0x1e, 0x11, 0x11, 0x1e, 0x10, 0x10, 0x10]),
    ('Q', [0x0e, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0d]),
    ('R', [0x1e, 0x11, 0x11, 0x1e, 0x14, 0x12, 0x11]),
    ('S', [0x0f, 0x10, 0x10, 0x0e, 0x01, 0x01, 0x1e]),
    ('T', [0x1f, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04]),
    ('U', [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e]),
    ('V', [0x11, 0x11, 0x11, 0x11, 0x11, 0x0a, 0x04]),
    ('W', [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0a]),
    ('X', [0x11, 0x11, 0x0a, 0x04, 0x0a, 0x11, 0x11]),
    ('Y', [0x11, 0x11, 0x11, 0x0a, 0x04, 0x04, 0x04]),
    ('Z', [0x1f, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1f]),
    ('.', [0x00, 0x00, 0x00, 0x00, 0x00, 0x0c, 0x0c]),
    (',', [0x00, 0x00, 0x00, 0x00, 0x0c, 0x04, 0x08]),
    (':', [0x00, 0x0c, 0x0c, 0x00, 0x0c, 0x0c, 0x00]),
    ('!', [0x04, 0x04, 0x04, 0x04, 0x04, 0x00, 0x04]),
    ('?', [0x0e, 0x11, 0x01, 0x02, 0x04, 0x00, 0x04]),
    ('-', [0x00, 0x00, 0x00, 0x1f, 0x00, 0x00, 0x00]),
    ('+', [0x00, 0x04, 0x04, 0x1f, 0x04, 0x04, 0x00]),
    ('/', [0x00, 0x01, 0x02, 0x04, 0x08, 0x10, 0x00]),
    ('\'', [0x04, 0x04, 0x08, 0x00, 0x00, 0x00, 0x00]),
    ('"', [0x0a, 0x0a, 0x00, 0x00, 0x00, 0x00, 0x00]),
    ('>', [0x08, 0x04, 0x02, 0x01, 0x02, 0x04, 0x08]),
    ('<', [0x02, 0x04, 0x08, 0x10, 0x08, 0x04, 0x02]),
    ('%', [0x18, 0x19, 0x02, 0x04, 0x08, 0x13, 0x03]),
    ('(', [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02]),
    (')', [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08]),
];

pub fn cell(character: char) -> Option<usize> {
    let code = character.to_ascii_uppercase() as usize;
    (33..128).contains(&code).then_some(code - 32)
}

pub fn bitmap(width: usize, height: usize) -> Vec<u8> {
    assert!(width >= COLUMNS * GLYPH_SIZE && height >= 6 * GLYPH_SIZE);
    let mut pixels = vec![0u8; width * height];
    for (character, rows) in GLYPHS {
        let Some(index) = cell(*character) else {
            continue;
        };
        let origin_x = (index % COLUMNS) * GLYPH_SIZE;
        let origin_y = (index / COLUMNS) * GLYPH_SIZE;
        let lit = |x: i32, y: i32| {
            (0..5).contains(&x) && (0..7).contains(&y) && rows[y as usize] & (0x10 >> x) != 0
        };
        for y in -1..8i32 {
            for x in -1..7i32 {
                let value = if lit(x, y) {
                    2
                } else if (-1..=1).any(|dy| (-1..=1).any(|dx| lit(x + dx, y + dy))) {
                    1
                } else {
                    continue;
                };
                let px = origin_x as i32 + x + 1;
                let py = origin_y as i32 + y;
                if px >= origin_x as i32
                    && py >= origin_y as i32
                    && px < (origin_x + GLYPH_SIZE) as i32
                    && py < (origin_y + GLYPH_SIZE) as i32
                {
                    pixels[py as usize * width + px as usize] = value;
                }
            }
        }
    }
    pixels
}

pub const OUTLINE: u8 = 1;
pub const SHADOW: u8 = 2;
pub const FACE_TOP: u8 = 3;

pub struct Style {
    pub scale_passes: usize,
    pub embolden: usize,
    pub shear: usize,
    pub outline: usize,
    pub shadow: usize,
    pub cell_width: usize,
    pub cell_height: usize,
}

pub const LARGE: Style = Style {
    scale_passes: 1,
    embolden: 1,
    shear: 2,
    outline: 1,
    shadow: 1,
    cell_width: 16,
    cell_height: 18,
};

pub const HUGE: Style = Style {
    scale_passes: 2,
    embolden: 2,
    shear: 4,
    outline: 2,
    shadow: 2,
    cell_width: 32,
    cell_height: 36,
};

pub const HUGE_CHARACTERS: &str = "0123456789!GOTIMEUPSXRALY:.";

pub fn rows(character: char) -> Option<[u8; 7]> {
    let upper = character.to_ascii_uppercase();
    GLYPHS
        .iter()
        .find(|(c, _)| *c == upper)
        .map(|(_, rows)| *rows)
}

fn scale2x(source: &[bool], width: usize, height: usize) -> Vec<bool> {
    let at = |x: i32, y: i32| {
        if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
            false
        } else {
            source[y as usize * width + x as usize]
        }
    };
    let mut out = vec![false; width * height * 4];
    for y in 0..height as i32 {
        for x in 0..width as i32 {
            let p = at(x, y);
            let (a, b, c, d) = (at(x, y - 1), at(x + 1, y), at(x - 1, y), at(x, y + 1));
            let corners = [
                if c == a && c != d && a != b { a } else { p },
                if a == b && a != c && b != d { b } else { p },
                if d == c && d != b && c != a { c } else { p },
                if b == d && b != a && d != c { d } else { p },
            ];
            for (index, value) in corners.into_iter().enumerate() {
                let ox = x as usize * 2 + index % 2;
                let oy = y as usize * 2 + index / 2;
                out[oy * width * 2 + ox] = value;
            }
        }
    }
    out
}

fn styled_glyph(rows: [u8; 7], style: &Style) -> (Vec<u8>, usize, usize) {
    let (mut width, mut height) = (5, 7);
    let mut mask: Vec<bool> = (0..35)
        .map(|i| rows[i / 5] & (0x10 >> (i % 5)) != 0)
        .collect();
    for _ in 0..style.scale_passes {
        mask = scale2x(&mask, width, height);
        width *= 2;
        height *= 2;
    }
    let bold_width = width + style.embolden;
    let mut bold = vec![false; bold_width * height];
    for y in 0..height {
        for x in 0..width {
            if mask[y * width + x] {
                for dx in 0..=style.embolden {
                    bold[y * bold_width + x + dx] = true;
                }
            }
        }
    }
    let margin = style.outline;
    let out_width = bold_width + style.shear + margin * 2 + style.shadow;
    let out_height = height + margin * 2 + style.shadow;
    let mut face = vec![0u8; out_width * out_height];
    for y in 0..height {
        let shift = if height > 1 {
            style.shear * (height - 1 - y) / (height - 1)
        } else {
            0
        };
        let shade = FACE_TOP + (y * 4 / height) as u8;
        for x in 0..bold_width {
            if bold[y * bold_width + x] {
                face[(y + margin) * out_width + x + shift + margin] = shade;
            }
        }
    }
    let solid: Vec<bool> = face.iter().map(|&value| value != 0).collect();
    let reach = margin as i32;
    for y in 0..out_height as i32 {
        for x in 0..out_width as i32 {
            if solid[y as usize * out_width + x as usize] {
                continue;
            }
            let near = (-reach..=reach).any(|dy| {
                (-reach..=reach).any(|dx| {
                    let (nx, ny) = (x + dx, y + dy);
                    nx >= 0
                        && ny >= 0
                        && nx < out_width as i32
                        && ny < out_height as i32
                        && solid[ny as usize * out_width + nx as usize]
                })
            });
            if near {
                face[y as usize * out_width + x as usize] = OUTLINE;
            }
        }
    }
    let inked: Vec<bool> = face.iter().map(|&value| value != 0).collect();
    let offset = style.shadow as i32;
    for y in 0..out_height as i32 {
        for x in 0..out_width as i32 {
            let index = y as usize * out_width + x as usize;
            if inked[index] {
                continue;
            }
            let (sx, sy) = (x - offset, y - offset);
            if sx >= 0 && sy >= 0 && inked[sy as usize * out_width + sx as usize] {
                face[index] = SHADOW;
            }
        }
    }
    (face, out_width, out_height)
}

pub fn styled_atlas(style: &Style, characters: &[char], columns: usize) -> (Vec<u8>, usize, usize) {
    let rows_needed = characters.len().div_ceil(columns);
    let width = columns * style.cell_width;
    let height = rows_needed * style.cell_height;
    let mut pixels = vec![0u8; width * height];
    for (slot, &character) in characters.iter().enumerate() {
        let Some(bitmap) = rows(character) else {
            continue;
        };
        let (glyph, glyph_width, glyph_height) = styled_glyph(bitmap, style);
        let origin_x = (slot % columns) * style.cell_width;
        let origin_y = (slot / columns) * style.cell_height;
        for y in 0..glyph_height.min(style.cell_height) {
            for x in 0..glyph_width.min(style.cell_width) {
                let value = glyph[y * glyph_width + x];
                if value != 0 {
                    pixels[(origin_y + y) * width + origin_x + x] = value;
                }
            }
        }
    }
    (pixels, width, height)
}

pub fn ascii_characters() -> Vec<char> {
    (32u8..128).map(|code| code as char).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn styled_glyphs_fit_their_cells() {
        for style in [&LARGE, &HUGE] {
            let (_, width, height) = styled_glyph(rows('W').unwrap(), style);
            assert!(
                width <= style.cell_width && height <= style.cell_height,
                "{width}x{height}"
            );
        }
    }

    #[test]
    fn scale2x_rounds_diagonals() {
        let diagonal = [true, false, false, true];
        let scaled = scale2x(&diagonal, 2, 2);
        assert_eq!(scaled.iter().filter(|&&pixel| pixel).count(), 10);
    }
}
