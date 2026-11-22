mod present;

pub use present::{letterbox, Presenter, Viewport};

pub const DISPLAY_HEIGHT: u32 = 240;

pub fn bgr555_to_rgba8(pixel: u16) -> [u8; 4] {
    let red = (pixel & 0x1f) as u8;
    let green = ((pixel >> 5) & 0x1f) as u8;
    let blue = ((pixel >> 10) & 0x1f) as u8;
    [red << 3, green << 3, blue << 3, 0xff]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bgr555_unpacks_to_5_bit_aligned_rgba() {
        assert_eq!(bgr555_to_rgba8(0x0000), [0, 0, 0, 255]);
        assert_eq!(bgr555_to_rgba8(0x001f), [248, 0, 0, 255]);
        assert_eq!(bgr555_to_rgba8(0x03e0), [0, 248, 0, 255]);
        assert_eq!(bgr555_to_rgba8(0x7c00), [0, 0, 248, 255]);
        assert_eq!(bgr555_to_rgba8(0xffff), [248, 248, 248, 255]);
    }
}
