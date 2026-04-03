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
mod level_repository;
mod mesh;
mod platform;
mod state;
mod types;

pub use editor_ui::{
    load_menu_wordmark_texture, show_editor_ui, show_menu_wordmark_ui, show_splash_screen_ui,
};
#[cfg(target_arch = "wasm32")]
pub use platform::application::run_game;
#[cfg(not(target_arch = "wasm32"))]
pub use platform::application::run_native_app;
#[cfg(not(target_arch = "wasm32"))]
pub use platform::input_mapping::{
    key_str_from_winit, mouse_button_index_from_winit, zoom_delta_from_winit,
};
pub use state::State;
