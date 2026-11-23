use crate::mesh::{add, length, normalize, sub, Vec3};

pub struct Key {
    pub position: Vec3,
    pub target: Vec3,
    pub speed: f32,
}

pub fn key(position: Vec3, target: Vec3, speed: f32) -> Key {
    Key {
        position,
        target: normalize(sub(target, position)),
        speed,
    }
}

const STEPS: usize = 64;

pub struct Path {
    keys: Vec<Key>,
    times: Vec<f32>,
    pub duration: f32,
}

fn catmull_rom(p0: Vec3, p1: Vec3, p2: Vec3, p3: Vec3, t: f32) -> Vec3 {
    let (t2, t3) = (t * t, t * t * t);
    [0, 1, 2].map(|c| {
        0.5 * (2.0 * p1[c]
            + (p2[c] - p0[c]) * t
            + (2.0 * p0[c] - 5.0 * p1[c] + 4.0 * p2[c] - p3[c]) * t2
            + (3.0 * p1[c] - p0[c] - 3.0 * p2[c] + p3[c]) * t3)
    })
}

impl Path {
    pub fn new(keys: Vec<Key>) -> Self {
        let mut path = Self {
            keys,
            times: vec![0.0],
            duration: 0.0,
        };
        let count = path.keys.len() * STEPS;
        let mut previous = path.position(0.0);
        let mut time = 0.0;
        for step in 1..=count {
            let parameter = step as f32 / STEPS as f32;
            let position = path.position(parameter);
            let (segment, t) = path.segment(parameter - 0.5 / STEPS as f32);
            let next = (segment + 1) % path.keys.len();
            let speed =
                path.keys[segment].speed + (path.keys[next].speed - path.keys[segment].speed) * t;
            time += length(sub(position, previous)) / speed.max(0.1);
            path.times.push(time);
            previous = position;
        }
        path.duration = time;
        path
    }

    fn segment(&self, parameter: f32) -> (usize, f32) {
        let wrapped = parameter.rem_euclid(self.keys.len() as f32);
        (wrapped.floor() as usize % self.keys.len(), wrapped.fract())
    }

    fn spline(&self, parameter: f32, field: fn(&Key) -> Vec3) -> Vec3 {
        let (index, t) = self.segment(parameter);
        let count = self.keys.len() as i32;
        let at =
            |offset: i32| field(&self.keys[(index as i32 + offset).rem_euclid(count) as usize]);
        catmull_rom(at(-1), at(0), at(1), at(2), t)
    }

    pub fn position(&self, parameter: f32) -> Vec3 {
        self.spline(parameter, |key| key.position)
    }

    fn parameter(&self, time: f32) -> f32 {
        let time = time.rem_euclid(self.duration);
        let index = self
            .times
            .partition_point(|&t| t <= time)
            .clamp(1, self.times.len() - 1);
        let (t0, t1) = (self.times[index - 1], self.times[index]);
        let fraction = if t1 > t0 {
            (time - t0) / (t1 - t0)
        } else {
            0.0
        };
        (index as f32 - 1.0 + fraction) / STEPS as f32
    }

    pub fn sample(&self, time: f32) -> (Vec3, Vec3) {
        let parameter = self.parameter(time);
        let position = self.position(parameter);
        let direction = normalize(self.spline(parameter, |key| key.target));
        (position, add(position, direction))
    }

    pub fn points(&self, per_segment: usize) -> Vec<Vec3> {
        (0..self.keys.len() * per_segment)
            .map(|index| self.position(index as f32 / per_segment as f32))
            .collect()
    }
}
