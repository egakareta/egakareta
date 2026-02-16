pub(crate) mod audio;
pub(crate) mod input_mapping;
pub(crate) mod io;
pub(crate) mod pipeline;
pub(crate) mod runtime;
pub(crate) mod services;
pub(crate) mod state_host;

#[cfg(not(target_arch = "wasm32"))]
/// Native runtime for desktop platforms, providing the entry point and window management using winit and rodio.
pub mod native_runtime;

#[cfg(target_arch = "wasm32")]
/// Web runtime for WASM targets, providing the browser entry point and web-specific APIs.
pub mod web_runtime;
