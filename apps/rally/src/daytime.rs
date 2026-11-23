use crate::biome::{lerp_rgb, scale_rgb, Rgb};

pub const CYCLE_SECONDS: f32 = 480.0;
const DAY_END: f32 = 0.5;
const DUSK_END: f32 = 0.62;
const NIGHT_END: f32 = 0.88;
const DAWN_START: f32 = NIGHT_END - 0.02;
const DAWN_AFTER: f32 = 0.06;

const NIGHT_SKY_TOP: Rgb = [10, 14, 32];
const NIGHT_SKY_HORIZON: Rgb = [28, 34, 60];
const NIGHT_FOG: Rgb = [18, 22, 40];
const NIGHT_AMBIENT: Rgb = [40, 46, 74];
const DUSK_HORIZON: Rgb = [245, 140, 70];
const DUSK_FOG: Rgb = [160, 95, 75];
const DUSK_SUN: Rgb = [255, 150, 80];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Phase {
    pub daylight: f32,
    pub warmth: f32,
    pub elevation: f32,
}

pub fn phase(clock: f32) -> Phase {
    let clock = clock.rem_euclid(1.0);
    let smooth = |x: f32| {
        let x = x.clamp(0.0, 1.0);
        x * x * (3.0 - 2.0 * x)
    };
    let (daylight, warmth) = if clock < DAY_END {
        (1.0, smooth((clock - (DAY_END - 0.06)) / 0.06) * 0.6)
    } else if clock < DUSK_END {
        let t = (clock - DAY_END) / (DUSK_END - DAY_END);
        (
            1.0 - smooth(t),
            (1.0 - (t * 2.0 - 1.0).abs()).max(0.0) * 0.4 + 0.6 * (1.0 - smooth(t)),
        )
    } else if clock < NIGHT_END {
        (0.0, 0.0)
    } else {
        let t = (clock - NIGHT_END) / (1.0 - NIGHT_END);
        (smooth(t), (1.0 - (t * 2.0 - 1.0).abs()).max(0.0))
    };
    let elevation = 0.15 + 0.6 * daylight * (1.0 - warmth * 0.6);
    Phase {
        daylight,
        warmth,
        elevation,
    }
}

pub fn dawn(clock: f32) -> f32 {
    let into = (clock.rem_euclid(1.0) - DAWN_START).rem_euclid(1.0);
    let t = into / (1.0 - DAWN_START + DAWN_AFTER);
    if t >= 1.0 {
        return 0.0;
    }
    (t * std::f32::consts::PI).sin()
}

pub struct Sky {
    pub top: Rgb,
    pub horizon: Rgb,
    pub fog: Rgb,
    pub sun: Rgb,
    pub ambient: Rgb,
}

pub fn apply(phase: Phase, day: Sky) -> Sky {
    let dusk = |color: Rgb, tint: Rgb, amount: f32| lerp_rgb(color, tint, phase.warmth * amount);
    let night = |color: Rgb, dark: Rgb| lerp_rgb(dark, color, phase.daylight);
    Sky {
        top: night(dusk(day.top, [70, 60, 110], 0.5), NIGHT_SKY_TOP),
        horizon: night(dusk(day.horizon, DUSK_HORIZON, 0.85), NIGHT_SKY_HORIZON),
        fog: night(dusk(day.fog, DUSK_FOG, 0.7), NIGHT_FOG),
        sun: scale_rgb(dusk(day.sun, DUSK_SUN, 0.8), phase.daylight),
        ambient: night(dusk(day.ambient, [90, 60, 60], 0.4), NIGHT_AMBIENT),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cycle_has_day_dusk_night_and_dawn() {
        assert_eq!(phase(0.2).daylight, 1.0);
        assert!(phase(0.56).warmth > 0.5);
        assert_eq!(phase(0.75).daylight, 0.0);
        assert!(phase(0.94).warmth > 0.5);
        assert!((phase(0.999).daylight - 1.0).abs() < 0.01);
        let mut previous = phase(0.0).daylight;
        for step in 1..1000 {
            let current = phase(step as f32 / 1000.0).daylight;
            assert!((current - previous).abs() < 0.05);
            previous = current;
        }
    }
}
