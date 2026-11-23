mod art;
mod biome;
mod blend;
mod car;
mod daytime;
mod dial;
mod font;
mod hud;
mod landscape;
mod livery;
mod render;
mod rng;
mod sound;
mod sprites;
mod storage;
mod textures;
mod track;
mod world;

use psx_engine::{Framebuffer, Game, Input, Key};
use psx_soft::{draw_all, fill_rect, DrawEnv};
use psx_vram::Vram;

use biome::{scale_rgb, Weather, BIOMES};
use car::{Car, CarMesh, Controls, Ground, DT};
use hud::{Hud, Size, CYAN, GREEN, RED, WHITE, YELLOW};
use render::{Camera, Renderer, SCREEN_HEIGHT, SCREEN_WIDTH};
use rng::Rng;
use textures::Atlas;
use track::{PaceNote, Track, CHECKPOINT_SPACING};
use world::{Lighting, World};

const START_TIME: f32 = 75.0;
const CHECKPOINT_BONUS: f32 = 32.0;
const CALL_AHEAD: f32 = 70.0;
const CALL_SECONDS: f32 = 2.6;
const BLEND_HOLD: f32 = 300.0;
const LOOK_DEAD_ZONE: f32 = 0.18;
const LOOK_SPEED: f32 = 3.0;
const LOOK_RETURN: f32 = 2.5;
const LOOK_LIFT: f32 = 3.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    RaceTheClock,
    EnjoyTheScenery,
}

const MODES: [(Mode, &str); 2] = [
    (Mode::RaceTheClock, "RACE THE CLOCK"),
    (Mode::EnjoyTheScenery, "ENJOY THE SCENERY"),
];
const MENU_Y: i16 = 104;
const MENU_SPACING: i16 = 22;

#[derive(Clone, Copy, PartialEq)]
enum State {
    Title,
    Countdown(f32),
    Racing,
    TimeUp(f32),
}

struct Banner {
    title: String,
    subtitle: String,
    color: [u8; 3],
    timer: f32,
}

struct Rally {
    vram: Vram,
    atlas: Atlas,
    art: art::Art,
    renderer: Renderer,
    track: Track,
    world: World,
    car: Car,
    mesh: CarMesh,
    livery: livery::Livery,
    state: State,
    camera: Camera,
    camera_heading: f32,
    shake: f32,
    look: (f32, f32),
    camera_orbit: f32,
    camera_lift: f32,
    time_left: f32,
    elapsed: f32,
    next_checkpoint: usize,
    best: f32,
    banners: Vec<Banner>,
    section: usize,
    called: usize,
    fresh_call: f32,
    controls: Controls,
    drag: Option<((i32, i32), (i32, i32))>,
    frame: u32,
    show_stats: bool,
    frame_ms: f32,
    last_render: Option<web_time::Instant>,
    autopilot: bool,
    mode: Mode,
    clock: f32,
    pad_info: String,
    headlight_override: Option<bool>,
    sound: sound::SoundSystem,
    last_throttle: f32,
    hide_hud: bool,
    menu_selection: usize,
    menu_latch: bool,
    livery_index: usize,
    render_ms: f32,
    seed: u64,
}

fn url_parameter(name: &str) -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    {
        let search = web_sys::window()?.location().search().ok()?;
        search.trim_start_matches('?').split('&').find_map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            (key == name).then(|| value.to_string())
        })
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        match name {
            "seed" => std::env::args().nth(1),
            _ => None,
        }
    }
}

fn requested_seed() -> Option<u64> {
    url_parameter("seed")?.parse().ok()
}

fn new_seed() -> u64 {
    let now = web_time::SystemTime::now()
        .duration_since(web_time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(1);
    rng::hash(now, 0x5eed, 1) % 100_000
}

impl Rally {
    fn new(seed: u64) -> Self {
        let mut vram = Vram::new();
        let atlas = Atlas::build(&mut vram, seed);
        let art = art::Art::load(&mut vram);
        let livery_index = storage::load_livery().unwrap_or(seed as usize) % livery::LIVERIES.len();
        let livery = livery::LIVERIES[livery_index];
        let car_atlas = livery::build(&mut vram, (seed % 98) as u32 + 1);
        livery::write_clut(&mut vram, &livery, false);
        let mut track = Track::new(seed);
        let mut car = Car::new(&track);
        if let Some(start) = url_parameter("at").and_then(|value| value.parse::<usize>().ok()) {
            track.ensure(start + 200);
            let sample = track.sample(start);
            car.x = sample.x;
            car.z = sample.z;
            car.reset_to_track(&track);
        }
        let camera_heading = car.heading;
        let mut rally = Self {
            vram,
            atlas,
            art,
            renderer: Renderer::new(),
            world: World::new(seed),
            car,
            mesh: CarMesh::build(&car_atlas),
            livery,
            state: State::Title,
            camera: Camera {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.0,
                roll: 0.0,
            },
            camera_heading,
            shake: 0.0,
            look: (0.0, 0.0),
            camera_orbit: 0.0,
            camera_lift: 0.0,
            time_left: START_TIME,
            elapsed: 0.0,
            next_checkpoint: CHECKPOINT_SPACING,
            best: storage::load_best(),
            banners: Vec::new(),
            section: 0,
            called: 0,
            fresh_call: 0.0,
            controls: Controls::default(),
            drag: None,
            frame: 0,
            show_stats: url_parameter("stats").is_some(),
            frame_ms: 16.7,
            last_render: None,
            autopilot: false,
            mode: Mode::RaceTheClock,
            clock: 0.12,
            pad_info: String::new(),
            headlight_override: None,
            sound: sound::SoundSystem::default(),
            last_throttle: 0.0,
            hide_hud: false,
            menu_selection: 0,
            menu_latch: false,
            livery_index,
            render_ms: 0.0,
            seed,
            track,
        };
        rally
            .atlas
            .update_biome_cluts(&mut rally.vram, &rally.track.biomes, 0.0);
        rally
    }

    fn restart(&mut self, seed: u64) {
        let best = self.best;
        let show_stats = self.show_stats;
        let autopilot = self.autopilot;
        let mode = self.mode;
        let menu_selection = self.menu_selection;
        let sound = std::mem::take(&mut self.sound);
        let clock = self.clock;
        *self = Self::new(seed);
        self.sound = sound;
        self.clock = clock;
        self.best = best;
        self.show_stats = show_stats;
        self.autopilot = autopilot;
        self.mode = mode;
        self.menu_selection = menu_selection;
    }

    fn select_livery(&mut self, step: i32) {
        let count = livery::LIVERIES.len() as i32;
        self.livery_index = (self.livery_index as i32 + step).rem_euclid(count) as usize;
        self.livery = livery::LIVERIES[self.livery_index];
        storage::save_livery(self.livery_index);
        self.sound.trigger(sound::Effect::Call, 0.3, 1.2);
    }

    fn autopilot_controls(&self) -> Controls {
        let car = &self.car;
        let speed = car.speed();
        let look = (8.0 + speed.max(0.0) * 0.5) / track::SAMPLE;
        let anchor = self
            .track
            .surface(car.x, car.z, car.y)
            .1
            .map(|road| road.index)
            .unwrap_or(car.index);
        let target = self.track.sample(anchor + look as usize + 1);
        let (dx, dz) = (target.x - car.x, target.z - car.z);
        let mut error = dx.atan2(dz) - car.heading;
        while error > std::f32::consts::PI {
            error -= std::f32::consts::TAU;
        }
        while error < -std::f32::consts::PI {
            error += std::f32::consts::TAU;
        }
        let grip = self.track.biomes.scalar(car.distance(), |b| b.grip);
        let horizon = anchor + 1..anchor + 2 + (speed.max(10.0) * 2.6 / track::SAMPLE) as usize;
        let curvature = horizon
            .map(|index| self.track.sample(index).curvature.abs())
            .fold(0.004, f32::max);
        let safe_speed = ((grip * 28.0 + 3.0) / curvature).sqrt() * 0.76;
        let notes: Vec<&PaceNote> = self.track.upcoming_notes(anchor, 5).collect();
        let crest_ahead = notes.windows(2).any(|pair| {
            let (crest, after) = (pair[0], pair[1]);
            matches!(crest.kind, track::NoteKind::Crest | track::NoteKind::Jump)
                && crest.start < anchor + (speed.max(10.0) * 2.5 / track::SAMPLE) as usize
                && matches!(
                    after.kind,
                    track::NoteKind::Turn {
                        severity: 0..=3,
                        ..
                    }
                )
                && after.start < crest.end + 20
        });
        let safe_speed = if crest_ahead {
            safe_speed.min(21.0)
        } else {
            safe_speed
        };
        let throttle = ((safe_speed - speed) * 0.4).clamp(-1.0, 1.0);
        Controls {
            throttle,
            steer: (error * 3.0 - car.yaw_rate * 0.15).clamp(-1.0, 1.0),
            handbrake: false,
        }
    }

    fn read_controls(&mut self, input: &Input) {
        let axis = |negative: &[Key], positive: &[Key]| {
            let held = |keys: &[Key]| keys.iter().any(|key| input.held(*key));
            (held(positive) as i32 - held(negative) as i32) as f32
        };
        let mut throttle = axis(&[Key::ArrowDown, Key::KeyS], &[Key::ArrowUp, Key::KeyW]);
        let mut steer = axis(&[Key::ArrowLeft, Key::KeyA], &[Key::ArrowRight, Key::KeyD]);
        let mut handbrake = input.held(Key::Space);
        self.drag = None;
        self.look = input
            .gamepad
            .map(|pad| (pad.stick(2, LOOK_DEAD_ZONE), pad.stick(3, LOOK_DEAD_ZONE)))
            .unwrap_or((0.0, 0.0));
        if let (Some(delta), Some(drag)) = (input.drag_delta(), input.drag) {
            let dead = |value: f32, zone: f32| {
                if value.abs() < zone {
                    0.0
                } else {
                    value - zone * value.signum()
                }
            };
            steer = (dead(delta.0, 0.015) / 0.13).clamp(-1.0, 1.0);
            throttle = (-dead(delta.1, 0.015) / 0.10).clamp(-1.0, 1.0);
            handbrake |= input.touches.len() >= 2;
            let width = SCREEN_WIDTH as u32;
            self.drag = Some((
                input.to_frame(drag.start, width),
                input.to_frame(drag.current, width),
            ));
        }
        if let Some(pad) = input.gamepad {
            use psx_engine::Gamepad as Pad;
            let stick = pad.stick(0, 0.12);
            let pad_steer = stick.signum() * stick.abs().powf(1.5) + pad.value(Pad::RIGHT)
                - pad.value(Pad::LEFT);
            let gas = pad.value(Pad::RT).max(pad.value(Pad::A));
            let brake = pad.value(Pad::LT).max(pad.value(Pad::X));
            if pad_steer != 0.0 {
                steer = pad_steer.clamp(-1.0, 1.0);
            }
            if gas > 0.05 || brake > 0.05 {
                throttle = (gas - brake).clamp(-1.0, 1.0);
            }
            handbrake |=
                pad.held(Pad::B) || pad.held(Pad::Y) || pad.held(Pad::RB) || pad.held(Pad::LB);
        }
        self.controls = Controls {
            throttle,
            steer,
            handbrake,
        };
        if self.autopilot && self.drag.is_none() && throttle == 0.0 && steer == 0.0 {
            self.controls = self.autopilot_controls();
        }
    }

    fn race_update(&mut self) {
        let events = self.car.update(self.controls, &mut self.track);
        let distance = self.car.distance();
        let speed = self.car.speed();

        if events.impact > 1.0 {
            self.sound
                .trigger(sound::Effect::Thud, (events.impact / 10.0).min(1.0), 1.0);
        }
        if events.landed > 3.0 {
            self.sound
                .trigger(sound::Effect::Thud, (events.landed / 12.0).min(0.9), 0.8);
        }
        if self.last_throttle > 0.7 && self.controls.throttle < 0.1 && self.engine_rpm() > 5.0 {
            self.sound.trigger(
                sound::Effect::Pop,
                0.6,
                0.9 + (self.frame % 7) as f32 * 0.04,
            );
            if self.frame.is_multiple_of(2) {
                self.sound.trigger(sound::Effect::Pop, 0.4, 1.15);
            }
        }
        self.last_throttle = self.controls.throttle;
        if events.impact > 2.0 {
            self.shake = (self.shake + events.impact * 0.04).min(0.6);
        }
        if events.landed > 4.0 {
            self.shake = (self.shake + events.landed * 0.02).min(0.5);
        }
        let dust = self.track.biomes.color(distance, |b| b.dust);
        let rough = self.car.ground != Ground::Road;
        let kick = events.sliding * 0.35
            + if rough { speed.abs() * 0.05 } else { 0.0 }
            + if self.controls.throttle > 0.5 && speed < 10.0 {
                0.6
            } else {
                0.0
            };
        if kick > 0.3 && !self.car.airborne && speed.abs() > 2.0 {
            let (fx, fz) = self.car.forward();
            for side in [-0.8f32, 0.8] {
                let (rx, rz) = self.car.right();
                let position = [
                    self.car.x - fx * 1.8 + rx * side,
                    self.car.y,
                    self.car.z - fz * 1.8 + rz * side,
                ];
                self.world.spawn_dust(
                    position,
                    (-self.car.velocity.0, -self.car.velocity.1),
                    dust,
                    kick * 0.25,
                );
            }
        }

        let spray = biome::lerp_rgb(
            self.track.biomes.color(distance, |b| b.sky_horizon),
            [235, 240, 245],
            0.5,
        );
        if events.splash > 2.0 {
            self.sound.trigger(
                sound::Effect::Splash,
                (events.splash / 18.0).min(1.0),
                0.9 + (self.frame % 5) as f32 * 0.05,
            );
        }
        if self.car.water_depth > 0.1 && speed.abs() > 4.0 && self.frame.is_multiple_of(20) {
            self.sound
                .trigger(sound::Effect::Splash, (speed.abs() / 40.0).min(0.6), 1.1);
        }
        if self.car.wet && speed.abs() > 3.0 {
            let (fx, fz) = self.car.forward();
            let (rx, rz) = self.car.right();
            let amount =
                speed.abs() * 0.05 * (1.0 + self.car.water_depth * 6.0) + events.splash * 0.15;
            for side in [-0.8f32, 0.8] {
                let position = [
                    self.car.x + fx * 1.2 + rx * side,
                    self.car.y,
                    self.car.z + fz * 1.2 + rz * side,
                ];
                self.world.spawn_dust(
                    position,
                    (self.car.velocity.0 * 0.5, self.car.velocity.1 * 0.5),
                    spray,
                    amount,
                );
            }
        }

        if self.car.index >= self.next_checkpoint {
            self.next_checkpoint += CHECKPOINT_SPACING;
            let bonus =
                CHECKPOINT_BONUS + (self.track.biomes.section(distance) as f32 * -1.5).max(-14.0);
            let subtitle = match self.mode {
                Mode::RaceTheClock => {
                    self.time_left += bonus;
                    format!("+{bonus:.1} SEC")
                }
                Mode::EnjoyTheScenery => format!("{:.1} KM", self.car.distance() / 1000.0),
            };
            self.banners.push(Banner {
                title: "CHECKPOINT".into(),
                subtitle,
                color: GREEN,
                timer: 2.5,
            });
        }

        let section = self.track.biomes.section(distance + biome::BLEND_LENGTH);
        if section != self.section {
            self.section = section;
            let next = &BIOMES[self.track.biomes.biome_index(section)];
            self.banners.push(Banner {
                title: next.name.into(),
                subtitle: next.surface.to_string(),
                color: CYAN,
                timer: 3.5,
            });
        }

        let call_distance = CALL_AHEAD + speed.max(0.0) * CALL_SECONDS;
        while let Some(note) = self.track.notes.get(self.called) {
            if Track::distance(note.start) - distance < call_distance {
                self.called += 1;
                self.fresh_call = 0.6;
                self.sound.trigger(sound::Effect::Call, 0.5, 1.0);
            } else {
                break;
            }
        }
        self.fresh_call = (self.fresh_call - DT).max(0.0);

        self.elapsed += DT;
        if self.mode == Mode::EnjoyTheScenery {
            return;
        }
        self.time_left -= DT;
        if self.time_left <= 0.0 {
            self.time_left = 0.0;
            self.best = self.best.max(distance);
            storage::save_best(self.best);
            self.state = State::TimeUp(0.0);
        }
    }

    fn update_camera(&mut self) {
        let car = &self.car;
        let title = matches!(self.state, State::Title);
        if title {
            let orbit = self.frame as f32 * 0.006;
            let target = car.heading + orbit.sin() * 2.4 + std::f32::consts::PI * 0.15;
            self.camera_heading = target;
        } else {
            let lag = if car.airborne { 0.04 } else { 0.09 };
            let mut difference = car.heading - self.camera_heading;
            while difference > std::f32::consts::PI {
                difference -= std::f32::consts::TAU;
            }
            while difference < -std::f32::consts::PI {
                difference += std::f32::consts::TAU;
            }
            self.camera_heading += difference * lag;
        }
        let return_rate = (LOOK_RETURN * DT).min(1.0);
        if self.look.0 == 0.0 {
            self.camera_orbit -= self.camera_orbit * return_rate;
        } else {
            self.camera_orbit = (self.camera_orbit + self.look.0 * LOOK_SPEED * DT)
                .clamp(-std::f32::consts::PI, std::f32::consts::PI);
        }
        let lift_target = -self.look.1 * LOOK_LIFT;
        self.camera_lift += (lift_target - self.camera_lift) * return_rate;
        let view_heading = self.camera_heading + self.camera_orbit;
        let (fx, fz) = (view_heading.sin(), view_heading.cos());
        let back = if title { 8.5 } else { 7.2 };
        let height = if title { 2.6 } else { 2.7 } + self.camera_lift;
        let mut x = car.x - fx * back;
        let mut z = car.z - fz * back;
        let ground = self.track.surface(x, z, car.y + 1.0).0;
        let mut y = (car.y + height).max(ground + 1.2);
        if let Some(hit) = self.track.structure_at(x, z, 1.0) {
            if hit.tunnel && car.y < hit.height + track::TUNNEL_HEIGHT {
                y = y.min(hit.height + track::TUNNEL_HEIGHT - 0.6);
            }
        }
        let mut rng = Rng::new(self.frame as u64);
        if self.shake > 0.0 {
            x += rng.range(-1.0, 1.0) * self.shake;
            y += rng.range(-1.0, 1.0) * self.shake;
            z += rng.range(-1.0, 1.0) * self.shake;
            self.shake = (self.shake - DT * 1.5).max(0.0);
        }
        let lead = 4.0 * self.camera_orbit.cos().max(0.0);
        let target = (
            car.x + car.forward().0 * lead,
            car.y + 1.0,
            car.z + car.forward().1 * lead,
        );
        let (dx, dy, dz) = (target.0 - x, target.1 - y, target.2 - z);
        self.camera = Camera {
            x,
            y,
            z,
            yaw: dx.atan2(dz),
            pitch: (-dy).atan2((dx * dx + dz * dz).sqrt()),
            roll: if title { 0.0 } else { -car.roll * 0.3 },
        };
    }

    fn engine_rpm(&self) -> f32 {
        let speed = self.car.speed();
        let gear_position = speed.abs() / 52.0 * 5.0;
        if speed.abs() < 0.5 {
            1.0 + self.controls.throttle.max(0.0) * 3.0
        } else if gear_position >= 5.0 {
            7.5
        } else {
            2.2 + gear_position.fract() * 5.3
        }
    }

    fn headlights_on(&self) -> bool {
        self.headlight_override
            .unwrap_or(daytime::phase(self.clock).daylight < 0.55)
    }

    fn lighting(&self, distance: f32) -> Lighting {
        let biomes = &self.track.biomes;
        let phase = daytime::phase(self.clock);
        let sky = daytime::apply(
            phase,
            daytime::Sky {
                top: biomes.color(distance, |b| b.sky_top),
                horizon: biomes.color(distance, |b| b.sky_horizon),
                fog: biomes.color(distance, |b| b.fog),
                sun: biomes.color(distance, |b| b.sun),
                ambient: biomes.color(distance, |b| b.ambient),
            },
        );
        let headlight = self.headlights_on().then(|| world::Headlight {
            x: self.car.x,
            z: self.car.z,
            forward: self.car.forward(),
            strength: 1.0 - phase.daylight * 0.7,
        });
        Lighting {
            sun: sky.sun,
            ambient: sky.ambient,
            fog: sky.fog,
            sun_direction: world::sun_direction_at(0.9, phase.elevation),
            sky_top: sky.top,
            sky_horizon: sky.horizon,
            daylight: phase.daylight,
            headlight,
        }
    }

    fn draw_hud(&mut self, distance: f32) {
        let rpm_for_hud = self.engine_rpm();
        if self.hide_hud {
            return;
        }
        let upcoming: Vec<PaceNote> = self
            .track
            .upcoming_notes(self.car.index, 6)
            .filter(|note| note.start > self.car.index)
            .filter(|note| {
                self.track
                    .notes
                    .iter()
                    .position(|n| n == *note)
                    .is_some_and(|position| position < self.called)
            })
            .copied()
            .collect();
        let mut hud = Hud {
            overlay: &mut self.renderer.overlay,
            atlas: &self.atlas,
        };
        match self.state {
            State::Title => {
                hud.centered(28, "PSX RALLY", YELLOW, Size::Huge);
                hud.centered(68, &format!("STAGE {:05}", self.seed), WHITE, Size::Large);
                if self.best > 0.0 {
                    hud.centered(
                        88,
                        &format!("BEST {:.2} KM", self.best / 1000.0),
                        CYAN,
                        Size::Small,
                    );
                }
                let livery_y = MENU_Y + MODES.len() as i16 * MENU_SPACING;
                hud.centered(
                    livery_y,
                    &format!("< {} >", self.livery.name),
                    CYAN,
                    Size::Large,
                );
                for (index, (_, label)) in MODES.iter().enumerate() {
                    let y = MENU_Y + index as i16 * MENU_SPACING;
                    if index == self.menu_selection {
                        let blink = (self.frame / 20).is_multiple_of(2);
                        let text = if blink {
                            format!("> {label} <")
                        } else {
                            format!("  {label}  ")
                        };
                        hud.centered(y, &text, YELLOW, Size::Large);
                    } else {
                        hud.centered(y, label, [90, 90, 90], Size::Large);
                    }
                }
                hud.centered(176, "DRAG UP: GAS   DRAG DOWN: BRAKE", WHITE, Size::Small);
                hud.centered(187, "DRAG SIDEWAYS: STEER", WHITE, Size::Small);
                hud.centered(
                    198,
                    "2ND FINGER / SPACE: HANDBRAKE   R: RESET",
                    WHITE,
                    Size::Small,
                );
                hud.centered(
                    209,
                    "PAD: RT GAS  LT BRAKE  B HANDBRAKE  Y RESET",
                    WHITE,
                    Size::Small,
                );
                hud.centered(
                    220,
                    "TAP A MODE OR START   LEFT / RIGHT: CAR",
                    WHITE,
                    Size::Small,
                );
                return;
            }
            State::Countdown(remaining) => {
                let number = remaining.ceil() as i32;
                let text = if number > 0 {
                    number.to_string()
                } else {
                    "GO!".to_string()
                };
                hud.centered(90, &text, YELLOW, Size::Huge);
            }
            State::TimeUp(_) => {
                hud.centered(62, "TIME UP", RED, Size::Huge);
                hud.centered(
                    104,
                    &format!("DISTANCE {:.2} KM", distance / 1000.0),
                    WHITE,
                    Size::Large,
                );
                hud.centered(
                    122,
                    &format!("BEST {:.2} KM", self.best / 1000.0),
                    CYAN,
                    Size::Large,
                );
                if (self.frame / 30).is_multiple_of(2) {
                    hud.centered(150, "PRESS START", WHITE, Size::Large);
                }
            }
            State::Racing => {}
        }

        let low_time = self.time_left < 10.0 && (self.frame / 15).is_multiple_of(2);
        match self.mode {
            Mode::RaceTheClock => {
                hud.large(6, 3, "TIME", YELLOW);
                hud.large(
                    6,
                    19,
                    &format!("{:.1}", self.time_left),
                    if low_time { RED } else { WHITE },
                );
            }
            Mode::EnjoyTheScenery => {
                let seconds = self.elapsed as u32;
                hud.large(6, 3, "DRIVE", YELLOW);
                hud.large(
                    6,
                    19,
                    &format!("{}:{:02}", seconds / 60, seconds % 60),
                    WHITE,
                );
            }
        }
        let right = SCREEN_WIDTH as i16 - 4;
        hud.right_aligned(right, 3, "STAGE", YELLOW, Size::Large);
        hud.text(right - 14, 27, "KM", WHITE);
        hud.right_aligned(
            right - 15,
            19,
            &format!("{:.2}", distance / 1000.0),
            WHITE,
            Size::Large,
        );

        let speed = self.car.speed();
        let gear_position = speed.abs() / 52.0 * 5.0;
        let gear = if speed < -0.5 {
            "R".to_string()
        } else {
            ((gear_position as i32).clamp(0, 4) + 1).to_string()
        };
        let rpm = rpm_for_hud;
        hud.tachometer(
            (SCREEN_WIDTH as i16 - 48, SCREEN_HEIGHT as i16 - 46),
            rpm,
            &gear,
            (speed.abs() * 3.6) as i32,
            self.controls.handbrake,
        );

        if let Some(note) = upcoming.first() {
            let to_go = Track::distance(note.start) - distance;
            hud.note(
                88,
                5,
                note,
                to_go,
                self.fresh_call > 0.0 && (self.frame / 4).is_multiple_of(2),
                false,
            );
            if let Some(next) = upcoming.get(1) {
                hud.note(94, 40, next, 0.0, false, true);
            }
        }

        let mut banner_y = SCREEN_HEIGHT as i16 - 34;
        for banner in self.banners.iter().take(2) {
            hud.large(6, banner_y, &banner.title, banner.color);
            hud.text(8, banner_y + 18, &banner.subtitle, WHITE);
            banner_y -= 32;
        }

        if let Some((start, current)) = self.drag {
            hud.drag_indicator(start, current);
        }
        if self.show_stats {
            let text = format!(
                "{:2.0}FPS {:3.1}MS T{:.2} I{} D{:.0} V{:.0}",
                1000.0 / self.frame_ms.max(1.0),
                self.render_ms,
                self.clock,
                self.car.index,
                self.track
                    .terrain(self.car.x, self.car.z)
                    .road
                    .map(|road| road.distance)
                    .unwrap_or(-1.0),
                self.car.speed()
            );
            hud.text(6, 38, &text, GREEN);
            hud.text(6, 48, &self.pad_info, GREEN);
            let location = format!(
                "SEED {} AT {} X{:.0} Z{:.0} Y{:.0}",
                self.seed, self.car.index, self.car.x, self.car.z, self.car.y
            );
            hud.text(6, 228, &location, GREEN);
        }
    }
}

impl Game for Rally {
    fn audio(&mut self, audio: &mut psx_engine::Audio) {
        let distance = self.car.distance();
        let (a, b, t) = self.track.biomes.at(distance);
        let biome = if t < 0.5 { &BIOMES[a] } else { &BIOMES[b] };
        let weather_amount = if t < 0.5 {
            1.0 - t * 2.0
        } else {
            t * 2.0 - 1.0
        };
        let racing = matches!(
            self.state,
            State::Racing | State::Countdown(_) | State::TimeUp(_)
        );
        let mix = sound::Mix {
            rpm: self.engine_rpm() * 1000.0,
            throttle: self.controls.throttle,
            speed: self.car.speed().abs(),
            slide: self.car.slide,
            handbrake: self.controls.handbrake,
            airborne: self.car.airborne,
            ground: Some(self.car.ground),
            surface: Some(sound::Surface::from_name(biome.surface)),
            weather: Some(biome.weather),
            weather_amount,
            engine_volume: if racing { 1.0 } else { 0.5 },
        };
        self.sound.update(audio, &mix);
    }

    fn update(&mut self, input: &Input) {
        self.frame = self.frame.wrapping_add(1);
        if input.just_pressed(Key::F1) {
            self.show_stats = !self.show_stats;
        }
        if input.just_pressed(Key::F2) {
            self.renderer.fog_enabled = !self.renderer.fog_enabled;
        }
        if input.just_pressed(Key::Escape) && self.state != State::Title {
            self.restart(new_seed());
            self.state = State::Title;
            return;
        }
        if input.just_pressed(Key::F5) || input.pad_pressed(psx_engine::Gamepad::SELECT) {
            self.hide_hud = !self.hide_hud;
        }
        if input.just_pressed(Key::KeyM) {
            self.sound.muted = !self.sound.muted;
        }
        self.pad_info = input
            .gamepad
            .map(|pad| {
                format!(
                    "PAD {} {:+.1} {:+.1}",
                    if pad.standard { "STD" } else { "RAW" },
                    pad.axes[0],
                    pad.axes[1]
                )
            })
            .unwrap_or_default();
        if input.just_pressed(Key::KeyL) {
            self.headlight_override = Some(!self.headlights_on());
        }
        if input.just_pressed(Key::KeyT) {
            self.clock = (self.clock + 0.125).rem_euclid(1.0);
        }
        if input.just_pressed(Key::KeyN) {
            self.sound.music = !self.sound.music;
        }
        if input.just_pressed(Key::F4) {
            self.autopilot = !self.autopilot;
        }
        if input.just_pressed(Key::F3) {
            let shift = (self.renderer.ot.shift() + 2) % 8;
            self.renderer.ot.set_shift(shift);
        }
        self.read_controls(input);
        let confirm = input.just_pressed(Key::Enter)
            || input.tapped
            || input.pad_pressed(psx_engine::Gamepad::START)
            || input.pad_pressed(psx_engine::Gamepad::A);
        match self.state {
            State::Title => {
                self.controls = self.autopilot_controls();
                self.controls.throttle = self.controls.throttle.min(0.55);
                self.car.update(self.controls, &mut self.track);
                let vertical = input.gamepad.map(|pad| pad.stick(1, 0.5)).unwrap_or(0.0);
                let up = input.just_pressed(Key::ArrowUp)
                    || input.just_pressed(Key::KeyW)
                    || input.pad_pressed(psx_engine::Gamepad::UP)
                    || (vertical < 0.0 && !self.menu_latch);
                let down = input.just_pressed(Key::ArrowDown)
                    || input.just_pressed(Key::KeyS)
                    || input.pad_pressed(psx_engine::Gamepad::DOWN)
                    || (vertical > 0.0 && !self.menu_latch);
                let horizontal = input.gamepad.map(|pad| pad.stick(0, 0.5)).unwrap_or(0.0);
                let mut cycle = 0i32;
                if input.just_pressed(Key::ArrowLeft)
                    || input.pad_pressed(psx_engine::Gamepad::LEFT)
                    || (horizontal < 0.0 && !self.menu_latch)
                {
                    cycle = -1;
                }
                if input.just_pressed(Key::ArrowRight)
                    || input.pad_pressed(psx_engine::Gamepad::RIGHT)
                    || (horizontal > 0.0 && !self.menu_latch)
                {
                    cycle = 1;
                }
                self.menu_latch = vertical != 0.0 || horizontal != 0.0;
                if input.tapped {
                    if let Some(drag) = input.drag {
                        let (x, y) = input.to_frame(drag.start, SCREEN_WIDTH as u32);
                        let row = (y as i16 - MENU_Y + 4).div_euclid(MENU_SPACING);
                        if row == MODES.len() as i16 {
                            cycle = if x < SCREEN_WIDTH / 2 { -1 } else { 1 };
                        }
                    }
                }
                if cycle != 0 {
                    self.select_livery(cycle);
                }
                if up || down {
                    self.menu_selection = (self.menu_selection + 1) % MODES.len();
                    self.sound.trigger(sound::Effect::Call, 0.3, 1.5);
                }
                if input.tapped {
                    if let Some(drag) = input.drag {
                        let (_, y) = input.to_frame(drag.start, SCREEN_WIDTH as u32);
                        let row = (y as i16 - MENU_Y + 4).div_euclid(MENU_SPACING);
                        if (0..MODES.len() as i16).contains(&row) {
                            self.menu_selection = row as usize;
                        }
                    }
                }
                let on_livery_row = input.tapped
                    && input.drag.is_some_and(|drag| {
                        let (_, y) = input.to_frame(drag.start, SCREEN_WIDTH as u32);
                        (y as i16 - MENU_Y + 4).div_euclid(MENU_SPACING) == MODES.len() as i16
                    });
                if confirm && !on_livery_row {
                    self.mode = MODES[self.menu_selection].0;
                    self.restart(self.seed);
                    self.state = State::Countdown(3.0);
                    return;
                }
            }
            State::Countdown(remaining) => {
                let remaining = remaining - DT;
                self.controls.steer = 0.0;
                self.controls.throttle = 0.0;
                self.car.update(self.controls, &mut self.track);
                self.state = if remaining <= -0.4 {
                    State::Racing
                } else {
                    State::Countdown(remaining)
                };
            }
            State::Racing => {
                let tapped_reset = input.tapped
                    && input.drag.is_some_and(|drag| {
                        let (x, y) = input.to_frame(drag.start, SCREEN_WIDTH as u32);
                        x > SCREEN_WIDTH - 90 && y < 44
                    });
                if input.just_pressed(Key::KeyR)
                    || input.pad_pressed(psx_engine::Gamepad::Y)
                    || tapped_reset
                {
                    self.car.reset_to_track(&self.track);
                    self.camera_heading = self.car.heading;
                }
                self.race_update();
            }
            State::TimeUp(elapsed) => {
                self.controls = Controls {
                    throttle: if self.car.speed() > 0.5 { -1.0 } else { 0.0 },
                    steer: 0.0,
                    handbrake: false,
                };
                self.car.update(self.controls, &mut self.track);
                let elapsed = elapsed + DT;
                self.state = State::TimeUp(elapsed);
                if confirm && elapsed > 1.0 {
                    self.restart(new_seed());
                    self.state = State::Title;
                    return;
                }
            }
        }
        for banner in &mut self.banners {
            banner.timer -= DT;
        }
        self.banners.retain(|banner| banner.timer > 0.0);
        let distance = self.car.distance();
        let (a, b, t) = self.track.biomes.at(distance);
        let weather = if t < 0.5 {
            BIOMES[a].weather
        } else {
            BIOMES[b].weather
        };
        let previous_heading = self.camera.yaw;
        self.update_camera();
        let turn = self.camera.yaw - previous_heading;
        self.world
            .update(DT, weather, turn.clamp(-0.1, 0.1), self.car.speed());
        let intensity = if t < 0.5 {
            1.0 - t * 2.0
        } else {
            t * 2.0 - 1.0
        };
        let shown = if self.state == State::Title && weather == Weather::Rain {
            Weather::Clear
        } else {
            weather
        };
        self.world
            .update_precipitation(DT, shown, intensity, &self.camera);
        livery::write_clut(
            &mut self.vram,
            &self.livery,
            self.controls.throttle < -0.1 || self.controls.handbrake,
        );
        if self.frame.is_multiple_of(2) {
            self.atlas
                .update_biome_cluts(&mut self.vram, &self.track.biomes, distance);
        }
        self.clock = (self.clock + DT / daytime::CYCLE_SECONDS).rem_euclid(1.0);
        let fog = self.lighting(distance).fog;
        self.atlas.update_fog_cluts(&mut self.vram, fog);
        self.art.sheet.update_fog(&mut self.vram, fog);
    }

    fn render(&mut self, framebuffer: &mut Framebuffer) {
        let started = web_time::Instant::now();
        if let Some(last) = self.last_render {
            let interval = (started - last).as_secs_f32() * 1000.0;
            self.frame_ms = self.frame_ms * 0.9 + interval * 0.1;
        }
        self.last_render = Some(started);
        let distance = self.car.distance();
        let section = self.track.biomes.section(distance);
        let into = distance - section as f32 * biome::SECTION_LENGTH;
        let first = if into < BLEND_HOLD && section > 0 {
            section - 1
        } else {
            section
        };
        self.art.blends.prepare(
            &mut self.vram,
            (
                self.track.biomes.biome_index(first),
                self.track.biomes.biome_index(first + 1),
            ),
        );
        let lighting = self.lighting(distance);
        self.renderer.begin(&self.camera, lighting.fog);
        let draw_index = self
            .track
            .locate(
                self.camera.x,
                self.camera.z,
                self.car.index.saturating_sub(3),
            )
            .0;
        self.world.draw(
            &mut self.renderer,
            &self.vram,
            &self.atlas,
            &self.art,
            &self.track,
            &self.camera,
            draw_index,
            lighting,
        );
        if let Some(beam) = lighting.headlight {
            let braking = self.controls.throttle < -0.1 || self.controls.handbrake;
            draw_lamps(
                &mut self.renderer,
                &self.atlas,
                &self.car,
                &self.camera,
                beam.strength,
                braking,
            );
        }
        self.mesh.draw(
            &mut self.renderer,
            &self.car,
            lighting.sun_direction,
            lighting.sun,
            lighting.ambient,
        );
        let shadow_color = scale_rgb([255, 255, 255], 0.22);
        draw_shadow(
            &mut self.renderer,
            &self.car,
            &self.world,
            &self.track,
            shadow_color,
        );

        let (a, b, t) = self.track.biomes.at(distance);
        let (weather, intensity) = if t < 0.5 {
            (BIOMES[a].weather, 1.0 - t * 2.0)
        } else {
            (BIOMES[b].weather, t * 2.0 - 1.0)
        };
        let sheltered = self
            .track
            .structure_at(self.camera.x, self.camera.z, 1.0)
            .is_some_and(|hit| hit.tunnel && self.camera.y < hit.height + track::TUNNEL_HEIGHT);
        let weather = if (self.state == State::Title && weather == Weather::Rain) || sheltered {
            Weather::Clear
        } else {
            weather
        };
        self.world
            .draw_precipitation(&mut self.renderer, weather, self.car.velocity, lighting.fog);
        self.world
            .draw_weather(&mut self.renderer.overlay, weather, intensity);
        self.draw_hud(distance);

        let buffer_y = (self.frame % 2) as i32 * SCREEN_HEIGHT;
        let env = DrawEnv::new(0, buffer_y, SCREEN_WIDTH, SCREEN_HEIGHT);
        let fog = lighting.fog;
        fill_rect(
            &mut self.vram,
            0,
            buffer_y as usize,
            SCREEN_WIDTH as usize,
            SCREEN_HEIGHT as usize,
            psx_vram::bgr555(fog[0], fog[1], fog[2]),
        );
        draw_all(&mut self.vram, &env, self.renderer.ot.draw_order());
        draw_all(&mut self.vram, &env, &self.renderer.overlay);

        self.render_ms = self.render_ms * 0.9 + started.elapsed().as_secs_f32() * 100.0;
        let width = framebuffer.width.min(SCREEN_WIDTH as usize);
        for row in 0..framebuffer.height {
            let source = (buffer_y as usize + row) * psx_vram::VRAM_WIDTH;
            framebuffer.pixels[row * framebuffer.width..row * framebuffer.width + width]
                .copy_from_slice(&self.vram.pixels[source..source + width]);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_lamps(
    renderer: &mut Renderer,
    atlas: &Atlas,
    car: &Car,
    camera: &render::Camera,
    strength: f32,
    braking: bool,
) {
    use psx_math::{Mat3Fx12, Vec3i16};
    use render::{units, world_to_psx, Point, Surface};
    let puff = atlas.puff;
    let (fx, fz) = car.forward();
    let (rx, rz) = car.right();
    let right = (camera.yaw.cos(), -camera.yaw.sin());
    let view = (camera.yaw.sin(), camera.yaw.cos());
    let base_y = car.y + 0.08 - car.suspension;
    let fog_enabled = renderer.fog_enabled;
    renderer.fog_enabled = false;
    let glow = |renderer: &mut Renderer,
                forward: f32,
                side: f32,
                up: f32,
                size: f32,
                color: [u8; 3],
                bias: i32| {
        let facing = (fx * view.0 + fz * view.1) * forward.signum();
        if facing > 0.2 {
            return;
        }
        let x = car.x + fx * forward + rx * side;
        let z = car.z + fz * forward + rz * side;
        renderer.set_object(world_to_psx(x, base_y + up, z), Mat3Fx12::IDENTITY);
        let half = size * 0.5;
        let (dx, dz) = (units(right.0 * half) as i16, units(right.1 * half) as i16);
        let h = units(half) as i16;
        let (u0, v0, u1, v1) = (puff.u, puff.v, puff.u + 15, puff.v + 15);
        let points = [
            Point::new(Vec3i16::new(-dx, -h, -dz), color, (u0, v0)),
            Point::new(Vec3i16::new(dx, -h, dz), color, (u1, v0)),
            Point::new(Vec3i16::new(-dx, h, -dz), color, (u0, v1)),
            Point::new(Vec3i16::new(dx, h, dz), color, (u1, v1)),
        ];
        renderer.polygon(
            &points,
            Surface::textured(puff.texture).blended(psx_prim::BlendMode::Add),
            bias,
            0,
        );
    };
    let white = render_scale([255, 240, 200], strength);
    for side in [-0.27f32, 0.27] {
        glow(renderer, 1.92, side, 0.48, 0.5, white, -430);
    }
    for side in [-0.31f32, 0.31] {
        glow(renderer, 1.97, side, 0.22, 0.5, white, -430);
    }
    for side in [-0.48f32, 0.48] {
        glow(renderer, 1.8, side, 0.53, 0.45, white, -430);
    }
    let tail = if braking {
        [255, 40, 30]
    } else {
        [150, 20, 15]
    };
    for side in [-0.51f32, 0.51] {
        glow(
            renderer,
            -2.2,
            side,
            0.55,
            if braking { 0.45 } else { 0.28 },
            render_scale(tail, strength.max(0.4)),
            -470,
        );
    }
    renderer.fog_enabled = fog_enabled;
}

fn render_scale(color: [u8; 3], amount: f32) -> [u8; 3] {
    color.map(|c| (c as f32 * amount.clamp(0.0, 1.0)) as u8)
}

fn draw_shadow(renderer: &mut Renderer, car: &Car, world: &World, track: &Track, color: [u8; 3]) {
    use psx_math::{Mat3Fx12, Vec3i16};
    use render::{world_to_psx, Point, Surface};
    let (fx, fz) = car.forward();
    let (rx, rz) = car.right();
    let corner = |forward: f32, side: f32| {
        let x = car.x + fx * forward + rx * side;
        let z = car.z + fz * forward + rz * side;
        (
            x,
            match track.structure_at(x, z, 0.5) {
                Some(_) => track.surface(x, z, car.y + 0.5).0,
                None => world
                    .landscape
                    .height_at(track, x, z)
                    .max(track.terrain(x, z).height),
            } + 0.06,
            z,
        )
    };
    let origin = world_to_psx(car.x, car.y, car.z);
    renderer.set_object(origin, Mat3Fx12::IDENTITY);
    let corners = [
        corner(2.3, -1.05),
        corner(2.3, 1.05),
        corner(-2.3, -1.05),
        corner(-2.3, 1.05),
    ];
    let points: Vec<Point> = corners
        .iter()
        .map(|&(x, y, z)| {
            let position = world_to_psx(x, y, z);
            Point::new(
                Vec3i16::new(
                    (position.x - origin.x) as i16,
                    (position.y - origin.y) as i16,
                    (position.z - origin.z) as i16,
                ),
                color,
                (0, 0),
            )
        })
        .collect();
    let fog_enabled = renderer.fog_enabled;
    renderer.fog_enabled = false;
    renderer.polygon(
        &points,
        Surface::flat().blended(psx_prim::BlendMode::Subtract),
        -360,
        1,
    );
    renderer.fog_enabled = fog_enabled;
}

fn main() {
    psx_engine::run(
        "PSX Rally",
        Rally::new(requested_seed().unwrap_or_else(new_seed)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn car_drives_out_of_stream() {
        let mut track = Track::new(8);
        track.ensure(800);
        let ford = track.fords[0].clone();
        let mut car = Car::new(&track);
        let point = ford.path[ford.crossing + 4];
        let next = ford.path[ford.crossing + 5];
        let (x, z) = (point.x, point.z);
        car.x = x;
        car.z = z;
        car.y = track.terrain(x, z).height;
        car.heading = (next.x - x).atan2(next.z - z);
        car.index = track.terrain(x, z).road.unwrap().index;
        let controls = Controls {
            throttle: 1.0,
            steer: 0.0,
            handbrake: false,
        };
        let mut deepest = 0.0f32;
        for _ in 0..180 {
            car.update(controls, &mut track);
            deepest = deepest.max(car.water_depth);
        }
        assert!(deepest > 0.2, "{deepest}");
        assert!(car.speed() > 9.0, "{}", car.speed());
    }

    #[test]
    fn autopilot_soak_keeps_car_and_camera_sane() {
        let input = Input::default();
        let mut framebuffer = Framebuffer {
            width: SCREEN_WIDTH as usize,
            height: SCREEN_HEIGHT as usize,
            pixels: vec![0; (SCREEN_WIDTH * SCREEN_HEIGHT) as usize],
        };
        let mut failures = Vec::new();
        for seed in 0..10u64 {
            let mut rally = Rally::new(seed);
            rally.mode = Mode::EnjoyTheScenery;
            rally.state = State::Racing;
            rally.autopilot = true;
            for tick in 0..3600 {
                rally.update(&input);
                if tick % 600 == 0 {
                    rally.render(&mut framebuffer);
                }
                let car = &rally.car;
                let index = car.index;
                let ground = rally.track.surface(car.x, car.z, car.y + 0.5).0;
                let camera = rally.camera;
                let camera_ground = rally.track.surface(camera.x, camera.z, camera.y).0;
                let problem = if car.y < ground - 1.0 {
                    Some(format!("car below ground by {:.1}", ground - car.y))
                } else if camera.y < camera_ground + 0.3 {
                    Some(format!(
                        "camera in ground ({:.1} vs {:.1})",
                        camera.y, camera_ground
                    ))
                } else {
                    None
                };
                if let Some(problem) = problem {
                    failures.push(format!(
                        "seed {seed} tick {tick} index {index} {:?}: {problem}",
                        rally.track.character(index)
                    ));
                    break;
                }
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
    }
}
