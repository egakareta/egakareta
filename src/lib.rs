mod block_repository;
mod editor_domain;
mod editor_ui;
mod game;
mod level_repository;
mod mesh;
mod platform;
mod state;
mod types;

pub use editor_ui::{load_menu_wordmark_texture, show_editor_ui, show_menu_wordmark_ui};
#[cfg(not(target_arch = "wasm32"))]
pub use platform::input_mapping::{
    key_str_from_winit, mouse_button_index_from_winit, zoom_delta_from_winit,
};
#[cfg(not(target_arch = "wasm32"))]
pub use platform::native_runtime::run_native_app;
#[cfg(target_arch = "wasm32")]
pub use platform::web_runtime::run_game;
pub use state::State;
