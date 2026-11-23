use psxel_prim::Texture;

pub const FOG_BANDS: [f32; 5] = [0.0, 0.33, 0.58, 0.78, 0.93];

#[derive(Clone, Copy, Debug)]
pub struct Image {
    pub u: u8,
    pub v: u8,
    pub width: u8,
    pub height: u8,
    pub texture: Texture,
}
