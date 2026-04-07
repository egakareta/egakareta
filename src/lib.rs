/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
//! egakareta is a high-performance 3D rhythm game engine written in Rust.
//!
//! This crate provides the core game logic, editor functionality, and platform-specific
//! runtimes for both WebAssembly and native platforms. It uses `wgpu` for hardware-accelerated
//! rendering and `egui` for the user interface.

#![warn(missing_docs)]
mod audio_service;
mod block_repository;
mod commands;
mod editor_domain;
mod editor_ui;
mod game;
mod import_export_service;
mod level_codec;
mod level_repository;
mod mesh;
mod platform;
mod state;
#[cfg(test)]
mod test_utils;
mod types;

pub use editor_ui::{load_menu_wordmark_texture, show_editor_ui, show_menu_wordmark_ui};
pub use import_export_service::convert_level_binary_to_json;
pub use import_export_service::convert_level_json_to_binary;
pub use import_export_service::normalize_level_binary_format;
#[cfg(target_arch = "wasm32")]
pub use platform::application::run_game;
#[cfg(not(target_arch = "wasm32"))]
pub use platform::application::run_native_app;
#[cfg(not(target_arch = "wasm32"))]
pub use platform::input_mapping::{
    key_str_from_winit, mouse_button_index_from_winit, zoom_delta_from_winit,
};
pub use state::State;

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
