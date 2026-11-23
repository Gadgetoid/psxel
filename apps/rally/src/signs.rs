use crate::art::Ground;
use crate::biome::BIOMES;
use crate::rng::{hash, hash_float};
use crate::track::{Direction, NoteKind, Track, SAMPLE};

const LEAD: usize = 12;
const ROCKS_LEAD: usize = 20;
const HILL_SPAN: usize = 12;
const STEEP: f32 = 0.12;
const BEND_SEVERITY: u8 = 3;
const BEND_CHANCE: f32 = 0.8;
const DOUBLE_BEND_GAP: usize = 14;
const NARROW_ENTRY: f32 = 0.5;
const ROCKS_CHANCE: f32 = 0.6;
const ROCKS_HILLINESS: f32 = 1.0;
const ANIMAL_CHANCE: f32 = 0.0007;
const QUIET_LANE_CHANCE: f32 = 0.002;
const QUIET_CURVATURE: f32 = 1.0 / 200.0;
const SPACING: usize = 3;
const AHEAD: usize = ROCKS_LEAD + HILL_SPAN + 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignKind {
    Bend,
    DoubleBend,
    Narrows,
    NarrowsLeft,
    NarrowsRight,
    HillDown,
    HillUp,
    Tunnel,
    Cattle,
    Sheep,
    Deer,
    Slippery,
    Rocks,
    QuietLane,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoadSign {
    pub kind: SignKind,
    pub mirror: bool,
}

impl RoadSign {
    fn plain(kind: SignKind) -> Self {
        Self {
            kind,
            mirror: false,
        }
    }
}

pub fn road_sign(track: &Track, index: usize) -> Option<RoadSign> {
    let sign = candidate(track, index)?;
    let crowded =
        (1..=SPACING).any(|back| index >= back && candidate(track, index - back).is_some());
    (!crowded).then_some(sign)
}

fn candidate(track: &Track, index: usize) -> Option<RoadSign> {
    if index < LEAD || index + AHEAD >= track.samples.len() || track.is_structure(index) {
        return None;
    }
    let ahead = index + LEAD;
    tunnel(track, ahead)
        .or_else(|| narrows(track, ahead))
        .or_else(|| bend(track, index, ahead))
        .or_else(|| hill(track, ahead))
        .or_else(|| slippery(track, ahead))
        .or_else(|| rocks(track, index))
        .or_else(|| roadside(track, index))
}

fn tunnel(track: &Track, ahead: usize) -> Option<RoadSign> {
    track
        .tunnels
        .iter()
        .any(|tunnel| tunnel.start == ahead)
        .then_some(RoadSign::plain(SignKind::Tunnel))
}

fn narrowing(track: &Track, index: usize) -> f32 {
    let character = track.character(index);
    character.cutting + character.ledge.abs()
}

fn entering_narrows(track: &Track, ahead: usize) -> bool {
    narrowing(track, ahead) > NARROW_ENTRY && narrowing(track, ahead - 1) <= NARROW_ENTRY
}

fn narrows(track: &Track, ahead: usize) -> Option<RoadSign> {
    if !entering_narrows(track, ahead) {
        return None;
    }
    let character = track.character(ahead);
    let kind = if character.ledge < 0.0 {
        SignKind::NarrowsLeft
    } else if character.ledge > 0.0 {
        SignKind::NarrowsRight
    } else {
        SignKind::Narrows
    };
    Some(RoadSign::plain(kind))
}

fn turn_starting(track: &Track, start: usize) -> Option<(Direction, u8, usize)> {
    let first = track.notes.partition_point(|note| note.start < start);
    let note = track.notes.get(first).filter(|note| note.start == start)?;
    match note.kind {
        NoteKind::Turn {
            direction,
            severity,
        } => Some((direction, severity, note.end)),
        _ => None,
    }
}

fn bend(track: &Track, index: usize, ahead: usize) -> Option<RoadSign> {
    let (direction, severity, end) = turn_starting(track, ahead)?;
    if severity > BEND_SEVERITY || hash_float(track.seed ^ 0xbe4d, index as u64, 0) > BEND_CHANCE {
        return None;
    }
    let first = track.notes.partition_point(|note| note.start <= ahead);
    let previous = first
        .checked_sub(2)
        .and_then(|before| track.notes.get(before));
    if previous.is_some_and(|note| {
        note.end + DOUBLE_BEND_GAP >= ahead && matches!(note.kind, NoteKind::Turn { .. })
    }) {
        return None;
    }
    let follows = track.notes.get(first).is_some_and(|next| {
        next.start <= end + DOUBLE_BEND_GAP
            && matches!(next.kind, NoteKind::Turn { direction: next_direction, .. } if next_direction != direction)
    });
    Some(if follows {
        RoadSign {
            kind: SignKind::DoubleBend,
            mirror: direction == Direction::Right,
        }
    } else {
        RoadSign {
            kind: SignKind::Bend,
            mirror: direction == Direction::Left,
        }
    })
}

fn gradient(track: &Track, index: usize) -> f32 {
    (track.samples[index + HILL_SPAN].y - track.samples[index].y) / (HILL_SPAN as f32 * SAMPLE)
}

fn hill(track: &Track, ahead: usize) -> Option<RoadSign> {
    let (now, before) = (gradient(track, ahead), gradient(track, ahead - 1));
    if now >= STEEP && before < STEEP {
        Some(RoadSign::plain(SignKind::HillUp))
    } else if now <= -STEEP && before > -STEEP {
        Some(RoadSign::plain(SignKind::HillDown))
    } else {
        None
    }
}

fn slippery(track: &Track, ahead: usize) -> Option<RoadSign> {
    let first = track.notes.partition_point(|note| note.start < ahead);
    let icy = track
        .notes
        .get(first)
        .is_some_and(|note| note.start == ahead && note.kind == NoteKind::Ice);
    let into_snow =
        track.ground_at(ahead) == Ground::Snow && track.ground_at(ahead - 1) != Ground::Snow;
    (icy || into_snow).then_some(RoadSign::plain(SignKind::Slippery))
}

fn rocks(track: &Track, index: usize) -> Option<RoadSign> {
    let ahead = index + ROCKS_LEAD;
    let hilliness = track
        .biomes
        .scalar(Track::distance(ahead), |biome| biome.hilliness);
    if hilliness < ROCKS_HILLINESS
        || !entering_narrows(track, ahead)
        || hash_float(track.seed ^ 0x40c5, ahead as u64, 0) > ROCKS_CHANCE
    {
        return None;
    }
    Some(RoadSign {
        kind: SignKind::Rocks,
        mirror: track.character(ahead).ledge > 0.0,
    })
}

fn roadside(track: &Track, index: usize) -> Option<RoadSign> {
    let key = hash(track.seed ^ 0x5197, index as u64, 0);
    let roll = hash_float(key, 1, 0);
    let (first, second, t) = track.biomes.at(Track::distance(index));
    let ground = BIOMES[if t > 0.5 { second } else { first }].ground;
    if roll < ANIMAL_CHANCE {
        let pick = hash_float(key, 2, 0);
        let kind = match ground {
            Ground::Wheat if pick < 0.5 => SignKind::Cattle,
            Ground::Wheat | Ground::Snow => SignKind::Sheep,
            Ground::Grass | Ground::Leaves if pick < 0.25 => SignKind::Sheep,
            Ground::Grass | Ground::Leaves | Ground::Jungle => SignKind::Deer,
            Ground::Earth => return None,
        };
        return Some(RoadSign::plain(kind));
    }
    let quiet = roll < ANIMAL_CHANCE + QUIET_LANE_CHANCE
        && matches!(ground, Ground::Wheat | Ground::Leaves)
        && track.samples[index].curvature.abs() < QUIET_CURVATURE;
    quiet.then_some(RoadSign::plain(SignKind::QuietLane))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signs_cover_every_kind_and_keep_apart() {
        let mut seen = Vec::new();
        let mut placed = 0;
        for seed in 0..12u64 {
            let mut track = Track::new(seed);
            track.ensure(6000);
            let mut last = None;
            for index in 0..5900 {
                if let Some(sign) = road_sign(&track, index) {
                    placed += 1;
                    if let Some(previous) = last {
                        assert!(index - previous > SPACING, "{seed} {index}");
                    }
                    last = Some(index);
                    if !seen.contains(&sign.kind) {
                        seen.push(sign.kind);
                    }
                }
            }
        }
        assert!(seen.len() == 14, "{seen:?}");
        assert!(placed > 200, "{placed}");
    }
}
