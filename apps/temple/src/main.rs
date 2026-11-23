mod art;
mod cave;
mod crypt;
mod effects;
mod grotto;
mod lighting;
mod mesh;
mod path;
mod procedural;
#[allow(dead_code)]
#[path = "../../rally/src/render.rs"]
mod render;
#[allow(dead_code)]
#[path = "../../rally/src/rng.rs"]
mod rng;
mod ruins;
mod scene;
mod secrets;
mod sound;
#[allow(dead_code)]
#[path = "../../rally/src/sprites.rs"]
mod sprites;
mod terrain;
mod textures;

mod biome {
    pub type Rgb = [u8; 3];
}

use psxel_engine::{Display, Framebuffer, Game, Input, Key};
use psxel_soft::{draw, draw_all, DrawEnv};
use psxel_vram::Vram;

use art::Art;
use biome::Rgb;
use lighting::LightKind;
use mesh::{length, sub, Vec3, View};
use path::Path;
use render::{
    screen_quad, Camera, Renderer, PROJECTION_H, SCREEN_HEIGHT, SCREEN_WIDTH, UNITS_PER_METER,
};
use scene::{insideness, temple_amount, Scene};
use sound::{Mix, SoundSystem, Voice};
use terrain::{lerp, CLIFF_TOP, FALLS_HALF_WIDTH, FALLS_Z, POOL_CENTER, WATER_Y};

const DT: f32 = 1.0 / 60.0;
const OUTSIDE_FOG: Rgb = [88, 108, 88];
const INSIDE_FOG: Rgb = [7, 6, 5];
const CAVE_FOG: Rgb = [4, 9, 11];
const GROTTO_FOG: Rgb = [6, 10, 8];
const OUTSIDE_SKY: Rgb = [128, 150, 140];
const FOG_NEAR: (f32, f32) = (16.0, 6.0);
const FOG_FAR: (f32, f32) = (115.0, 60.0);
const SHORT_FOG: Rgb = [16, 22, 17];
const ADAPTATION: f32 = 0.4;
const SHORT_NEAR: (f32, f32) = (18.0, 4.0);
const SHORT_FAR: (f32, f32) = (130.0, 35.0);
const LOOK_SPEED: f32 = 1.6;
const FLY_SPEED: f32 = 6.0;
const PERIOD_FRAME_RATE: u32 = 20;

fn lerp_rgb(from: Rgb, to: Rgb, t: f32) -> Rgb {
    [0, 1, 2].map(|c| lerp(from[c] as f32, to[c] as f32, t).round() as u8)
}

fn falloff(distance: f32, range: f32) -> f32 {
    (1.0 - distance / range).max(0.0).powi(2)
}

fn heading(from: Vec3, to: Vec3) -> f32 {
    (to[0] - from[0]).atan2(to[2] - from[2])
}

fn wrap_angle(angle: f32) -> f32 {
    (angle + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

fn set_fog_range(renderer: &mut Renderer, near: f32, far: f32) {
    let divisor =
        |meters: f32| PROJECTION_H as f64 * 65536.0 / (meters as f64 * UNITS_PER_METER as f64);
    let (near, far) = (divisor(near), divisor(far));
    let dqa = (4096.0 * 4096.0 / (far - near)).round();
    renderer.gte.dqa = dqa as i16;
    renderer.gte.dqb = (-dqa * near) as i32;
}

struct FreeCamera {
    position: Vec3,
    yaw: f32,
    pitch: f32,
}

impl FreeCamera {
    fn update(&mut self, input: &Input) {
        let axis = |positive: Key, negative: Key| {
            input.held(positive) as i32 as f32 - input.held(negative) as i32 as f32
        };
        self.yaw += axis(Key::ArrowRight, Key::ArrowLeft) * LOOK_SPEED * DT;
        self.pitch =
            (self.pitch + axis(Key::ArrowDown, Key::ArrowUp) * LOOK_SPEED * DT).clamp(-1.4, 1.4);
        let speed = FLY_SPEED * if input.held(Key::ShiftLeft) { 3.0 } else { 1.0 } * DT;
        let forward = axis(Key::KeyW, Key::KeyS) * speed;
        let strafe = axis(Key::KeyD, Key::KeyA) * speed;
        let lift = axis(Key::KeyE, Key::KeyQ) * speed;
        let (sin, cos) = self.yaw.sin_cos();
        self.position[0] += sin * self.pitch.cos() * forward + cos * strafe;
        self.position[1] += -self.pitch.sin() * forward + lift;
        self.position[2] += cos * self.pitch.cos() * forward - sin * strafe;
    }

    fn camera(&self) -> Camera {
        Camera {
            x: self.position[0],
            y: self.position[1],
            z: self.position[2],
            yaw: self.yaw,
            pitch: self.pitch,
            roll: 0.0,
        }
    }
}

struct Temple {
    vram: Vram,
    renderer: Renderer,
    art: Art,
    scene: Scene,
    path: Path,
    sound: SoundSystem,
    time: f32,
    tour_time: f32,
    paused: bool,
    free: Option<FreeCamera>,
    camera: Camera,
    frame: u32,
    display: Display,
    short_fog: bool,
    atmosphere: Option<Atmosphere>,
}

#[derive(Clone, Copy)]
struct Atmosphere {
    fog: [f32; 3],
    sky: [f32; 3],
    near: f32,
    far: f32,
    time: f32,
}

impl Atmosphere {
    fn ease(self, target: Atmosphere) -> Atmosphere {
        let elapsed = (target.time - self.time).max(0.0);
        let amount = 1.0 - (-elapsed / ADAPTATION).exp();
        let mix3 = |a: [f32; 3], b: [f32; 3]| [0, 1, 2].map(|c| lerp(a[c], b[c], amount));
        Atmosphere {
            fog: mix3(self.fog, target.fog),
            sky: mix3(self.sky, target.sky),
            near: lerp(self.near, target.near, amount),
            far: lerp(self.far, target.far, amount),
            time: target.time,
        }
    }
}

impl Temple {
    fn new() -> Self {
        let mut vram = Vram::new();
        let art = Art::load(&mut vram);
        let path = scene::tour();
        let scene = scene::build(&art, &path);
        let mut temple = Self {
            vram,
            renderer: Renderer::new(),
            art,
            scene,
            path,
            sound: SoundSystem::default(),
            time: 0.0,
            tour_time: 0.0,
            paused: false,
            free: None,
            camera: Camera {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.0,
                roll: 0.0,
            },
            frame: 0,
            atmosphere: None,
            short_fog: !matches!(psxel_engine::option("fog").as_deref(), Some("0" | "off")),
            display: Display {
                crt: !matches!(psxel_engine::option("crt").as_deref(), Some("0" | "off")),
                frame_limit: psxel_engine::option("fps")
                    .map(|value| value.parse().unwrap_or(PERIOD_FRAME_RATE)),
            },
        };
        temple.camera = temple.tour_camera();
        temple
    }

    fn tour_camera(&self) -> Camera {
        let (position, target) = self.path.sample(self.tour_time);
        let (ahead, ahead_target) = self.path.sample(self.tour_time + 0.5);
        let yaw = heading(position, target);
        let turn = wrap_angle(heading(ahead, ahead_target) - yaw) / 0.5;
        let (dx, dy, dz) = (
            target[0] - position[0],
            target[1] - position[1],
            target[2] - position[2],
        );
        Camera {
            x: position[0],
            y: position[1],
            z: position[2],
            yaw,
            pitch: (-dy).atan2(dx.hypot(dz)),
            roll: (-turn * 0.05).clamp(-0.12, 0.12),
        }
    }

    fn position(&self) -> Vec3 {
        [self.camera.x, self.camera.y, self.camera.z]
    }

    fn render_frame(&mut self) -> usize {
        let camera = self.camera;
        let position = self.position();
        let inside = insideness(position);
        let indoor_fog = lerp_rgb(
            lerp_rgb(INSIDE_FOG, CAVE_FOG, cave::amount(position)),
            GROTTO_FOG,
            grotto::amount(position),
        );
        let (outside_fog, near_range, far_range) = if self.short_fog {
            (SHORT_FOG, SHORT_NEAR, SHORT_FAR)
        } else {
            (OUTSIDE_FOG, FOG_NEAR, FOG_FAR)
        };
        let outside_sky = if self.short_fog {
            SHORT_FOG
        } else {
            OUTSIDE_SKY
        };
        let to_float = |color: Rgb| color.map(|channel| channel as f32);
        let target = Atmosphere {
            fog: to_float(lerp_rgb(outside_fog, indoor_fog, inside)),
            sky: to_float(lerp_rgb(outside_sky, indoor_fog, inside)),
            near: lerp(near_range.0, near_range.1, inside),
            far: lerp(far_range.0, far_range.1, inside),
            time: self.time,
        };
        let atmosphere = match self.atmosphere {
            Some(previous) if self.time - previous.time < 1.0 => previous.ease(target),
            _ => target,
        };
        self.atmosphere = Some(atmosphere);
        let to_rgb = |color: [f32; 3]| color.map(|channel| channel.round().clamp(0.0, 255.0) as u8);
        let fog = to_rgb(atmosphere.fog);
        let (near, far) = (atmosphere.near, atmosphere.far);
        set_fog_range(&mut self.renderer, near, far);
        self.renderer.begin(&camera, fog);
        let view = View {
            position,
            forward: [
                camera.yaw.sin() * camera.pitch.cos(),
                -camera.pitch.sin(),
                camera.yaw.cos() * camera.pitch.cos(),
            ],
            right: (camera.yaw.cos(), -camera.yaw.sin()),
            fog,
            near,
            far,
            interior_bias: if inside < 0.3 { 160 } else { 0 },
            time: self.time,
        };
        let flicker = self.scene.lighting.flicker(self.time);
        self.art.procedural.animate(&mut self.vram, self.time);
        self.scene.mesh.draw(&mut self.renderer, &view, &flicker);
        effects::draw(&mut self.renderer, &view, &self.art, &self.scene, &flicker);

        let buffer_y = (self.frame % 2) as i32 * SCREEN_HEIGHT;
        self.frame = self.frame.wrapping_add(1);
        let env = DrawEnv::new(0, buffer_y, SCREEN_WIDTH, SCREEN_HEIGHT);
        let sky = to_rgb(atmosphere.sky);
        draw(
            &mut self.vram,
            &env,
            &screen_quad(
                0,
                0,
                SCREEN_WIDTH as i16,
                SCREEN_HEIGHT as i16,
                [sky, sky, fog, fog],
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

    fn emitter(&self, source: Vec3, volume: f32, range: f32) -> Voice {
        let offset = sub(source, self.position());
        let distance = length(offset);
        let flat = offset[0].hypot(offset[2]).max(0.001);
        let side = (offset[0] * self.camera.yaw.cos() - offset[2] * self.camera.yaw.sin()) / flat;
        let spread = (distance / 6.0).min(1.0);
        Voice {
            volume: volume * falloff(distance, range),
            pan: side * spread * 0.85,
        }
    }

    fn mix(&self) -> Mix {
        let position = self.position();
        let temple = temple_amount(position).max(crypt::amount(position));
        let cave = cave::amount(position);
        let grotto = grotto::amount(position);
        let outside = 1.0 - temple.max(cave).max(grotto);
        let falls = [
            -38.5,
            position[1].clamp(WATER_Y, CLIFF_TOP),
            position[2].clamp(FALLS_Z - FALLS_HALF_WIDTH, FALLS_Z + FALLS_HALF_WIDTH),
        ];
        let torch = self
            .scene
            .lighting
            .lights
            .iter()
            .filter(|light| light.kind == LightKind::Torch)
            .map(|light| light.position)
            .min_by(|a, b| length(sub(*a, position)).total_cmp(&length(sub(*b, position))))
            .unwrap_or(position);
        let mut waterfall = self.emitter(falls, 1.0, 95.0);
        waterfall.volume *= 1.0 - 0.85 * temple - 0.35 * cave - 0.3 * grotto;
        let mut stream = self.emitter([POOL_CENTER.0, WATER_Y, POOL_CENTER.1], 0.5, 30.0);
        stream.volume *= outside;
        Mix {
            jungle: Voice::ambient(0.6 * outside),
            birds: Voice {
                volume: 0.55 * outside * outside,
                pan: 0.55 * (self.time * 0.07).sin() * (self.time * 0.023 + 1.0).cos(),
            },
            waterfall,
            stream,
            drone: Voice::ambient(0.5 * temple + 0.15 * cave),
            drips: Voice::ambient(0.6 * temple + 0.8 * cave + 0.7 * grotto),
            torch: self.emitter(torch, 0.6, 9.0),
            cascade: {
                let nearest = grotto::CRACKS
                    .iter()
                    .map(|&(x, z)| [x, position[1].clamp(grotto::WATER, grotto::CEILING), z])
                    .min_by(|a, b| length(sub(*a, position)).total_cmp(&length(sub(*b, position))))
                    .unwrap_or(position);
                let mut voice = self.emitter(nearest, 0.75, 16.0);
                voice.volume *= grotto.max(0.25);
                voice
            },
        }
    }
}

impl Game for Temple {
    fn update(&mut self, input: &Input) {
        if input.just_pressed(Key::KeyV) {
            self.display.crt = !self.display.crt;
        }
        if input.just_pressed(Key::KeyG) {
            self.short_fog = !self.short_fog;
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
        if input.just_pressed(Key::Space) || input.tapped {
            self.paused = !self.paused;
        }
        if input.just_pressed(Key::KeyC) {
            self.free = match self.free {
                Some(_) => None,
                None => Some(FreeCamera {
                    position: self.position(),
                    yaw: self.camera.yaw,
                    pitch: self.camera.pitch,
                }),
            };
        }
        self.time += DT;
        match &mut self.free {
            Some(free) => free.update(input),
            None if !self.paused => self.tour_time += DT,
            None => {}
        }
        self.camera = match &self.free {
            Some(free) => free.camera(),
            None => self.tour_camera(),
        };
    }

    fn display(&self) -> Display {
        self.display
    }

    fn audio(&mut self, audio: &mut psxel_engine::Audio) {
        let mix = self.mix();
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
fn look_at(position: Vec3, target: Vec3) -> Camera {
    let (dx, dy, dz) = (
        target[0] - position[0],
        target[1] - position[1],
        target[2] - position[2],
    );
    Camera {
        x: position[0],
        y: position[1],
        z: position[2],
        yaw: dx.atan2(dz),
        pitch: (-dy).atan2(dx.hypot(dz)),
        roll: 0.0,
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn screenshots(directory: &str, times: Option<&str>, views: Option<&str>) {
    let mut temple = Temple::new();
    let duration = temple.path.duration;
    println!("tour: {duration:.1} s");
    let mut shots: Vec<(String, f32, Option<Camera>)> = Vec::new();
    if let Some(views) = views {
        for (index, view) in views.split(';').enumerate() {
            let values: Vec<f32> = view
                .split(',')
                .filter_map(|v| v.trim().parse().ok())
                .collect();
            if values.len() == 6 {
                let camera = look_at(
                    [values[0], values[1], values[2]],
                    [values[3], values[4], values[5]],
                );
                shots.push((format!("view_{index:02}"), 10.0, Some(camera)));
            }
        }
    } else {
        let times: Vec<f32> = match times {
            Some(list) => list
                .split(',')
                .filter_map(|time| time.parse().ok())
                .collect(),
            None => (0..12).map(|shot| duration * shot as f32 / 12.0).collect(),
        };
        shots.extend(
            times
                .into_iter()
                .map(|time| (format!("temple_{time:07.3}"), time, None)),
        );
    }
    let (width, height) = (SCREEN_WIDTH as usize, SCREEN_HEIGHT as usize);
    let mut pixels = vec![0u16; width * height];
    for (name, time, camera) in shots {
        temple.tour_time = time;
        temple.time = time;
        temple.camera = camera.unwrap_or_else(|| temple.tour_camera());
        let buffer_y = temple.render_frame();
        temple.copy_frame(buffer_y, &mut pixels, width, height);
        let rgb: Vec<u8> = pixels
            .iter()
            .flat_map(|&pixel| [0, 5, 10].map(|shift| (((pixel >> shift) & 0x1f) << 3) as u8))
            .collect();
        let path = format!("{directory}/{name}.png");
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

fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let arguments: Vec<String> = std::env::args().collect();
        if let Some(index) = arguments.iter().position(|argument| argument == "--shots") {
            let directory = arguments.get(index + 1).map(String::as_str).unwrap_or(".");
            let option = |name: &str| {
                arguments
                    .iter()
                    .position(|argument| argument == name)
                    .and_then(|index| arguments.get(index + 1))
                    .map(String::as_str)
            };
            screenshots(directory, option("--times"), option("--views"));
            return;
        }
    }
    psxel_engine::run("psxel temple", Temple::new());
}
