mod alien;
mod art;
mod effects;
mod fog;
mod lighting;
#[allow(dead_code)]
#[path = "../../temple/src/mesh.rs"]
mod mesh;
mod procedural;
#[allow(dead_code)]
#[path = "../../rally/src/render.rs"]
mod render;
#[allow(dead_code)]
#[path = "../../rally/src/rng.rs"]
mod rng;
mod scene;
mod sound;
#[allow(dead_code)]
#[path = "../../rally/src/sprites.rs"]
mod sprites;
mod sub;
mod terrain;
#[path = "../../temple/src/textures.rs"]
mod textures;
mod wreck;

mod biome {
    pub type Rgb = [u8; 3];
}

use psxel_engine::{Display, Framebuffer, Game, Gamepad, Input, Key};
use psxel_soft::{draw, draw_all, DrawEnv};
use psxel_vram::Vram;

use art::Art;
use biome::Rgb;
use lighting::Lighting;
use mesh::{add, length, scale, sub, Vec3, View};
use render::{
    screen_quad, Camera, Renderer, PROJECTION_H, SCREEN_HEIGHT, SCREEN_WIDTH, UNITS_PER_METER,
};
use scene::Scene;
use sound::{Mix, SoundSystem, Voice};
use sub::{Controls, Sub};
use terrain::{lerp, seabed, smoothstep, SURFACE};

const DT: f32 = 1.0 / 60.0;
const SHALLOW_FOG: [f32; 3] = [34.0, 104.0, 112.0];
const MID_FOG: [f32; 3] = [10.0, 46.0, 66.0];
const DEEP_FOG: [f32; 3] = [2.0, 8.0, 18.0];
const FOG_NEAR: f32 = 8.0;
const FOG_COMPLETE: f32 = 0.72;
const AIR_FOG: [f32; 3] = [6.0, 14.0, 16.0];
const AIR_FAR: f32 = 40.0;
const FOG_FAR: (f32, f32) = (48.0, 30.0);
const CAMERA_DISTANCE: f32 = 7.5;
const CAMERA_HEIGHT: f32 = 2.4;
const PERIOD_FRAME_RATE: u32 = 20;
const SONAR_PERIOD: f32 = 9.0;

fn set_fog_range(renderer: &mut Renderer, near: f32, far: f32) {
    let divisor =
        |meters: f32| PROJECTION_H as f64 * 65536.0 / (meters as f64 * UNITS_PER_METER as f64);
    let (near, far) = (divisor(near), divisor(far));
    let dqa = (4096.0 * 4096.0 / (far - near)).round();
    renderer.gte.dqa = dqa as i16;
    renderer.gte.dqb = (-dqa * near) as i32;
}

fn to_rgb(color: [f32; 3]) -> Rgb {
    color.map(|channel| channel.round().clamp(0.0, 255.0) as u8)
}

fn mix3(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [0, 1, 2].map(|c| lerp(a[c], b[c], t))
}

fn falloff(distance: f32, range: f32) -> f32 {
    (1.0 - distance / range).max(0.0).powi(2)
}

struct Dive {
    vram: Vram,
    renderer: Renderer,
    art: Art,
    lighting: Lighting,
    scene: Scene,
    sub: Sub,
    camera: Camera,
    camera_position: Vec3,
    sound: SoundSystem,
    time: f32,
    sonar_timer: f32,
    sonar_due: bool,
    frame: u32,
    display: Display,
}

impl Dive {
    fn new() -> Self {
        let mut vram = Vram::new();
        let art = Art::load(&mut vram);
        let lighting = Lighting;
        let scene = scene::build(&art, &lighting);
        let mut sub = Sub::new([2.0, -4.0, -24.0], 0.12);
        sub.panels = art.rov.clone();
        let mut dive = Self {
            vram,
            renderer: Renderer::new(),
            art,
            lighting,
            scene,
            sub,
            camera: Camera {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.0,
                roll: 0.0,
            },
            camera_position: [0.0; 3],
            sound: SoundSystem::default(),
            time: 0.0,
            sonar_timer: 2.0,
            sonar_due: false,
            frame: 0,
            display: Display {
                crt: !matches!(psxel_engine::option("crt").as_deref(), Some("0" | "off")),
                frame_limit: psxel_engine::option("fps")
                    .map(|value| value.parse().unwrap_or(PERIOD_FRAME_RATE)),
            },
        };
        dive.snap_camera();
        dive
    }

    fn chase_target(&self) -> Vec3 {
        let heading = [self.sub.yaw.sin(), 0.0, self.sub.yaw.cos()];
        let mut target = add(
            self.sub.position,
            add(scale(heading, -CAMERA_DISTANCE), [0.0, CAMERA_HEIGHT, 0.0]),
        );
        if alien::in_cave(self.sub.position) {
            let mut point = self.sub.position;
            let steps = 12;
            for step in 1..=steps {
                let candidate = add(
                    self.sub.position,
                    scale(sub(target, self.sub.position), step as f32 / steps as f32),
                );
                if !alien::in_cave(candidate) {
                    break;
                }
                point = candidate;
            }
            return alien::collide(point, 0.6);
        }
        target[1] = target[1]
            .max(seabed(target[0], target[2]) + 1.2)
            .min(SURFACE - 1.8);
        target
    }

    fn snap_camera(&mut self) {
        self.camera_position = self.chase_target();
        self.aim_camera();
    }

    fn aim_camera(&mut self) {
        let look = add(self.sub.position, [0.0, 0.6, 0.0]);
        let offset = sub(look, self.camera_position);
        self.camera = Camera {
            x: self.camera_position[0],
            y: self.camera_position[1],
            z: self.camera_position[2],
            yaw: offset[0].atan2(offset[2]),
            pitch: (-offset[1]).atan2(offset[0].hypot(offset[2])),
            roll: -self.sub.roll * 0.25,
        };
    }

    fn step(&mut self, controls: Controls) {
        self.sub.update(controls, DT);
        let target = self.chase_target();
        let follow = 1.0 - (-DT * 3.0).exp();
        self.camera_position = add(
            self.camera_position,
            scale(sub(target, self.camera_position), follow),
        );
        self.aim_camera();
    }

    fn controls(input: &Input) -> Controls {
        let axis = |positive: &[Key], negative: &[Key]| {
            let held = |keys: &[Key]| keys.iter().any(|&key| input.held(key)) as i32 as f32;
            held(positive) - held(negative)
        };
        let mut controls = Controls {
            thrust: axis(&[Key::KeyW, Key::ArrowUp], &[Key::KeyS, Key::ArrowDown]),
            turn: axis(&[Key::KeyD, Key::ArrowRight], &[Key::KeyA, Key::ArrowLeft]),
            lift: axis(&[Key::KeyE, Key::Space], &[Key::KeyQ, Key::ShiftLeft]),
        };
        if let Some(pad) = input.gamepad {
            controls.turn += pad.stick(0, 0.2);
            controls.thrust -= pad.stick(1, 0.2);
            controls.lift -= pad.stick(3, 0.2);
            controls.thrust += pad.value(Gamepad::RT) - pad.value(Gamepad::LT);
            if pad.held(Gamepad::A) {
                controls.lift += 1.0;
            }
            if pad.held(Gamepad::B) {
                controls.lift -= 1.0;
            }
        }
        if let Some((dx, dy)) = input.drag_delta() {
            controls.turn += (dx * 4.0).clamp(-1.0, 1.0);
            if input.touches.len() >= 2 {
                controls.lift -= (dy * 4.0).clamp(-1.0, 1.0);
            } else {
                controls.thrust -= (dy * 4.0).clamp(-1.0, 1.0);
            }
        }
        Controls {
            thrust: controls.thrust.clamp(-1.0, 1.0),
            turn: controls.turn.clamp(-1.0, 1.0),
            lift: controls.lift.clamp(-1.0, 1.0),
        }
    }

    fn render_frame(&mut self) -> usize {
        let camera = self.camera;
        let position = self.camera_position;
        let depth = (SURFACE - position[1]).max(0.0);
        let fog = if depth < 30.0 {
            mix3(SHALLOW_FOG, MID_FOG, smoothstep(0.0, 30.0, depth))
        } else {
            mix3(MID_FOG, DEEP_FOG, smoothstep(30.0, 65.0, depth))
        };
        let fog = mix3(
            fog,
            scale(fog, 0.45),
            wreck::inside(position).max(alien::inside(position)),
        );
        let air = alien::in_air(position);
        let fog = if air { AIR_FOG } else { fog };
        let fog = to_rgb(fog);
        let far = if air {
            AIR_FAR
        } else {
            lerp(FOG_FAR.0, FOG_FAR.1, smoothstep(10.0, 60.0, depth))
        };
        set_fog_range(&mut self.renderer, FOG_NEAR, far * FOG_COMPLETE);
        self.renderer.begin(&camera, fog);
        self.art.bands.update(&mut self.vram, fog);
        let view = View {
            position,
            forward: [
                camera.yaw.sin() * camera.pitch.cos(),
                -camera.pitch.sin(),
                camera.yaw.cos() * camera.pitch.cos(),
            ],
            right: (camera.yaw.cos(), -camera.yaw.sin()),
            fog,
            texture_fog: fog,
            fog_bands: fog::BANDS,
            near: FOG_NEAR,
            far,
            interior_bias: 0,
            time: self.time,
        };
        let craft = &self.sub;
        let time = self.time;
        self.scene
            .mesh
            .draw_lit(&mut self.renderer, &view, &[], |position| {
                let amount = effects::headlight(craft, position) * 1.6;
                add(
                    [255.0 * amount, 240.0 * amount, 205.0 * amount],
                    alien::glow(position, time),
                )
            });
        effects::draw(
            &mut self.renderer,
            &view,
            &self.art,
            &self.scene,
            &self.sub,
            &self.lighting,
        );
        self.sub.draw(&mut self.renderer);

        let buffer_y = (self.frame % 2) as i32 * SCREEN_HEIGHT;
        self.frame = self.frame.wrapping_add(1);
        let env = DrawEnv::new(0, buffer_y, SCREEN_WIDTH, SCREEN_HEIGHT);
        draw(
            &mut self.vram,
            &env,
            &screen_quad(
                0,
                0,
                SCREEN_WIDTH as i16,
                SCREEN_HEIGHT as i16,
                [fog, fog, fog, fog],
                None,
            ),
        );
        draw_all(&mut self.vram, &env, self.renderer.ot.draw_order());
        buffer_y as usize
    }

    fn copy_frame(&self, buffer_y: usize, pixels: &mut [u16], width: usize, height: usize) {
        let columns = width.min(SCREEN_WIDTH as usize);
        for row in 0..height {
            let source = (buffer_y + row) * psxel_vram::VRAM_WIDTH;
            pixels[row * width..row * width + columns]
                .copy_from_slice(&self.vram.pixels[source..source + columns]);
        }
    }

    fn mix(&self) -> Mix {
        let wreck_distance = length(sub(wreck::to_world([0.0, 0.0, 0.0]), self.sub.position));
        let depth = -self.sub.position[1];
        let inside = wreck::inside(self.sub.position);
        let wreck_side = sub(wreck::to_world([0.0, 0.0, 0.0]), self.camera_position);
        let pan = (wreck_side[0] * self.camera.yaw.cos() - wreck_side[2] * self.camera.yaw.sin())
            / wreck_side[0].hypot(wreck_side[2]).max(0.01);
        Mix {
            ambience: Voice {
                volume: 0.45 + 0.25 * smoothstep(5.0, 40.0, depth),
                pan: 0.0,
            },
            engine: Voice {
                volume: 0.06 + 0.2 * self.sub.thrust.abs(),
                pan: 0.0,
            },
            engine_rate: 0.85 + 0.35 * self.sub.thrust.abs(),
            bubbles: Voice {
                volume: 0.4 * self.sub.thrust.abs()
                    + 0.1 * smoothstep(-6.0, -1.0, self.sub.position[1]),
                pan: 0.0,
            },
            whale: Voice {
                volume: 0.35 * smoothstep(10.0, 35.0, depth) * (1.0 - inside),
                pan: (self.time * 0.05).sin() * 0.6,
            },
            creak: Voice {
                volume: 0.6 * falloff(wreck_distance, 55.0).max(inside),
                pan: pan * 0.7 * (1.0 - inside),
            },
            alien: {
                let source = alien::cupola();
                let offset = sub(source, self.camera_position);
                let side = (offset[0] * self.camera.yaw.cos() - offset[2] * self.camera.yaw.sin())
                    / offset[0].hypot(offset[2]).max(0.01);
                Voice {
                    volume: 0.7 * falloff(length(offset), 60.0),
                    pan: side * 0.7,
                }
            },
            sonar: self.sonar_due,
        }
    }
}

impl Game for Dive {
    fn update(&mut self, input: &Input) {
        if input.just_pressed(Key::KeyL) || input.pad_pressed(Gamepad::Y) {
            self.sub.lights = !self.sub.lights;
        }
        if input.just_pressed(Key::KeyV) {
            self.display.crt = !self.display.crt;
        }
        if input.just_pressed(Key::KeyF) {
            self.display.frame_limit = match self.display.frame_limit {
                Some(_) => None,
                None => Some(PERIOD_FRAME_RATE),
            };
        }
        if input.just_pressed(Key::KeyM) {
            self.sound.muted = !self.sound.muted;
        }
        self.time += DT;
        self.sonar_timer -= DT;
        if self.sonar_timer <= 0.0 {
            self.sonar_timer += SONAR_PERIOD;
            self.sonar_due = true;
        }
        self.step(Self::controls(input));
    }

    fn display(&self) -> Display {
        self.display
    }

    fn audio(&mut self, audio: &mut psxel_engine::Audio) {
        let mix = self.mix();
        self.sonar_due = false;
        self.sound.update(audio, &mix);
    }

    fn render(&mut self, framebuffer: &mut Framebuffer) {
        let buffer_y = self.render_frame();
        self.copy_frame(
            buffer_y,
            &mut framebuffer.pixels,
            framebuffer.width,
            framebuffer.height,
        );
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn screenshots(directory: &str, poses: &str) {
    let mut dive = Dive::new();
    let (width, height) = (SCREEN_WIDTH as usize, SCREEN_HEIGHT as usize);
    let mut pixels = vec![0u16; width * height];
    for (index, pose) in poses.split(';').enumerate() {
        let values: Vec<f32> = pose
            .split(',')
            .filter_map(|v| v.trim().parse().ok())
            .collect();
        if values.len() < 4 {
            continue;
        }
        let ground = seabed(values[0], values[2]);
        let raw = [values[0], values[1], values[2]];
        dive.sub.position = if alien::in_cave(raw) {
            raw
        } else {
            [values[0], values[1].max(ground + 1.6), values[2]]
        };
        dive.sub.yaw = values[3];
        dive.sub.thrust = values.get(4).copied().unwrap_or(0.5);
        dive.sub.lights = values.get(5).is_none_or(|&value| value > 0.5);
        dive.time = 10.0 + index as f32 * 0.37;
        dive.snap_camera();
        let buffer_y = dive.render_frame();
        dive.copy_frame(buffer_y, &mut pixels, width, height);
        let rgb: Vec<u8> = pixels
            .iter()
            .flat_map(|&pixel| [0, 5, 10].map(|shift| (((pixel >> shift) & 0x1f) << 3) as u8))
            .collect();
        let path = format!("{directory}/dive_{index:02}.png");
        let file = std::fs::File::create(&path).expect("create screenshot");
        let mut encoder =
            png::Encoder::new(std::io::BufWriter::new(file), width as u32, height as u32);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .and_then(|mut writer| writer.write_image_data(&rgb))
            .expect("write screenshot");
        println!("{path}");
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn drive(directory: &str) {
    let mut dive = Dive::new();
    let (width, height) = (SCREEN_WIDTH as usize, SCREEN_HEIGHT as usize);
    let mut pixels = vec![0u16; width * height];
    for tick in 0..(60 * 24) {
        let t = tick as f32 * DT;
        dive.time = t;
        dive.step(Controls {
            thrust: 1.0,
            turn: (t * 0.4).sin() * 0.6,
            lift: if t < 10.0 { -0.4 } else { 0.0 },
        });
        if tick % 90 == 89 {
            let buffer_y = dive.render_frame();
            dive.copy_frame(buffer_y, &mut pixels, width, height);
            let rgb: Vec<u8> = pixels
                .iter()
                .flat_map(|&pixel| [0, 5, 10].map(|shift| (((pixel >> shift) & 0x1f) << 3) as u8))
                .collect();
            let path = format!("{directory}/drive_{:02}.png", tick / 90);
            let file = std::fs::File::create(&path).expect("create screenshot");
            let mut encoder =
                png::Encoder::new(std::io::BufWriter::new(file), width as u32, height as u32);
            encoder.set_color(png::ColorType::Rgb);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .and_then(|mut writer| writer.write_image_data(&rgb))
                .expect("write screenshot");
        }
    }
}

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let arguments: Vec<String> = std::env::args().collect();
        if let Some(index) = arguments.iter().position(|argument| argument == "--drive") {
            drive(arguments.get(index + 1).map(String::as_str).unwrap_or("."));
            return;
        }
        if let Some(index) = arguments.iter().position(|argument| argument == "--shots") {
            let directory = arguments.get(index + 1).map(String::as_str).unwrap_or(".");
            let poses = arguments
                .iter()
                .position(|argument| argument == "--poses")
                .and_then(|index| arguments.get(index + 1))
                .map(String::as_str)
                .unwrap_or("2,-4,-24,0.12");
            screenshots(directory, poses);
            return;
        }
    }
    psxel_engine::run("psxel dive", Dive::new());
}
