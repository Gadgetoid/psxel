#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Gamepad {
    pub axes: [f32; 4],
    pub buttons: [f32; 17],
    pub standard: bool,
}

const RESTING_TRIGGER: f32 = 0.5;

pub fn calibrate(pad: &mut Gamepad, rest: &[f32; 4]) {
    for (axis, &centre) in pad.axes.iter_mut().zip(rest) {
        *axis = if centre.abs() > RESTING_TRIGGER {
            0.0
        } else {
            (*axis - centre).clamp(-1.0, 1.0)
        };
    }
}

impl Gamepad {
    pub const A: usize = 0;
    pub const B: usize = 1;
    pub const X: usize = 2;
    pub const Y: usize = 3;
    pub const LB: usize = 4;
    pub const RB: usize = 5;
    pub const LT: usize = 6;
    pub const RT: usize = 7;
    pub const SELECT: usize = 8;
    pub const START: usize = 9;
    pub const UP: usize = 12;
    pub const DOWN: usize = 13;
    pub const LEFT: usize = 14;
    pub const RIGHT: usize = 15;

    pub fn held(&self, button: usize) -> bool {
        self.buttons[button] > 0.5
    }

    pub fn value(&self, button: usize) -> f32 {
        self.buttons[button]
    }

    pub fn stick(&self, axis: usize, dead_zone: f32) -> f32 {
        let value = self.axes[axis];
        if value.abs() < dead_zone {
            0.0
        } else {
            (value - dead_zone * value.signum()) / (1.0 - dead_zone)
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub fn poll() -> Option<Gamepad> {
    use wasm_bindgen::JsCast;
    let pads = web_sys::window()?.navigator().get_gamepads().ok()?;
    for entry in pads.iter() {
        let Ok(pad) = entry.dyn_into::<web_sys::Gamepad>() else {
            continue;
        };
        if !pad.connected() {
            continue;
        }
        let mut state = Gamepad {
            standard: pad.mapping() == web_sys::GamepadMappingType::Standard,
            ..Default::default()
        };
        for (index, axis) in pad.axes().iter().take(4).enumerate() {
            state.axes[index] = axis.as_f64().unwrap_or(0.0) as f32;
        }
        for (index, button) in pad.buttons().iter().take(17).enumerate() {
            if let Ok(button) = button.dyn_into::<web_sys::GamepadButton>() {
                let value = button.value() as f32;
                state.buttons[index] = if button.pressed() {
                    value.max(1.0)
                } else {
                    value
                };
            }
        }
        return Some(state);
    }
    None
}

#[cfg(not(target_arch = "wasm32"))]
pub fn poll() -> Option<Gamepad> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calibration_ignores_resting_triggers_and_centres_sticks() {
        let mut pad = Gamepad {
            axes: [0.1, -1.0, 0.5, 0.0],
            ..Default::default()
        };
        calibrate(&mut pad, &[0.05, -1.0, 0.0, 0.0]);
        assert!((pad.axes[0] - 0.05).abs() < 1e-6);
        assert_eq!(pad.axes[1], 0.0);
        assert_eq!(pad.axes[2], 0.5);
    }
}
