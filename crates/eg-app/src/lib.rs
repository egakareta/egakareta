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
mod platform;
mod tests;

pub use eg_ui::{
    MenuTextures, load_menu_textures, show_editor_ui_commands_from_view, show_editor_ui_from_view,
    show_menu_favicon_ui, show_menu_play_ui, show_menu_topbar_ui, show_pause_menu_ui,
    show_perf_overlay, show_practice_checkpoint_ui, show_settings_sidebar,
};
#[cfg(target_arch = "wasm32")]
pub use platform::application::run_game;
#[cfg(not(target_arch = "wasm32"))]
pub use platform::application::run_native_app;
#[cfg(not(target_arch = "wasm32"))]
pub use platform::input_mapping::{
    key_str_from_winit, mouse_button_index_from_winit, zoom_delta_from_winit,
};
#[cfg(target_arch = "wasm32")]
pub use wasm_bindgen_rayon::init_thread_pool;

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
