use psx_engine::{Framebuffer, Game, Input};

fn bgr555(red: u16, green: u16, blue: u16) -> u16 {
    (blue << 10) | (green << 5) | red
}

#[derive(Default)]
struct Demo {
    frame: u32,
}

impl Game for Demo {
    fn update(&mut self, _input: &Input) {
        self.frame = self.frame.wrapping_add(1);
    }

    fn render(&mut self, framebuffer: &mut Framebuffer) {
        let bars = [
            bgr555(31, 31, 31),
            bgr555(31, 31, 0),
            bgr555(0, 31, 31),
            bgr555(0, 31, 0),
            bgr555(31, 0, 31),
            bgr555(31, 0, 0),
            bgr555(0, 0, 31),
            bgr555(0, 0, 0),
        ];
        let width = framebuffer.width;
        let scroll = (self.frame / 4) as usize;
        for y in 0..framebuffer.height {
            for x in 0..width {
                let pixel = if y < 160 {
                    bars[x * bars.len() / width]
                } else if y < 200 {
                    let level = (x * 32 / width) as u16;
                    bgr555(level, level, level)
                } else if ((x + scroll) / 8 + y / 8).is_multiple_of(2) {
                    bgr555(31, 31, 31)
                } else {
                    bgr555(4, 4, 8)
                };
                framebuffer.pixels[y * width + x] = pixel;
            }
        }
    }
}

fn main() {
    psx_engine::run("psx demo", Demo::default());
}
