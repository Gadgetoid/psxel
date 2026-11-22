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
}

pub struct Track {
    pub seed: u64,
    pub samples: Vec<Sample>,
    pub notes: Vec<PaceNote>,
    pub biomes: BiomeSchedule,
    rng: Rng,
    bumps: Vec<Bump>,
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
const PREVIEW_IGNORE: usize = 30;
const PREVIEW_CLEARANCE: f32 = 35.0;
const SECTION_FADE: f32 = 45.0;

const FAR_BLEND_START: f32 = 30.0;
const FAR_BLEND_LENGTH: f32 = 70.0;
const BLEND_RANGE: f32 = 30.0;
const BLEND_SOFTNESS: f32 = 9.0;
const OVERSHOOT_SOFTNESS: f32 = 1.5;

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
            };
            sample.y = self.elevation(Self::distance(index));
            self.samples.push(sample);
            self.clamp_inside_of_bend(index);
            self.clamp_after_bend(index);
            self.register(index);
        }
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

    fn preview_clear(&self, count: usize, target_curvature: f32) -> bool {
        self.preview_gap(count, target_curvature) >= PREVIEW_CLEARANCE
    }

    fn preview_gap(&self, count: usize, target_curvature: f32) -> f32 {
        let mut gap = f32::MAX;
        let Some(last) = self.samples.last().copied() else {
            return gap;
        };
        let newest = self.samples.len();
        let (mut x, mut z, mut heading, mut curvature) =
            (last.x, last.z, last.heading, self.curvature);
        for _ in 0..count + PREVIEW_MARGIN {
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
                            if index + PREVIEW_IGNORE >= newest {
                                continue;
                            }
                            let other = self.samples[index];
                            gap = gap.min(((other.x - x).powi(2) + (other.z - z).powi(2)).sqrt());
                        }
                    }
                }
            }
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
                let d = (distance - bump.center) / bump.width;
                bump.height * (-d * d).exp()
            })
            .sum();
        hills * hilliness * start_ramp + bumps
    }

    fn generate_turn(&mut self, start: usize, heading: f32, deviation: f32) {
        let severity = self
            .rng
            .pick_weighted(&[4.0, 6.0, 13.0, 20.0, 22.0, 20.0, 15.0]) as u8;
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
            let gap = self.preview_gap(length, candidate.sign() / candidate_radius);
            let aligned = (final_heading - self.general_heading).abs() < MAX_DEVIATION;
            (gap, aligned)
        };
        let chosen = candidates
            .iter()
            .find(|candidate| {
                let (gap, aligned) = assess(candidate);
                aligned && gap >= PREVIEW_CLEARANCE
            })
            .or_else(|| {
                candidates
                    .iter()
                    .max_by(|a, b| assess(a).0.total_cmp(&assess(b).0))
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
            self.push_samples(length, 0.0);
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
        for point in &points {
            let excess = point.distance - nearest.distance;
            if excess > BLEND_RANGE {
                continue;
            }
            let weight = (-(excess / BLEND_SOFTNESS).powi(2)).exp()
                * (-(point.overshoot / OVERSHOOT_SOFTNESS).powi(2)).exp();
            if weight < 1e-6 {
                continue;
            }
            weighted += self.profile_height(point, x, z) * weight;
            total += weight;
        }
        let far = self.far_height(x, z);
        Terrain {
            height: if total > 0.0 { weighted / total } else { far },
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
    fn stages_never_cross_themselves() {
        for seed in 0..12u64 {
            let mut track = Track::new(seed);
            track.ensure(4000);
            let samples = &track.samples;
            for (index, sample) in samples.iter().enumerate().step_by(3) {
                for other in samples.iter().take(index.saturating_sub(40)).step_by(2) {
                    let distance =
                        ((other.x - sample.x).powi(2) + (other.z - sample.z).powi(2)).sqrt();
                    assert!(
                        distance > 20.0,
                        "seed {seed}: sample {index} within {distance:.1} m of earlier road"
                    );
                }
            }
        }
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
