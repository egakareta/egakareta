/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! egakareta is a high-performance 3D rhythm game engine written in Rust.
//!
//! This crate provides the core game logic, editor functionality, and platform-specific
//! runtimes for both WebAssembly and native platforms. It uses `wgpu` for hardware-accelerated
//! rendering and `egui` for the user interface.

#![warn(missing_docs)]
#[cfg(feature = "benchmarks")]
pub mod bench_support;
mod bevy_app;
mod tests;

pub use bevy_app::{EgakaretaPlugin, GAME_CANVAS_SELECTOR, build_bevy_app};
pub use eg_ui::{
    MenuTextures, load_menu_textures, show_editor_ui, show_menu_favicon_ui, show_menu_play_ui,
    show_menu_topbar_ui, show_pause_menu_ui, show_perf_overlay, show_practice_checkpoint_ui,
    show_settings_sidebar,
};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::{JsCast, prelude::*};
#[cfg(target_arch = "wasm32")]
pub use wasm_bindgen_rayon::init_thread_pool;

/// Start the browser build through the configured Bevy application root.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub async fn run_game() -> Result<(), JsValue> {
    let browser_window = web_sys::window().ok_or_else(|| JsValue::from_str("Missing window"))?;
    let document = browser_window
        .document()
        .ok_or_else(|| JsValue::from_str("Missing document"))?;
    let canvas = match document.get_element_by_id("gameCanvas") {
        Some(existing) => existing.dyn_into::<web_sys::HtmlCanvasElement>()?,
        None => {
            let canvas = document
                .create_element("canvas")?
                .dyn_into::<web_sys::HtmlCanvasElement>()?;
            canvas.set_id("gameCanvas");
            document
                .body()
                .ok_or_else(|| JsValue::from_str("Missing document body"))?
                .append_child(&canvas)?;
            canvas
        }
    };
    canvas.set_attribute(
        "style",
        "display:block;width:100vw;height:100vh;touch-action:none;",
    )?;

    let mut app = build_bevy_app();
    let _ = app.run();
    Ok(())
}

#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
/// Polyfill for `asinh` in WebAssembly (used by C dependencies).
pub extern "C" fn asinh(value: f64) -> f64 {
    value.asinh()
}

#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
/// Polyfill for `asinhf` in WebAssembly (used by C dependencies).
pub extern "C" fn asinhf(value: f32) -> f32 {
    value.asinh()
}

#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
/// Polyfill for `acosh` in WebAssembly (used by C dependencies).
pub extern "C" fn acosh(value: f64) -> f64 {
    value.acosh()
}

#[cfg(target_arch = "wasm32")]
#[unsafe(no_mangle)]
/// Polyfill for `acoshf` in WebAssembly (used by C dependencies).
pub extern "C" fn acoshf(value: f32) -> f32 {
    value.acosh()
}
