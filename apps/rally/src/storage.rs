const KEY: &str = "psx-rally-best";

#[cfg(target_arch = "wasm32")]
fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

#[cfg(target_arch = "wasm32")]
pub fn load_best() -> f32 {
    storage()
        .and_then(|storage| storage.get_item(KEY).ok().flatten())
        .and_then(|value| value.parse().ok())
        .unwrap_or(0.0)
}

#[cfg(target_arch = "wasm32")]
pub fn save_best(best: f32) {
    if let Some(storage) = storage() {
        let _ = storage.set_item(KEY, &best.to_string());
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_best() -> f32 {
    let _ = KEY;
    0.0
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_best(_best: f32) {}
