use std::collections::HashMap;

use crate::biome::BiomeSchedule;
use crate::rng::{value_noise, value_noise_2d, Rng};

pub const SAMPLE: f32 = 4.0;
pub const ROAD_HALF: f32 = 4.5;
pub const SHOULDER: f32 = 6.5;
pub const BOUNDARY: f32 = 12.5;
pub const CHECKPOINT_SPACING: usize = 400;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
}

impl Direction {
    pub fn sign(self) -> f32 {
        match self {
            Self::Left => -1.0,
            Self::Right => 1.0,
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Self::Left => "LEFT",
            Self::Right => "RIGHT",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteKind {
    Turn { direction: Direction, severity: u8 },
    Crest,
    Jump,
    Ford,
    Bridge,
    Tunnel,
    Ice,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PaceNote {
    pub start: usize,
    pub end: usize,
    pub kind: NoteKind,
    pub long: bool,
    pub tightens: bool,
    pub into: bool,
}

impl PaceNote {
    pub fn caution(&self) -> bool {
        matches!(
            self.kind,
            NoteKind::Jump
                | NoteKind::Ford
                | NoteKind::Ice
                | NoteKind::Turn {
                    severity: 0..=1,
                    ..
                }
        )
    }

    pub fn modifiers(&self) -> String {
        let mut words = Vec::new();
        if self.long {
            words.push("LONG");
        }
        if self.tightens {
            words.push("TIGHTENS");
        }
        if self.into {
            words.push("INTO");
        }
        words.join(" ")
    }

    pub fn call(&self) -> String {
        let mut words: Vec<String> = Vec::new();
        match self.kind {
            NoteKind::Turn {
                direction,
                severity: 0,
            } => {
                words.push("HAIRPIN".into());
                words.push(direction.word().into());
            }
            NoteKind::Turn {
                direction,
                severity,
            } => {
                words.push(direction.word().into());
                words.push(severity.to_string());
            }
            NoteKind::Crest => words.push("OVER CREST".into()),
            NoteKind::Jump => words.push("JUMP!".into()),
            NoteKind::Ford => words.push("WATER SPLASH".into()),
            NoteKind::Bridge => words.push("OVER BRIDGE".into()),
            NoteKind::Tunnel => words.push("TUNNEL".into()),
            NoteKind::Ice => words.push("ICE!".into()),
        }
        words.join(" ")
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Sample {
    pub x: f32,
    pub z: f32,
    pub y: f32,
    pub heading: f32,
    pub curvature: f32,
    pub clearance: [f32; 2],
    pub fold: [f32; 2],
    pub deck: bool,
    pub tunnel: bool,
}

impl Sample {
    pub fn forward(&self) -> (f32, f32) {
        (self.heading.sin(), self.heading.cos())
    }

    pub fn right(&self) -> (f32, f32) {
        (self.heading.cos(), -self.heading.sin())
    }

    pub fn clearance(&self, lateral: f32) -> f32 {
        self.clearance[usize::from(lateral > 0.0)]
    }

    pub fn fold(&self, lateral: f32) -> f32 {
        self.fold[usize::from(lateral > 0.0)]
    }

    pub fn sink(&self, lateral: f32) -> f32 {
        let beyond = lateral.abs() - self.clearance(lateral);
        if beyond > 0.0 {
            -(beyond * 1.2).min(45.0)
        } else {
            0.0
        }
    }

    pub fn offset(&self, lateral: f32) -> (f32, f32) {
        let (rx, rz) = self.right();
        (self.x + rx * lateral, self.z + rz * lateral)
    }
}

struct Bump {
    center: f32,
    height: f32,
    width: f32,
    flat: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct StreamPoint {
    pub x: f32,
    pub z: f32,
    pub surface: f32,
    pub width: f32,
}

#[derive(Clone, Debug)]
pub struct Ford {
    pub x: f32,
    pub z: f32,
    pub level: f32,
    pub crossing: usize,
    pub frozen: bool,
    pub path: Vec<StreamPoint>,
    bounds: (f32, f32, f32, f32),
}

struct StreamHit {
    distance: f32,
    surface: f32,
    width: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Puddle {
    pub x: f32,
    pub z: f32,
    pub y: f32,
    pub heading: f32,
    pub radius: f32,
    pub frozen: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Crossing {
    pub old: usize,
    pub center: f32,
    pub end: f32,
    pub old_upper: bool,
}

pub struct StructureHit {
    pub height: f32,
    pub road: RoadPoint,
    pub tunnel: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Ravine {
    pub x: f32,
    pub z: f32,
    pub line: (f32, f32),
    pub road_y: f32,
    pub depth: f32,
    pub half_top: f32,
    pub half_bed: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Tunnel {
    pub start: usize,
    pub end: usize,
}

pub const STRUCTURE_KERB: f32 = 0.6;
pub const TUNNEL_HEIGHT: f32 = 5.0;
const RAVINE_CHANCE: f32 = 0.2;
const RAVINE_REACH: f32 = 120.0;
const RAVINE_FADE: f32 = 30.0;
const RAVINE_RIM: f32 = 80.0;
const RIVERBED_MARGIN: f32 = 2.0;
const ICE_PATCH_CHANCE: f32 = 0.06;
const WALL_START: usize = 40;
const WALL_RUN: usize = 28;
const WALL_CHANCE: f32 = 0.22;
const WALL_OFFSET: f32 = 0.8;
pub const WALL_HEIGHT: f32 = 1.1;
pub const WALL_THICKNESS: f32 = 0.6;
const BARRIER_HEIGHT: f32 = 0.8;
const BARRIER_THICKNESS: f32 = 0.3;
const BARRIER_OFFSET: f32 = 0.3;
const BARRIER_CURVATURE: f32 = 1.0 / 30.0;

#[derive(Clone, Copy, Debug)]
pub struct Obstacle {
    pub lateral: f32,
    pub height: f32,
    pub thickness: f32,
    pub barrier: bool,
}
const RAVINE_RIVER_CHANCE: f32 = 0.5;
const TUNNEL_CHANCE: f32 = 0.35;
const TUNNEL_COVER: f32 = 9.0;
const TUNNEL_SHOULDER: f32 = 14.0;
const TUNNEL_SPREAD: f32 = 45.0;
const PORTAL_FACE: f32 = 6.0;
const PORTAL_SET: f32 = 3.0;
const PORTAL_RAMP: f32 = 3.0;
const SLOT_REACH: f32 = 0.5;
const SLOT_MARGIN: f32 = 1.5;
pub const TUNNEL_ROOF_HALF: f32 = 15.0;
pub const TUNNEL_COVER_HEIGHT: f32 = TUNNEL_COVER;

const BRIDGE_CLEARANCE: f32 = 7.5;
const BRIDGE_RAMP: f32 = 42.0;
const DECK_REACH: f32 = 18.0;
const DECK_RISE: f32 = 3.0;
const DECK_STEP_UP: f32 = 1.2;
const DECK_BLEND: f32 = 1.0;
const ROAD_REACH: f32 = 130.0;
const CROSSING_ARC: usize = 40;

const FORD_DEPTH: f32 = 1.2;
const FORD_WIDTH: f32 = 7.0;
const FORD_FLAT: f32 = 5.0;
const FORD_WATER: f32 = 0.35;
pub const STREAM_HALF_WIDTH: f32 = 4.5;
const STREAM_STEP: f32 = 6.0;
const STREAM_STEPS: usize = 30;
const STREAM_BED: f32 = 0.6;
const STREAM_BANK: f32 = 12.0;

pub struct Track {
    pub seed: u64,
    pub samples: Vec<Sample>,
    pub notes: Vec<PaceNote>,
    pub biomes: BiomeSchedule,
    rng: Rng,
    bumps: Vec<Bump>,
    pub fords: Vec<Ford>,
    pub crossings: Vec<Crossing>,
    pub ravines: Vec<Ravine>,
    pub rivers: Vec<Ford>,
    pub tunnels: Vec<Tunnel>,
    structures: Vec<(usize, usize)>,
    pub dirty: Vec<(f32, f32, f32, f32)>,
    general_heading: f32,
    curvature: f32,
    phases: [f32; 3],
    grid: HashMap<(i32, i32), Vec<u32>>,
}

const GRID_CELL: f32 = 64.0;
const GRID_REACH: i32 = 3;
const LOCAL_SAMPLES: usize = 8;
const INSIDE_BEND_REACH: usize = 10;
pub const MAX_TERRAIN: f32 = 160.0;
pub const MIN_CLEARANCE: f32 = BOUNDARY + 1.5;
const SECTION_LENGTH: f32 = 260.0;
const MAX_DEVIATION: f32 = 1.9;
const GENERAL_DRIFT: f32 = 0.7;
const PREVIEW_MARGIN: usize = 10;
const LOOP_RATIO: f32 = 0.3;
const PREVIEW_CLEARANCE: f32 = 35.0;
const SECTION_FADE: f32 = 45.0;

const FAR_BLEND_START: f32 = 30.0;
const FAR_BLEND_LENGTH: f32 = 70.0;
const BLEND_RANGE: f32 = 30.0;
const BLEND_SOFTNESS: f32 = 9.0;
const OVERSHOOT_SOFTNESS: f32 = 1.5;
const NEAR_ROAD: f32 = 40.0;
const NEAR_ROAD_CURVE: f32 = 0.12;
const NEAR_ROAD_RAMP: f32 = 0.7;
const DITCH_DEPTH: f32 = 0.6;
const DITCH_OFFSET: f32 = 0.3;
const DITCH_WIDTH: f32 = 3.0;
const DITCH_LENGTH: f32 = 60.0;
const DITCH_THRESHOLD: f32 = 0.8;
const DITCH_FADE: f32 = 0.08;
const DITCH_START_INDEX: usize = 60;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoadPoint {
    pub index: usize,
    pub t: f32,
    pub lateral: f32,
    pub distance: f32,
    pub overshoot: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Terrain {
    pub height: f32,
    pub road: Option<RoadPoint>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Character {
    pub forest: f32,
    pub open: f32,
    pub cutting: f32,
    pub ledge: f32,
    pub valley: f32,
}

fn wrap_angle(mut angle: f32) -> f32 {
    while angle > std::f32::consts::PI {
        angle -= std::f32::consts::TAU;
    }
    while angle < -std::f32::consts::PI {
        angle += std::f32::consts::TAU;
    }
    angle
}

fn severity_radius(severity: u8) -> f32 {
    [13.0, 26.0, 40.0, 60.0, 95.0, 150.0, 260.0][severity as usize]
}

impl Track {
    pub fn new(seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let turn = std::f32::consts::TAU;
        let phases = [
            rng.range(0.0, turn),
            rng.range(0.0, turn),
            rng.range(0.0, turn),
        ];
        let mut track = Self {
            seed,
            samples: Vec::new(),
            notes: Vec::new(),
            biomes: BiomeSchedule::new(seed),
            rng,
            bumps: Vec::new(),
            fords: Vec::new(),
            crossings: Vec::new(),
            ravines: Vec::new(),
            rivers: Vec::new(),
            tunnels: Vec::new(),
            structures: Vec::new(),
            dirty: Vec::new(),
            general_heading: 0.0,
            curvature: 0.0,
            phases,
            grid: HashMap::new(),
        };
        track.push_samples(30, 0.0);
        track.ensure(200);
        track
    }

    pub fn distance(index: usize) -> f32 {
        index as f32 * SAMPLE
    }

    pub fn ensure(&mut self, index: usize) {
        while self.samples.len() < index + 2 {
            self.generate_segment();
        }
    }

    fn push_samples(&mut self, count: usize, target_curvature: f32) {
        let first = self.samples.len();
        for _ in 0..count {
            let previous = self.samples.last().copied().unwrap_or_default();
            let difference = (target_curvature - self.curvature).clamp(-0.018, 0.018);
            self.curvature += difference;
            let heading = if self.samples.is_empty() {
                0.0
            } else {
                previous.heading + self.curvature * SAMPLE
            };
            let (x, z) = if self.samples.is_empty() {
                (0.0, 0.0)
            } else {
                let middle = (previous.heading + heading) * 0.5;
                (
                    previous.x + middle.sin() * SAMPLE,
                    previous.z + middle.cos() * SAMPLE,
                )
            };
            let index = self.samples.len();
            let mut sample = Sample {
                x,
                z,
                y: 0.0,
                heading,
                curvature: self.curvature,
                clearance: [MAX_TERRAIN; 2],
                fold: [MAX_TERRAIN; 2],
                deck: false,
                tunnel: false,
            };
            sample.y = self.elevation(Self::distance(index));
            self.detect_crossing(index, &mut sample);
            sample.deck =
                self.over_crossing(&sample, Self::distance(index)) || self.over_ravine(&sample);
            if sample.deck {
                self.mark_structure(index);
            }
            self.samples.push(sample);
            self.clamp_inside_of_bend(index);
            self.clamp_after_bend(index);
            self.register(index);
        }
        self.touch_samples(first.saturating_sub(1), self.samples.len(), ROAD_REACH);
    }

    fn touch(&mut self, (min_x, min_z, max_x, max_z): (f32, f32, f32, f32), reach: f32) {
        self.dirty
            .push((min_x - reach, min_z - reach, max_x + reach, max_z + reach));
    }

    fn touch_samples(&mut self, low: usize, high: usize, reach: f32) {
        let high = high.min(self.samples.len());
        if low >= high {
            return;
        }
        let bounds = self.samples[low..high].iter().fold(
            (f32::MAX, f32::MAX, f32::MIN, f32::MIN),
            |(min_x, min_z, max_x, max_z), sample| {
                (
                    min_x.min(sample.x),
                    min_z.min(sample.z),
                    max_x.max(sample.x),
                    max_z.max(sample.z),
                )
            },
        );
        self.touch(bounds, reach);
    }

    fn cell(x: f32, z: f32) -> (i32, i32) {
        (
            (x / GRID_CELL).floor() as i32,
            (z / GRID_CELL).floor() as i32,
        )
    }

    fn narrow(&mut self, index: usize, other: usize) {
        let sample = self.samples[index];
        let neighbour = self.samples[other];
        let (dx, dz) = (neighbour.x - sample.x, neighbour.z - sample.z);
        let (fx, fz) = sample.forward();
        let (rx, rz) = sample.right();
        let lateral = dx * rx + dz * rz;
        let along = dx * fx + dz * fz;
        if along.abs() > lateral.abs() * 1.5 + SAMPLE * 2.0 {
            return;
        }
        let side = usize::from(lateral > 0.0);
        let half = (lateral.abs() * 0.5).max(MIN_CLEARANCE);
        let clearance = &mut self.samples[index].clearance[side];
        *clearance = clearance.min(half);
    }

    fn clamp_inside_of_bend(&mut self, index: usize) {
        let curvature = self.samples[index].curvature;
        if curvature.abs() < 1e-4 {
            return;
        }
        let side = usize::from(curvature > 0.0);
        let limit = (0.7 / curvature.abs()).max(SHOULDER + 1.0);
        let first = index.saturating_sub(INSIDE_BEND_REACH);
        for sample in &mut self.samples[first..=index] {
            sample.fold[side] = sample.fold[side].min(limit);
        }
    }

    fn clamp_after_bend(&mut self, index: usize) {
        let first = index.saturating_sub(INSIDE_BEND_REACH);
        for earlier in first..index {
            let curvature = self.samples[earlier].curvature;
            if curvature.abs() < 1e-4 {
                continue;
            }
            let side = usize::from(curvature > 0.0);
            let limit = (0.7 / curvature.abs()).max(SHOULDER + 1.0);
            let fold = &mut self.samples[index].fold[side];
            *fold = fold.min(limit);
        }
    }

    fn register(&mut self, index: usize) {
        let sample = self.samples[index];
        let (cx, cz) = Self::cell(sample.x, sample.z);
        let mut nearby = Vec::new();
        for gx in cx - GRID_REACH..=cx + GRID_REACH {
            for gz in cz - GRID_REACH..=cz + GRID_REACH {
                if let Some(entries) = self.grid.get(&(gx, gz)) {
                    nearby.extend(
                        entries
                            .iter()
                            .map(|&other| other as usize)
                            .filter(|&other| other + LOCAL_SAMPLES < index),
                    );
                }
            }
        }
        for other in nearby {
            let neighbour = self.samples[other];
            let distance =
                ((neighbour.x - sample.x).powi(2) + (neighbour.z - sample.z).powi(2)).sqrt();
            if distance < MAX_TERRAIN * 2.0 {
                self.narrow(index, other);
                self.narrow(other, index);
            }
        }
        self.grid.entry((cx, cz)).or_default().push(index as u32);
    }

    fn detect_crossing(&mut self, index: usize, sample: &mut Sample) {
        let (cx, cz) = Self::cell(sample.x, sample.z);
        let mut best: Option<(f32, usize)> = None;
        for gx in cx - 1..=cx + 1 {
            for gz in cz - 1..=cz + 1 {
                for &entry in self.grid.get(&(gx, gz)).into_iter().flatten() {
                    let old = entry as usize;
                    if old + CROSSING_ARC > index {
                        continue;
                    }
                    let other = self.samples[old];
                    let distance = (other.x - sample.x).hypot(other.z - sample.z);
                    if distance < DECK_REACH && best.is_none_or(|(near, _)| distance < near) {
                        best = Some((distance, old));
                    }
                }
            }
        }
        let Some((distance, old)) = best else {
            return;
        };
        let old_height = self.samples[old].y;
        let existing = self
            .crossings
            .iter_mut()
            .find(|crossing| crossing.old.abs_diff(old) < 24)
            .map(|crossing| {
                crossing.old = old;
                crossing.end = Self::distance(index);
                *crossing
            });
        let old_upper = match existing {
            Some(crossing) => crossing.old_upper,
            None => old_height - sample.y >= BRIDGE_CLEARANCE,
        };
        let center = Self::distance(index) + distance * 0.5;
        if existing.is_none() {
            self.crossings.push(Crossing {
                old,
                center,
                end: center,
                old_upper,
            });
            if !old_upper {
                let note = PaceNote {
                    start: index,
                    end: index + (distance / SAMPLE) as usize,
                    kind: NoteKind::Bridge,
                    long: false,
                    tightens: false,
                    into: false,
                };
                let position = self
                    .notes
                    .partition_point(|existing| existing.end <= note.end);
                self.notes.insert(position, note);
            }
        }
        let target = if old_upper {
            old_height - BRIDGE_CLEARANCE
        } else {
            old_height + BRIDGE_CLEARANCE
        };
        let short = if old_upper {
            sample.y - target
        } else {
            target - sample.y
        };
        if short > 0.3 {
            self.bumps.push(Bump {
                center,
                height: target - self.elevation(center),
                width: BRIDGE_RAMP,
                flat: 0.0,
            });
            let reach = (BRIDGE_RAMP * 2.5 / SAMPLE) as usize;
            for earlier in index.saturating_sub(reach)..index {
                self.samples[earlier].y = self.elevation(Self::distance(earlier));
            }
            self.touch_samples(index.saturating_sub(reach), index, ROAD_REACH);
            sample.y = self.elevation(Self::distance(index));
        }
        let reach = (BRIDGE_RAMP * 1.5 / SAMPLE) as usize;
        for earlier in index.saturating_sub(reach)..index {
            let previous = self.samples[earlier];
            let deck = self.over_crossing(&previous, Self::distance(earlier))
                || self.over_ravine(&previous);
            self.samples[earlier].deck = deck;
            if deck {
                self.mark_structure(earlier);
            }
        }
    }

    fn over_crossing(&mut self, sample: &Sample, distance: f32) -> bool {
        let mut deck = false;
        for crossing in self.crossings.clone() {
            if distance < crossing.center - BRIDGE_RAMP * 1.5
                || distance > crossing.end + BRIDGE_RAMP * 1.5
            {
                continue;
            }
            let low = crossing.old.saturating_sub(12);
            let high = (crossing.old + 12).min(self.samples.len() - 1);
            for old_index in low..=high {
                let old = self.samples[old_index];
                if (old.x - sample.x).hypot(old.z - sample.z) >= DECK_REACH {
                    continue;
                }
                if sample.y - old.y > DECK_RISE {
                    deck = true;
                } else if old.y - sample.y > DECK_RISE {
                    self.samples[old_index].deck = true;
                    self.mark_structure(old_index);
                }
            }
        }
        deck
    }

    fn mark_structure(&mut self, index: usize) {
        self.touch_samples(index.saturating_sub(1), index + 2, ROAD_REACH);
        if let Some(span) = self
            .structures
            .iter_mut()
            .find(|(low, high)| index + 24 >= *low && index <= *high + 24)
        {
            span.0 = span.0.min(index);
            span.1 = span.1.max(index + 1);
        } else {
            self.structures.push((index, index + 1));
        }
    }

    pub fn structure_at(&self, x: f32, z: f32, margin: f32) -> Option<StructureHit> {
        for &(low, high) in &self.structures {
            let low = low.saturating_sub(2);
            let high = (high + 2).min(self.samples.len().saturating_sub(2));
            let near = (low..=high).step_by(6).chain([high]).any(|index| {
                let anchor = self.samples[index];
                (anchor.x - x).hypot(anchor.z - z) < SAMPLE * 6.0 + DECK_REACH
            });
            if !near {
                continue;
            }
            for index in low..high {
                let point = self.project(index, x, z);
                let half = self.half_width(point.index);
                if !self.is_structure(point.index)
                    || point.overshoot > 0.01
                    || point.lateral.abs() > half + STRUCTURE_KERB + margin
                {
                    continue;
                }
                return Some(StructureHit {
                    height: self.ground(point.index, point.t, point.lateral.clamp(-half, half)),
                    road: point,
                    tunnel: self.is_tunnel(point.index),
                });
            }
        }
        None
    }

    pub fn spans_ravine(&self, index: usize) -> bool {
        self.over_ravine(&self.sample(index))
    }

    pub fn riverbed(&self, x: f32, z: f32) -> bool {
        self.fords
            .iter()
            .chain(&self.rivers)
            .any(|ford| Self::stream_hit(ford, x, z, RIVERBED_MARGIN).is_some())
    }

    pub fn ground_at(&self, index: usize) -> crate::art::Ground {
        let (a, b, t) = self.biomes.at(Self::distance(index));
        crate::biome::BIOMES[if t > 0.5 { b } else { a }].ground
    }

    pub fn obstacle(&self, index: usize, side: f32) -> Option<Obstacle> {
        let flooded = |lateral: f32| {
            let (x, z) = self.sample(index).offset(lateral);
            self.riverbed(x, z)
        };
        self.barrier(index, side)
            .filter(|&lateral| !flooded(lateral))
            .map(|lateral| Obstacle {
                lateral,
                height: BARRIER_HEIGHT,
                thickness: BARRIER_THICKNESS,
                barrier: true,
            })
            .or_else(|| {
                self.wall(index, side)
                    .filter(|&lateral| !flooded(lateral))
                    .map(|lateral| Obstacle {
                        lateral,
                        height: WALL_HEIGHT,
                        thickness: WALL_THICKNESS,
                        barrier: false,
                    })
            })
    }

    fn barrier(&self, index: usize, side: f32) -> Option<f32> {
        if index < WALL_START || index + 3 >= self.samples.len() || self.is_structure(index) {
            return None;
        }
        let bend = (index - 2..=index + 2)
            .map(|near| self.samples[near].curvature)
            .max_by(|a, b| a.abs().total_cmp(&b.abs()))
            .unwrap_or(0.0);
        if bend.abs() < BARRIER_CURVATURE || side != -bend.signum() {
            return None;
        }
        Some(side * (self.shoulder(index) + BARRIER_OFFSET))
    }

    pub fn wall(&self, index: usize, side: f32) -> Option<f32> {
        if index < WALL_START || index + 1 >= self.samples.len() || self.is_structure(index) {
            return None;
        }
        let character = self.character(index);
        if character.ledge.abs() > 0.3 || character.cutting > 0.3 {
            return None;
        }
        let ground = self.ground_at(index);
        let chance = match ground {
            crate::art::Ground::Grass | crate::art::Ground::Wheat => WALL_CHANCE,
            crate::art::Ground::Leaves | crate::art::Ground::Earth | crate::art::Ground::Jungle => {
                WALL_CHANCE * 0.7
            }
            crate::art::Ground::Snow => return None,
        };
        let run = (index / WALL_RUN) as u64;
        let key = crate::rng::hash(self.seed ^ 0x3a11, run, (side > 0.0) as u64);
        if crate::rng::hash_float(key, 0, 0) >= chance {
            return None;
        }
        let gap = (crate::rng::hash_float(key, 1, 0) * WALL_RUN as f32 * 0.4) as usize;
        if index % WALL_RUN < gap {
            return None;
        }
        Some(side * (self.shoulder(index) + WALL_OFFSET))
    }

    pub fn is_deck(&self, index: usize) -> bool {
        let next = (index + 1).min(self.samples.len() - 1);
        self.samples[index].deck || self.samples[next].deck
    }

    pub fn is_tunnel(&self, index: usize) -> bool {
        let next = (index + 1).min(self.samples.len() - 1);
        self.samples[index].tunnel && self.samples[next].tunnel
    }

    pub fn is_structure(&self, index: usize) -> bool {
        self.is_deck(index) || self.is_tunnel(index)
    }

    pub fn surface(&self, x: f32, z: f32, y: f32) -> (f32, Option<RoadPoint>) {
        let (height, road) = self.solid_surface(x, z, y);
        match self.ice_at(x, z) {
            Some(ice) if ice > height && ice < y + DECK_STEP_UP => (ice, road),
            _ => (height, road),
        }
    }

    fn solid_surface(&self, x: f32, z: f32, y: f32) -> (f32, Option<RoadPoint>) {
        let terrain = self.terrain(x, z);
        match self.structure_at(x, z, 0.0) {
            Some(hit)
                if hit.height < y + DECK_STEP_UP
                    && (hit.height > terrain.height - DECK_BLEND
                        || terrain.height > y + DECK_STEP_UP) =>
            {
                (hit.height, Some(hit.road))
            }
            _ => (terrain.height, terrain.road),
        }
    }

    fn ravine_profile(ravine: &Ravine, x: f32, z: f32) -> Option<(f32, f32)> {
        let (dx, dz) = (x - ravine.x, z - ravine.z);
        let along = dx * ravine.line.0 + dz * ravine.line.1;
        let across = (dx * ravine.line.1 - dz * ravine.line.0).abs();
        if along.abs() > RAVINE_REACH + RAVINE_FADE || across > ravine.half_top {
            return None;
        }
        let wall =
            ((across - ravine.half_bed) / (ravine.half_top - ravine.half_bed)).clamp(0.0, 1.0);
        let wall = wall * wall * (3.0 - 2.0 * wall);
        let floor = ravine.road_y - ravine.depth;
        let target =
            floor + (ravine.road_y + 3.0 - floor) * wall.powf(0.6) + RAVINE_RIM * wall.powi(6);
        let end = ((along.abs() - RAVINE_REACH) / RAVINE_FADE).clamp(0.0, 1.0);
        let rim = 1.0 - wall.powi(4);
        Some((target, (1.0 - end * end * (3.0 - 2.0 * end)) * rim))
    }

    fn over_ravine(&self, sample: &Sample) -> bool {
        self.ravines.iter().any(|ravine| {
            Self::ravine_profile(ravine, sample.x, sample.z)
                .is_some_and(|(target, weight)| weight > 0.5 && target < sample.y - DECK_RISE)
        })
    }

    fn add_ravine(&mut self, center: usize) -> bool {
        let sample = self.samples[center];
        let (fx, fz) = sample.forward();
        let key = crate::rng::hash(self.seed ^ 0x2a71, center as u64, 0);
        let ravine = Ravine {
            x: sample.x,
            z: sample.z,
            line: sample.right(),
            road_y: sample.y,
            depth: 10.0 + crate::rng::hash_float(key, 1, 0) * 14.0,
            half_top: 13.0 + crate::rng::hash_float(key, 2, 0) * 7.0,
            half_bed: 3.0 + crate::rng::hash_float(key, 3, 0) * 2.0,
        };
        let blocked = self.samples.iter().enumerate().any(|(index, other)| {
            if index.abs_diff(center) < 30 {
                return false;
            }
            let (dx, dz) = (other.x - ravine.x, other.z - ravine.z);
            let along = dx * ravine.line.0 + dz * ravine.line.1;
            let across = dx * fx + dz * fz;
            along.abs() < RAVINE_REACH + RAVINE_FADE + 10.0 && across.abs() < ravine.half_top + 20.0
        });
        if blocked {
            return false;
        }
        self.ravines.push(ravine);
        let span = RAVINE_REACH + RAVINE_FADE + ravine.half_top;
        self.touch((ravine.x, ravine.z, ravine.x, ravine.z), span);
        let low = center.saturating_sub(12);
        let high = (center + 12).min(self.samples.len() - 1);
        for index in low..=high {
            let other = self.samples[index];
            if self.over_ravine(&other) {
                self.samples[index].deck = true;
                self.mark_structure(index);
            }
        }
        if crate::rng::hash_float(key, 4, 0) < RAVINE_RIVER_CHANCE {
            let floor = ravine.road_y - ravine.depth;
            let level = floor + 0.5;
            let path: Vec<StreamPoint> = (-20..=20)
                .map(|step| {
                    let along = step as f32 * 6.0;
                    let (x, z) = (
                        ravine.x + ravine.line.0 * along,
                        ravine.z + ravine.line.1 * along,
                    );
                    StreamPoint {
                        x,
                        z,
                        surface: floor.min(self.terrain_raw(x, z).height) + 0.4,
                        width: ravine.half_bed - 0.5,
                    }
                })
                .collect();
            let bounds = path.iter().fold(
                (f32::MAX, f32::MAX, f32::MIN, f32::MIN),
                |(min_x, min_z, max_x, max_z), point| {
                    (
                        min_x.min(point.x),
                        min_z.min(point.z),
                        max_x.max(point.x),
                        max_z.max(point.z),
                    )
                },
            );
            self.touch(bounds, STREAM_HALF_WIDTH + STREAM_BANK + 10.0);
            self.rivers.push(Ford {
                x: ravine.x,
                z: ravine.z,
                level,
                crossing: usize::MAX,
                frozen: self.ground_at(center) == crate::art::Ground::Snow,
                path,
                bounds,
            });
        }
        true
    }

    fn add_tunnel(&mut self, start: usize, end: usize) {
        for index in start..end {
            self.samples[index].tunnel = true;
            self.mark_structure(index);
        }
        self.tunnels.push(Tunnel { start, end });
        self.touch_samples(
            start,
            end,
            TUNNEL_SHOULDER + TUNNEL_SPREAD + PORTAL_FACE + 10.0,
        );
    }

    pub fn tunnel_slot(&self, x: f32, z: f32) -> bool {
        self.tunnels.iter().any(|tunnel| {
            let middle = self.samples[(tunnel.start + tunnel.end) / 2];
            let half_length = (tunnel.end - tunnel.start) as f32 * SAMPLE * 0.5;
            if (middle.x - x).hypot(middle.z - z) > half_length + SLOT_REACH + 20.0 {
                return false;
            }
            let low = tunnel.start.saturating_sub(3);
            let high = (tunnel.end + 3).min(self.samples.len() - 2);
            let Some(point) = (low..high)
                .map(|index| self.project(index, x, z))
                .min_by(|a, b| a.distance.total_cmp(&b.distance))
            else {
                return false;
            };
            let position = Self::distance(point.index) + point.t * SAMPLE;
            let (first, last) = (Self::distance(tunnel.start), Self::distance(tunnel.end - 1));
            let along = (first - position).max(position - last).max(0.0) + point.overshoot;
            along < SLOT_REACH && point.lateral.abs() < self.half_width(point.index) + SLOT_MARGIN
        })
    }

    pub fn tunnel_hill(&self, x: f32, z: f32) -> bool {
        self.tunnels
            .iter()
            .any(|tunnel| self.tunnel_cover(tunnel, x, z, 0.0).is_some())
    }

    fn tunnel_cover(&self, tunnel: &Tunnel, x: f32, z: f32, set_back: f32) -> Option<(f32, f32)> {
        let middle = self.samples[(tunnel.start + tunnel.end) / 2];
        let half_length = (tunnel.end - tunnel.start) as f32 * SAMPLE * 0.5;
        let reach = half_length + TUNNEL_SHOULDER + TUNNEL_SPREAD + 20.0;
        if (middle.x - x).hypot(middle.z - z) > reach {
            return None;
        }
        let low = tunnel.start.saturating_sub(3);
        let high = (tunnel.end + 3).min(self.samples.len() - 2);
        let point = (low..high)
            .map(|index| self.project(index, x, z))
            .min_by(|a, b| a.distance.total_cmp(&b.distance))?;
        let position = Self::distance(point.index) + point.t * SAMPLE;
        let (first, last) = (Self::distance(tunnel.start), Self::distance(tunnel.end - 1));
        let inside = (position - first).min(last - position) - point.overshoot;
        if inside < 0.0 {
            return None;
        }
        let face = if set_back > 0.0 {
            let face = ((inside - set_back) / PORTAL_RAMP).clamp(0.0, 1.0);
            face * face * (3.0 - 2.0 * face)
        } else {
            1.0
        };
        let side = ((point.lateral.abs() - TUNNEL_SHOULDER) / TUNNEL_SPREAD).clamp(0.0, 1.0);
        let side = 1.0 - side * side * (3.0 - 2.0 * side);
        let weight = face.sqrt() * side;
        if weight <= 0.0 {
            return None;
        }
        Some((
            self.ground(point.index, point.t, 0.0) + TUNNEL_COVER,
            weight,
        ))
    }

    fn preview_clear(&self, count: usize, target_curvature: f32) -> bool {
        self.preview_gap(count, target_curvature, usize::MAX) >= PREVIEW_CLEARANCE
    }

    fn preview_gap(&self, count: usize, target_curvature: f32, arc_limit: usize) -> f32 {
        let mut gap = f32::MAX;
        let Some(last) = self.samples.last().copied() else {
            return gap;
        };
        let newest = self.samples.len();
        let (mut x, mut z, mut heading, mut curvature) =
            (last.x, last.z, last.heading, self.curvature);
        let mut preview: Vec<(f32, f32)> = Vec::with_capacity(count + PREVIEW_MARGIN);
        let scaled = |distance: f32, arc_samples: usize| {
            let required = arc_samples as f32 * SAMPLE * LOOP_RATIO;
            if required < PREVIEW_CLEARANCE {
                distance * PREVIEW_CLEARANCE / required.max(1e-3)
            } else {
                distance
            }
        };
        for step in 0..count + PREVIEW_MARGIN {
            curvature += (target_curvature - curvature).clamp(-0.018, 0.018);
            let next = heading + curvature * SAMPLE;
            let middle = (heading + next) * 0.5;
            x += middle.sin() * SAMPLE;
            z += middle.cos() * SAMPLE;
            heading = next;
            let (cx, cz) = Self::cell(x, z);
            for gx in cx - 1..=cx + 1 {
                for gz in cz - 1..=cz + 1 {
                    if let Some(entries) = self.grid.get(&(gx, gz)) {
                        for &entry in entries {
                            let index = entry as usize;
                            if newest + step - index > arc_limit {
                                continue;
                            }
                            let other = self.samples[index];
                            let distance = (other.x - x).hypot(other.z - z);
                            gap = gap.min(scaled(distance, newest + step - index));
                        }
                    }
                }
            }
            for (earlier, &(px, pz)) in preview.iter().enumerate() {
                gap = gap.min(scaled((px - x).hypot(pz - z), step - earlier));
            }
            preview.push((x, z));
        }
        gap
    }

    fn elevation(&self, distance: f32) -> f32 {
        let hilliness = self.biomes.scalar(distance, |biome| biome.hilliness);
        let [a, b, c] = self.phases;
        let hills = (distance / 190.0 + a).sin() * 9.0
            + (distance / 71.0 + b).sin() * 3.5
            + (distance / 33.0 + c).sin() * 0.8;
        let start_ramp = (distance / 200.0).min(1.0);
        let bumps: f32 = self
            .bumps
            .iter()
            .map(|bump| {
                let d = ((distance - bump.center).abs() - bump.flat).max(0.0) / bump.width;
                bump.height * (-d * d).exp()
            })
            .sum();
        hills * hilliness * start_ramp + bumps
    }

    fn generate_turn(&mut self, start: usize, heading: f32, deviation: f32) {
        let severity = self
            .rng
            .pick_weighted(&[10.0, 8.0, 13.0, 20.0, 22.0, 20.0, 15.0]) as u8;
        let direction = if deviation.abs() > 1.1 {
            if deviation > 0.0 {
                Direction::Left
            } else {
                Direction::Right
            }
        } else if self.rng.chance(0.5) {
            Direction::Left
        } else {
            Direction::Right
        };
        let angle = match severity {
            0 => self.rng.range(2.6, 3.0),
            1..=2 => self.rng.range(1.0, 1.9),
            3..=4 => self.rng.range(0.6, 1.5),
            _ => self.rng.range(0.3, 0.9),
        };
        let opposite = match direction {
            Direction::Left => Direction::Right,
            Direction::Right => Direction::Left,
        };
        let candidates = [
            (direction, angle, severity),
            (opposite, angle, severity),
            (direction, angle * 0.5, severity),
            (opposite, angle * 0.5, severity),
            (direction, 2.9, 0),
            (opposite, 2.9, 0),
        ];
        let assess = |&(candidate, candidate_angle, candidate_severity): &(Direction, f32, u8)| {
            let candidate_radius = severity_radius(candidate_severity);
            let length = ((candidate_angle * candidate_radius / SAMPLE).round() as usize).max(3);
            let final_heading = heading + candidate.sign() * candidate_angle;
            let gap = self.preview_gap(length, candidate.sign() / candidate_radius, usize::MAX);
            let aligned = (final_heading - self.general_heading).abs() < MAX_DEVIATION;
            (gap, aligned)
        };
        let loop_gap =
            |&(candidate, candidate_angle, candidate_severity): &(Direction, f32, u8)| {
                let candidate_radius = severity_radius(candidate_severity);
                let length =
                    ((candidate_angle * candidate_radius / SAMPLE).round() as usize).max(3);
                self.preview_gap(length, candidate.sign() / candidate_radius, CROSSING_ARC)
                    .min(PREVIEW_CLEARANCE)
            };
        let chosen = candidates
            .iter()
            .find(|candidate| {
                let (gap, aligned) = assess(candidate);
                aligned && gap >= PREVIEW_CLEARANCE
            })
            .or_else(|| {
                candidates.iter().max_by(|a, b| {
                    (loop_gap(a), assess(a).0)
                        .partial_cmp(&(loop_gap(b), assess(b).0))
                        .unwrap()
                })
            })
            .copied()
            .unwrap();
        let (direction, angle, severity) = chosen;
        let radius = severity_radius(severity);
        let length = ((angle * radius / SAMPLE).round() as usize).max(3);

        if let Some(previous) = self.notes.last_mut() {
            if previous.end + 2 >= start {
                previous.into = true;
                if let NoteKind::Turn {
                    direction: previous_direction,
                    severity: previous_severity,
                } = previous.kind
                {
                    if previous_direction == direction && severity < previous_severity {
                        previous.tightens = true;
                    }
                }
            }
        }
        self.notes.push(PaceNote {
            start,
            end: start + length,
            kind: NoteKind::Turn {
                direction,
                severity,
            },
            long: angle > 1.6 && severity > 0,
            tightens: false,
            into: false,
        });
        self.push_samples(length, direction.sign() / radius);
        if self.rng.chance(0.6) {
            let settle = 2 + self.rng.below(5) as usize;
            self.push_samples(settle, 0.0);
        }
    }

    fn generate_segment(&mut self) {
        let start = self.samples.len();
        let heading = self.samples.last().map(|s| s.heading).unwrap_or(0.0);
        self.general_heading = (self.general_heading + self.rng.range(-0.35, 0.35))
            .clamp(-GENERAL_DRIFT, GENERAL_DRIFT);
        let deviation = heading - self.general_heading;

        if self.rng.chance(0.3) {
            let mut length = 8 + self.rng.below(28) as usize;
            while length > 6 && !self.preview_clear(length, 0.0) {
                length /= 2;
            }
            if !self.preview_clear(length, 0.0) {
                return self.generate_turn(start, heading, deviation);
            }
            let bump = length >= 14 && self.rng.chance(0.45);
            if bump {
                let center = Self::distance(start + length / 2);
                let width = self.rng.range(9.0, 15.0);
                let height = self.rng.range(1.2, 3.8);
                self.bumps.push(Bump {
                    center,
                    height,
                    width,
                    flat: 0.0,
                });
                let jump = height / (width * width) > 0.022;
                self.notes.push(PaceNote {
                    start: start + length / 2 - (width / SAMPLE) as usize,
                    end: start + length / 2,
                    kind: if jump {
                        NoteKind::Jump
                    } else {
                        NoteKind::Crest
                    },
                    long: false,
                    tightens: false,
                    into: false,
                });
            }
            let center = (start + 4..start + length.saturating_sub(4).max(5))
                .min_by(|&a, &b| {
                    self.elevation(Self::distance(a))
                        .total_cmp(&self.elevation(Self::distance(b)))
                })
                .unwrap_or(start + length / 2);
            let ford = !bump
                && length >= 12
                && self
                    .biomes
                    .mix(Self::distance(center), |biome| biome.weather)
                    .0
                    != crate::biome::Weather::Dust
                && crate::rng::hash_float(self.seed ^ 0xf04d, start as u64, 0) < 0.35;
            if ford {
                self.bumps.push(Bump {
                    center: Self::distance(center),
                    height: -FORD_DEPTH,
                    width: FORD_WIDTH,
                    flat: FORD_FLAT,
                });
                let frozen = self.ground_at(center) == crate::art::Ground::Snow;
                self.notes.push(PaceNote {
                    start: center.saturating_sub(4),
                    end: center,
                    kind: if frozen {
                        NoteKind::Ice
                    } else {
                        NoteKind::Ford
                    },
                    long: false,
                    tightens: false,
                    into: false,
                });
            }
            let roll = crate::rng::hash_float(self.seed ^ 0x7a1e, start as u64, 1);
            let hilliness = self
                .biomes
                .scalar(Self::distance(center), |biome| biome.hilliness);
            let ravine = !bump && !ford && length >= 16 && start > 60 && roll < RAVINE_CHANCE;
            let tunnel = !bump
                && !ford
                && !ravine
                && length >= 16
                && start > 60
                && hilliness >= 1.0
                && roll > 1.0 - TUNNEL_CHANCE;
            let middle = start + length / 2;
            if ravine {
                self.notes.push(PaceNote {
                    start: middle.saturating_sub(8),
                    end: middle.saturating_sub(3),
                    kind: NoteKind::Bridge,
                    long: false,
                    tightens: false,
                    into: false,
                });
            }
            if tunnel {
                self.notes.push(PaceNote {
                    start: start + 2,
                    end: start + 3,
                    kind: NoteKind::Tunnel,
                    long: length >= 28,
                    tightens: false,
                    into: false,
                });
            }
            self.push_samples(length, 0.0);
            if ford {
                let sample = self.samples[center];
                let mut ford = self.build_stream(sample, sample.y + FORD_WATER, self.fords.len());
                ford.frozen = self.ground_at(center) == crate::art::Ground::Snow;
                self.touch(ford.bounds, STREAM_HALF_WIDTH + STREAM_BANK + 10.0);
                self.fords.push(ford);
            }
            if ravine && !self.add_ravine(middle) {
                let bridge_end = middle.saturating_sub(3);
                if let Some(index) = self
                    .notes
                    .iter()
                    .rposition(|note| note.kind == NoteKind::Bridge && note.end == bridge_end)
                {
                    self.notes.remove(index);
                }
            }
            if tunnel {
                self.add_tunnel(start + 3, start + length - 3);
            }
            return;
        }

        self.generate_turn(start, heading, deviation);
    }

    pub fn sample(&self, index: usize) -> Sample {
        self.samples[index.min(self.samples.len() - 1)]
    }

    fn project(&self, index: usize, x: f32, z: f32) -> RoadPoint {
        let along = |index: usize| {
            let sample = self.samples[index];
            let (fx, fz) = sample.forward();
            ((x - sample.x) * fx + (z - sample.z) * fz) / SAMPLE
        };
        let mut index = index.min(self.samples.len() - 2);
        if along(index) < 0.0 && index > 0 {
            index -= 1;
        }
        let sample = self.samples[index];
        let raw = along(index);
        let t = raw.clamp(0.0, 1.0);
        let (rx, rz) = sample.right();
        let lateral = (x - sample.x) * rx + (z - sample.z) * rz;
        let overshoot = (raw - t).abs() * SAMPLE;
        RoadPoint {
            index,
            t,
            lateral,
            distance: (lateral * lateral + overshoot * overshoot).sqrt(),
            overshoot,
        }
    }

    fn profile_height(&self, road: &RoadPoint, x: f32, z: f32) -> f32 {
        let near = self.ground(road.index, road.t, road.lateral);
        let far = self.far_height(x, z);
        let distance = road.distance;
        let weight = ((distance - FAR_BLEND_START) / FAR_BLEND_LENGTH).clamp(0.0, 1.0);
        let weight = weight * weight * (3.0 - 2.0 * weight);
        near + (far - near) * weight
    }

    pub fn far_height(&self, x: f32, z: f32) -> f32 {
        let noise =
            |scale: f32, salt: u64| value_noise_2d(self.seed ^ salt, x / scale, z / scale) - 0.5;
        noise(420.0, 0xa1) * 44.0 + noise(160.0, 0xb2) * 16.0 + noise(55.0, 0xc3) * 4.0 + 4.0
    }

    pub fn terrain(&self, x: f32, z: f32) -> Terrain {
        self.terrain_with(x, z, 0.0)
    }

    pub fn drawn_terrain(&self, x: f32, z: f32) -> Terrain {
        self.terrain_with(x, z, PORTAL_SET)
    }

    fn terrain_with(&self, x: f32, z: f32, portal_set_back: f32) -> Terrain {
        let mut terrain = self.terrain_raw(x, z);
        let road_guard = terrain.road.map_or(1.0, |road| {
            let half = self.half_width(road.index);
            ((road.distance - half) / (self.shoulder(road.index) - half)).clamp(0.0, 1.0)
        });
        for ravine in &self.ravines {
            if let Some((target, weight)) = Self::ravine_profile(ravine, x, z) {
                let weight = weight * road_guard;
                terrain.height += (terrain.height.min(target) - terrain.height) * weight;
            }
        }
        for tunnel in &self.tunnels {
            if let Some((cover, weight)) = self.tunnel_cover(tunnel, x, z, portal_set_back) {
                let raised = terrain.height + (cover - terrain.height) * weight;
                terrain.height = terrain.height.max(raised);
            }
        }
        for ford in self.fords.iter().chain(&self.rivers) {
            let Some(hit) = Self::stream_hit(ford, x, z, STREAM_BANK) else {
                continue;
            };
            let bed = hit.surface - STREAM_BED;
            if terrain.height > bed {
                let off_road = terrain.road.map_or(1.0, |road| {
                    let half = self.half_width(road.index);
                    ((road.distance - half) / 3.0).clamp(0.0, 1.0)
                });
                let wall = ((hit.distance - hit.width - 2.0) / (STREAM_BANK - 2.0)).clamp(0.0, 1.0);
                let weight = (1.0 - wall * wall * (3.0 - 2.0 * wall)) * off_road;
                terrain.height += (bed - terrain.height) * weight;
            }
        }
        if let Some(road) = terrain.road {
            let shoulder = self.shoulder(road.index);
            if road.distance < shoulder + NEAR_ROAD && !self.tunnel_hill(x, z) {
                let road_y = self.ground(road.index, road.t, 0.0);
                let out = (road.distance - shoulder).max(0.0);
                let ramp = road_y + 0.3 + out * NEAR_ROAD_RAMP + out * out * NEAR_ROAD_CURVE;
                terrain.height = terrain.height.min(ramp);
            }
        }
        terrain
    }

    fn stream_hit(ford: &Ford, x: f32, z: f32, margin: f32) -> Option<StreamHit> {
        let reach = STREAM_HALF_WIDTH + margin;
        let (min_x, min_z, max_x, max_z) = ford.bounds;
        if x < min_x - reach || x > max_x + reach || z < min_z - reach || z > max_z + reach {
            return None;
        }
        let mut best: Option<StreamHit> = None;
        for pair in ford.path.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let (sx, sz) = (b.x - a.x, b.z - a.z);
            let length = sx * sx + sz * sz;
            let t = (((x - a.x) * sx + (z - a.z) * sz) / length.max(1e-6)).clamp(0.0, 1.0);
            let distance = (x - a.x - sx * t).hypot(z - a.z - sz * t);
            if best.as_ref().is_none_or(|hit| distance < hit.distance) {
                best = Some(StreamHit {
                    distance,
                    surface: a.surface + (b.surface - a.surface) * t,
                    width: a.width + (b.width - a.width) * t,
                });
            }
        }
        best.filter(|hit| hit.distance < hit.width + margin)
    }

    fn build_stream(&self, sample: Sample, level: f32, number: usize) -> Ford {
        let (rx, rz) = sample.right();
        let raw = |x: f32, z: f32| self.terrain_raw(x, z).height;
        let probe = |sign: f32| raw(sample.x + rx * 30.0 * sign, sample.z + rz * 30.0 * sign);
        let up_sign = if probe(1.0) > probe(-1.0) { 1.0 } else { -1.0 };
        let walk = |sign: f32, uphill: bool| {
            let initial = (rx * sign).atan2(rz * sign);
            let mut angle = initial;
            let (mut x, mut z) = (sample.x, sample.z);
            let mut previous = level;
            let mut points = Vec::new();
            for step in 0..STREAM_STEPS {
                let gx = raw(x + 2.0, z) - raw(x - 2.0, z);
                let gz = raw(x, z + 2.0) - raw(x, z - 2.0);
                let pull = if uphill { 1.0 } else { -1.0 };
                let slope = gx.hypot(gz) / 4.0;
                if slope > 0.005 {
                    let target = (gx * pull).atan2(gz * pull);
                    let turn = wrap_angle(target - angle).clamp(-0.3, 0.3);
                    angle += turn * (slope * 8.0).min(1.0);
                }
                let wander = crate::rng::hash_float(self.seed ^ 0x57e4, number as u64, step as u64);
                angle += (wander - 0.5) * 0.4;
                if step < 4 {
                    angle = initial + wrap_angle(angle - initial).clamp(-0.4, 0.4);
                }
                x += angle.sin() * STREAM_STEP;
                z += angle.cos() * STREAM_STEP;
                let terrain = self.terrain_raw(x, z);
                if step > 2
                    && terrain
                        .road
                        .is_some_and(|road| road.distance < self.shoulder(road.index) + 4.0)
                {
                    break;
                }
                let ground = terrain.height + 0.2;
                let surface = if step < 2 {
                    level
                } else if uphill {
                    if ground < previous - 0.5 {
                        break;
                    }
                    previous.max(ground)
                } else {
                    if ground > previous + 3.0 {
                        break;
                    }
                    previous.min(ground)
                };
                previous = surface;
                let progress = step as f32 / STREAM_STEPS as f32;
                let width = if uphill {
                    STREAM_HALF_WIDTH * (1.0 - 0.75 * progress)
                } else {
                    STREAM_HALF_WIDTH * (1.0 + 0.3 * progress)
                };
                points.push(StreamPoint {
                    x,
                    z,
                    surface,
                    width,
                });
            }
            points
        };
        let mut path = walk(up_sign, true);
        path.reverse();
        let crossing = path.len();
        path.push(StreamPoint {
            x: sample.x,
            z: sample.z,
            surface: level,
            width: STREAM_HALF_WIDTH,
        });
        path.extend(walk(-up_sign, false));
        let bounds = path.iter().fold(
            (f32::MAX, f32::MAX, f32::MIN, f32::MIN),
            |(min_x, min_z, max_x, max_z), point| {
                (
                    min_x.min(point.x),
                    min_z.min(point.z),
                    max_x.max(point.x),
                    max_z.max(point.z),
                )
            },
        );
        Ford {
            x: sample.x,
            z: sample.z,
            level,
            crossing,
            frozen: false,
            path,
            bounds,
        }
    }

    pub fn nearest_water(&self, x: f32, z: f32, reach: f32) -> Option<(f32, f32, f32)> {
        let span = STREAM_STEP * STREAM_STEPS as f32 + reach;
        self.fords
            .iter()
            .chain(&self.rivers)
            .filter(|ford| !ford.frozen && (ford.x - x).hypot(ford.z - z) < span)
            .flat_map(|ford| ford.path.iter())
            .map(|point| ((point.x - x).hypot(point.z - z), point.x, point.z))
            .filter(|&(distance, _, _)| distance < reach)
            .min_by(|a, b| a.0.total_cmp(&b.0))
    }

    pub fn water_at(&self, x: f32, z: f32) -> Option<f32> {
        self.fords
            .iter()
            .chain(&self.rivers)
            .filter(|ford| !ford.frozen)
            .find_map(|ford| Self::stream_hit(ford, x, z, 0.0))
            .map(|hit| self.settled_level(hit.surface, x, z))
    }

    pub fn ice_at(&self, x: f32, z: f32) -> Option<f32> {
        self.fords
            .iter()
            .chain(&self.rivers)
            .filter(|ford| ford.frozen)
            .find_map(|ford| Self::stream_hit(ford, x, z, 0.0))
            .map(|hit| self.settled_level(hit.surface, x, z))
    }

    pub fn settled_level(&self, surface: f32, x: f32, z: f32) -> f32 {
        surface.min(self.terrain(x, z).height + STREAM_BED)
    }

    pub fn puddle(&self, index: usize) -> Option<Puddle> {
        if index < 40 || index + 1 >= self.samples.len() {
            return None;
        }
        let key = crate::rng::hash(self.seed ^ 0x9dd1e, index as u64, 0);
        let roll = crate::rng::hash_float(key, 1, 0);
        let weather = self
            .biomes
            .mix(Self::distance(index), |biome| biome.weather)
            .0;
        let chance = match weather {
            crate::biome::Weather::Rain => 0.09,
            crate::biome::Weather::Clear => 0.015,
            crate::biome::Weather::Snow => ICE_PATCH_CHANCE,
            _ => 0.0,
        };
        if roll >= chance {
            return None;
        }
        let sample = self.samples[index];
        let half = self.half_width(index);
        let radius = 0.7 + crate::rng::hash_float(key, 2, 0) * 1.1;
        let lateral = (crate::rng::hash_float(key, 3, 0) * 2.0 - 1.0) * (half - radius).max(0.0);
        let (x, z) = sample.offset(lateral);
        Some(Puddle {
            x,
            z,
            y: self.ground(index, 0.0, lateral),
            heading: sample.heading,
            radius,
            frozen: weather == crate::biome::Weather::Snow,
        })
    }

    pub fn puddle_at(&self, index: usize, x: f32, z: f32) -> Option<Puddle> {
        (index.saturating_sub(1)..=index + 1).find_map(|index| {
            self.puddle(index).filter(|puddle| {
                let (dx, dz) = (x - puddle.x, z - puddle.z);
                let (sin, cos) = puddle.heading.sin_cos();
                let along = (dx * sin + dz * cos) / 1.4;
                let across = dx * cos - dz * sin;
                along * along + across * across < puddle.radius * puddle.radius
            })
        })
    }

    fn terrain_raw(&self, x: f32, z: f32) -> Terrain {
        let (cx, cz) = Self::cell(x, z);
        let mut points: Vec<RoadPoint> = Vec::new();
        for gx in cx - GRID_REACH..=cx + GRID_REACH {
            for gz in cz - GRID_REACH..=cz + GRID_REACH {
                if let Some(entries) = self.grid.get(&(gx, gz)) {
                    for &entry in entries {
                        let index = entry as usize;
                        if index + 1 >= self.samples.len() {
                            continue;
                        }
                        let point = self.project(index, x, z);
                        if self.is_structure(point.index) {
                            continue;
                        }
                        if !points.iter().any(|existing| existing.index == point.index) {
                            points.push(point);
                        }
                    }
                }
            }
        }
        let Some(nearest) = points
            .iter()
            .copied()
            .min_by(|a, b| a.distance.total_cmp(&b.distance))
        else {
            return Terrain {
                height: self.far_height(x, z),
                road: None,
            };
        };
        let mut total = 0.0;
        let mut weighted = 0.0;
        let least_overshoot = points
            .iter()
            .filter(|point| point.distance - nearest.distance <= BLEND_RANGE)
            .map(|point| point.overshoot)
            .fold(f32::MAX, f32::min);
        for point in &points {
            let excess = point.distance - nearest.distance;
            if excess > BLEND_RANGE {
                continue;
            }
            let overshoot = point.overshoot - least_overshoot;
            let weight = (-(excess / BLEND_SOFTNESS).powi(2)).exp()
                * (-(overshoot / OVERSHOOT_SOFTNESS).powi(2)).exp();
            if weight < 1e-6 {
                continue;
            }
            weighted += self.profile_height(point, x, z) * weight;
            total += weight;
        }
        let far = self.far_height(x, z);
        let blended = if total > 0.0 { weighted / total } else { far };
        let half = self.half_width(nearest.index);
        let own = ((nearest.lateral.abs() - half) / (self.shoulder(nearest.index) - half))
            .clamp(0.0, 1.0);
        let own = own * own * (3.0 - 2.0 * own);
        let on_road = self.profile_height(&nearest, x, z);
        Terrain {
            height: on_road + (blended - on_road) * own,
            road: Some(nearest),
        }
    }

    pub fn locate(&self, x: f32, z: f32, hint: usize) -> (usize, f32, f32) {
        let low = hint.saturating_sub(6);
        let high = (hint + 16).min(self.samples.len() - 2);
        let mut best = (hint.min(high), f32::MAX);
        for (index, sample) in self.samples.iter().enumerate().take(high + 1).skip(low) {
            let distance = (sample.x - x).powi(2) + (sample.z - z).powi(2);
            if distance < best.1 {
                best = (index, distance);
            }
        }
        let mut index = best.0;
        let along = |index: usize| {
            let sample = self.samples[index];
            let (fx, fz) = sample.forward();
            ((x - sample.x) * fx + (z - sample.z) * fz) / SAMPLE
        };
        if along(index) < 0.0 && index > 0 {
            index -= 1;
        }
        let sample = self.samples[index];
        let t = along(index).clamp(0.0, 1.0);
        let (rx, rz) = sample.right();
        let lateral = (x - sample.x) * rx + (z - sample.z) * rz;
        (index, t, lateral)
    }

    pub fn character(&self, index: usize) -> Character {
        let distance = Self::distance(index);
        let section = (distance / SECTION_LENGTH).floor();
        let into = distance - section * SECTION_LENGTH;
        let fade = (into / SECTION_FADE)
            .min((SECTION_LENGTH - into) / SECTION_FADE)
            .clamp(0.0, 1.0);
        let weight = fade * fade * (3.0 - 2.0 * fade);
        let key = crate::rng::hash(self.seed ^ 0xc4a7, section as u64, 0);
        let pick = crate::rng::hash_float(key, 1, 0);
        let side = if crate::rng::hash_float(key, 2, 0) < 0.5 {
            -1.0
        } else {
            1.0
        };
        let mut character = Character::default();
        if index < 40 {
            return character;
        }
        match pick {
            p if p < 0.30 => character.forest = weight,
            p if p < 0.45 => character.cutting = weight,
            p if p < 0.57 => character.ledge = weight * side,
            p if p < 0.70 => character.valley = weight * side,
            p if p < 0.85 => character.open = weight,
            _ => {}
        }
        character
    }

    pub fn half_width(&self, index: usize) -> f32 {
        let character = self.character(index);
        ROAD_HALF - 1.3 * character.ledge.abs() - 0.7 * character.cutting
    }

    pub fn shoulder(&self, index: usize) -> f32 {
        self.half_width(index) + SHOULDER - ROAD_HALF
    }

    pub fn boundary(&self, index: usize, lateral: f32) -> f32 {
        let character = self.character(index);
        let half = self.half_width(index);
        let wall_side = character.ledge.signum() == lateral.signum();
        let ledge_limit = if wall_side { half + 3.0 } else { half + 2.2 };
        let walled = BOUNDARY + (half + 3.0 - BOUNDARY) * character.cutting;
        walled + (ledge_limit - walled) * character.ledge.abs()
    }

    pub fn bank(&self, index: usize, side: f32) -> f32 {
        let distance = Self::distance(index);
        let height = self.biomes.scalar(distance, |biome| biome.bank_height);
        let side_key = if side < 0.0 { 1 } else { 2 };
        let character = value_noise(self.seed ^ side_key, distance / 90.0) * 2.0 - 0.8;
        height * character
    }

    pub fn side_height(&self, index: usize, lateral: f32) -> f32 {
        let distance = lateral.abs();
        let half = self.half_width(index);
        let shoulder = self.shoulder(index);
        if distance <= shoulder {
            return -((distance - half).max(0.0) * 0.12);
        }
        let beyond = distance - shoulder;
        let ditch = self.ditch(index, lateral) * Self::ditch_profile(beyond);
        let smooth = |x: f32| {
            let x = x.clamp(0.0, 1.0);
            x * x * (3.0 - 2.0 * x)
        };
        let bank = self.bank(index, lateral);
        let t = (beyond / 25.0).min(1.6);
        let side_seed = self.seed ^ if lateral < 0.0 { 0x51de } else { 0x7a11 };
        let roughness = value_noise(side_seed, index as f32 * 0.35 + distance * 0.11) - 0.5;
        let character = self.character(index);
        let open = 1.0 - character.open * 0.7;
        let mut height = -0.24 + bank * t * open + roughness * t.min(1.0);
        height += character.cutting * (5.5 * smooth(beyond / 4.0) + beyond.min(12.0) * 0.15);
        height -= ditch;
        if character.ledge != 0.0 {
            let amount = character.ledge.abs();
            if character.ledge.signum() == lateral.signum() {
                height += amount * (8.0 * smooth(beyond / 3.0) + beyond.min(20.0) * 0.3);
            } else {
                height -= amount * (14.0 * smooth(beyond / 7.0) + beyond.min(40.0) * 0.25);
            }
        }
        if character.valley != 0.0 {
            let amount = character.valley.abs();
            if character.valley.signum() == lateral.signum() {
                height -= amount * 18.0 * smooth(beyond / 45.0);
            } else {
                height += amount * 5.0 * smooth(beyond / 25.0);
            }
        }
        height
    }

    pub fn ditch(&self, index: usize, lateral: f32) -> f32 {
        if index < DITCH_START_INDEX
            || self.ground_at(index) == crate::art::Ground::Earth
            || self.character(index).cutting > 0.0
        {
            return 0.0;
        }
        let side_key = if lateral < 0.0 { 0xd17c } else { 0xd17d };
        let noise = value_noise(self.seed ^ side_key, Self::distance(index) / DITCH_LENGTH);
        let amount = ((noise - DITCH_THRESHOLD) / DITCH_FADE).clamp(0.0, 1.0);
        DITCH_DEPTH * amount * amount * (3.0 - 2.0 * amount)
    }

    fn ditch_profile(beyond: f32) -> f32 {
        let across = (beyond - DITCH_OFFSET) / DITCH_WIDTH;
        if (0.0..=1.0).contains(&across) {
            (across * std::f32::consts::PI).sin()
        } else {
            0.0
        }
    }

    pub fn drawn_lateral(&self, index: usize, offset: f32) -> f32 {
        let sample = self.sample(index);
        if offset.abs() > self.shoulder(index) {
            offset.signum() * offset.abs().min(sample.fold(offset))
        } else {
            offset
        }
    }

    pub fn ground(&self, index: usize, t: f32, lateral: f32) -> f32 {
        let road = self.elevation(Self::distance(index) + t * SAMPLE);
        let side_a = self.side_height(index, lateral);
        let side_b = self.side_height(index + 1, lateral);
        let smooth = t * t * (3.0 - 2.0 * t);
        road + side_a + (side_b - side_a) * smooth
    }

    pub fn upcoming_notes(&self, index: usize, count: usize) -> impl Iterator<Item = &PaceNote> {
        let first = self.notes.partition_point(|note| note.end <= index);
        self.notes[first..].iter().take(count)
    }

    pub fn is_checkpoint(index: usize) -> bool {
        index > 0 && index.is_multiple_of(CHECKPOINT_SPACING)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_for_seed() {
        let mut a = Track::new(42);
        let mut b = Track::new(42);
        a.ensure(2000);
        b.ensure(2000);
        assert_eq!(a.samples.len(), b.samples.len());
        assert_eq!(a.samples[1500].x, b.samples[1500].x);
        assert_eq!(a.notes, b.notes);
    }

    #[test]
    fn fords_flood_the_road_and_carve_a_channel() {
        let mut track = Track::new(11);
        track.ensure(6000);
        assert!(track.fords.len() > 3);
        for ford in &track.fords {
            let road = track.terrain(ford.x, ford.z).road.unwrap();
            let road_y = track.ground(road.index, road.t, 0.0);
            assert!(ford.level > road_y + 0.2);
            let surface = if ford.frozen {
                track.ice_at(ford.x, ford.z)
            } else {
                track.water_at(ford.x, ford.z)
            };
            assert_eq!(surface, Some(ford.level));
            assert!(ford.path.len() > 4);
            for point in ford.path.iter().skip(ford.crossing + 3) {
                let bed = track.terrain(point.x, point.z).height;
                assert!(bed < point.surface, "{bed} {}", point.surface);
            }
            for pair in ford.path[ford.crossing..].windows(2) {
                assert!(pair[1].surface <= pair[0].surface);
            }
            for pair in ford.path[..=ford.crossing].windows(2) {
                assert!(pair[1].surface <= pair[0].surface);
            }
            assert!(track
                .notes
                .iter()
                .any(|note| matches!(note.kind, NoteKind::Ford | NoteKind::Ice)
                    && note.end <= road.index + 1));
        }
    }

    #[test]
    fn puddles_sit_on_the_road() {
        let mut track = Track::new(5);
        track.ensure(4000);
        let puddles: Vec<(usize, Puddle)> = (0..3900)
            .filter_map(|index| track.puddle(index).map(|puddle| (index, puddle)))
            .collect();
        assert!(puddles.len() > 10);
        for (index, puddle) in puddles {
            let (_, _, lateral) = track.locate(puddle.x, puddle.z, index);
            assert!(lateral.abs() <= track.half_width(index) + 0.01);
            assert!(track.puddle_at(index, puddle.x, puddle.z).is_some());
        }
    }

    #[test]
    fn samples_are_evenly_spaced() {
        let mut track = Track::new(7);
        track.ensure(3000);
        for pair in track.samples.windows(2) {
            let distance =
                ((pair[1].x - pair[0].x).powi(2) + (pair[1].z - pair[0].z).powi(2)).sqrt();
            assert!((distance - SAMPLE).abs() < 0.01, "{distance}");
        }
    }

    #[test]
    fn notes_are_ordered_and_called() {
        let mut track = Track::new(3);
        track.ensure(3000);
        assert!(track.notes.len() > 50);
        assert!(track
            .notes
            .windows(2)
            .all(|pair| pair[0].start <= pair[1].start));
        assert!(track
            .notes
            .iter()
            .any(|note| note.call().starts_with("HAIRPIN")));
        assert!(track
            .notes
            .iter()
            .any(|note| note.modifiers().contains("TIGHTENS")));
    }

    #[test]
    fn terrain_sinks_before_reaching_nearby_road() {
        let mut track = Track::new(5);
        track.ensure(4000);
        let narrowed = track
            .samples
            .iter()
            .filter(|s| s.clearance.iter().any(|&c| c < MAX_TERRAIN))
            .count();
        assert!(narrowed > 0);
        for (index, sample) in track.samples.iter().enumerate() {
            for (other_index, other) in track.samples.iter().enumerate() {
                if other_index.abs_diff(index) <= LOCAL_SAMPLES {
                    continue;
                }
                let (dx, dz) = (other.x - sample.x, other.z - sample.z);
                let (rx, rz) = sample.right();
                let (fx, fz) = sample.forward();
                let lateral = dx * rx + dz * rz;
                if (dx * fx + dz * fz).abs() < 2.0
                    && lateral.abs() > MIN_CLEARANCE * 2.0
                    && lateral.abs() < MAX_TERRAIN
                {
                    assert!(
                        sample.clearance(lateral) <= lateral.abs() * 0.5 + 0.01
                            && sample.sink(lateral) < -1.0,
                        "sample {index} side reaches road {other_index}"
                    );
                }
            }
        }
    }

    #[test]
    fn terrain_never_folds_inside_a_bend() {
        let mut track = Track::new(3);
        track.ensure(3000);
        for (index, sample) in track
            .samples
            .iter()
            .enumerate()
            .take(track.samples.len() - 1)
        {
            if sample.curvature.abs() < 1e-3 {
                continue;
            }
            let inside = if sample.curvature > 0.0 { 1.0 } else { -1.0 };
            let reach = sample.fold(inside).min(MAX_TERRAIN);
            assert!(
                reach * sample.curvature.abs() < 0.75,
                "sample {index} inside reach {reach} radius {}",
                1.0 / sample.curvature.abs()
            );
        }
    }

    #[test]
    fn stage_characters_vary_and_stay_drivable() {
        let mut track = Track::new(21);
        track.ensure(5000);
        let characters: Vec<Character> = (0..5000).map(|index| track.character(index)).collect();
        assert!(characters.iter().any(|c| c.ledge.abs() > 0.99));
        assert!(characters.iter().any(|c| c.cutting > 0.99));
        assert!(characters.iter().any(|c| c.forest > 0.99));
        assert!(characters.iter().any(|c| c.valley.abs() > 0.99));
        for index in 0..5000 {
            let half = track.half_width(index);
            assert!((3.0..=ROAD_HALF).contains(&half));
            for side in [-1.0f32, 1.0] {
                let boundary = track.boundary(index, side);
                assert!(
                    boundary >= half + 2.0 && boundary <= BOUNDARY + 0.01,
                    "{index} {boundary}"
                );
            }
        }
    }

    #[test]
    fn terrain_matches_road_and_is_continuous() {
        let mut track = Track::new(12);
        track.ensure(1500);
        for index in (60..1400).step_by(37) {
            let sample = track.sample(index);
            if track.is_structure(index) || track.tunnel_hill(sample.x, sample.z) {
                continue;
            }
            let terrain = track.terrain(sample.x, sample.z);
            assert!(
                (terrain.height - sample.y).abs() < 0.05,
                "road height mismatch at {index}"
            );
        }
        let mut worst = (0.0f32, 0usize, 0.0f32);
        for index in (100..1300).step_by(23) {
            for side in [-1.0f32, 1.0] {
                let start = track.sample(index);
                let (rx, rz) = start.right();
                let mut previous = track.terrain(start.x, start.z).height;
                for step in 1..300 {
                    let lateral = side * step as f32 * 0.5;
                    let height = track
                        .terrain(start.x + rx * lateral, start.z + rz * lateral)
                        .height;
                    let jump = (height - previous).abs();
                    let jump = if lateral.abs() < 14.0 {
                        jump * 0.5
                    } else {
                        jump
                    };
                    if jump > worst.0 {
                        worst = (jump, index, lateral);
                    }
                    previous = height;
                }
            }
        }
        assert!(worst.0 < 2.5, "worst step {worst:?}");
    }

    #[test]
    fn stages_only_cross_on_bridges() {
        for seed in 0..12u64 {
            let mut track = Track::new(seed);
            track.ensure(4000);
            let samples = &track.samples;
            let near_deck = |index: usize| {
                (index.saturating_sub(8)..(index + 8).min(samples.len()))
                    .any(|near| samples[near].deck)
            };
            for (index, sample) in samples.iter().enumerate().step_by(3) {
                for (other_index, other) in samples
                    .iter()
                    .enumerate()
                    .take(index.saturating_sub(40))
                    .step_by(2)
                {
                    let distance = (other.x - sample.x).hypot(other.z - sample.z);
                    if distance > 20.0 {
                        continue;
                    }
                    let rise = sample.y - other.y;
                    let bridged = (rise.abs() > DECK_RISE || distance > 12.0)
                        && (near_deck(index) || near_deck(other_index));
                    assert!(
                        bridged,
                        "seed {seed}: sample {index} within {distance:.1} m of {other_index}, rise {rise:.1}"
                    );
                    if distance < 10.0 {
                        let upper = if rise > 0.0 { sample } else { other };
                        assert!(upper.deck, "seed {seed}: crossing at {index} has no deck");
                    }
                }
            }
        }
    }

    #[test]
    fn ravines_and_tunnels_are_bridged_and_covered() {
        let (mut ravines, mut tunnels) = (0, 0);
        for seed in 0..12u64 {
            let mut track = Track::new(seed);
            track.ensure(4000);
            ravines += track.ravines.len();
            tunnels += track.tunnels.len();
            for ravine in &track.ravines {
                let (x, z) = (ravine.x, ravine.z);
                let terrain = track.terrain(x, z).height;
                assert!(terrain < ravine.road_y - ravine.depth * 0.6, "{terrain}");
                let (deck, road) = track.surface(x, z, ravine.road_y + 0.5);
                assert!(
                    road.is_some() && (deck - ravine.road_y).abs() < 1.0,
                    "{deck}"
                );
            }
            for river in &track.rivers {
                for point in &river.path {
                    let ground = track.terrain(point.x, point.z).height;
                    let level = track.settled_level(point.surface, point.x, point.z);
                    assert!(level < ground + 1.0, "{level} {ground}");
                }
            }
            for tunnel in &track.tunnels {
                for index in tunnel.start + 1..tunnel.end - 1 {
                    let sample = track.samples[index];
                    let cover = track.terrain(sample.x, sample.z).height;
                    assert!(
                        cover > sample.y + TUNNEL_HEIGHT,
                        "seed {seed} {index}: {cover} {}",
                        sample.y
                    );
                    let (inside, _) = track.surface(sample.x, sample.z, sample.y + 0.5);
                    assert!(
                        (inside - sample.y).abs() < 0.6,
                        "seed {seed} {index}: {inside}"
                    );
                }
            }
        }
        eprintln!("ravines {ravines} tunnels {tunnels}");
        assert!(ravines > 3 && tunnels > 3, "{ravines} {tunnels}");
    }

    #[test]
    fn locate_recovers_lateral_offset() {
        let track = Track::new(11);
        let sample = track.sample(120);
        let (x, z) = sample.offset(3.0);
        let (index, t, lateral) = track.locate(x, z, 115);
        assert!(index == 120 || index == 119, "{index}");
        assert!((lateral - 3.0).abs() < 0.3, "{lateral}");
        assert!((0.0..=1.0).contains(&t));
    }
}
