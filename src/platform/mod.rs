pub(crate) mod audio;
pub(crate) mod input_mapping;
pub(crate) mod io;
pub(crate) mod pipeline;
pub(crate) mod runtime;
pub(crate) mod services;
pub(crate) mod state_host;

#[cfg(not(target_arch = "wasm32"))]
pub mod native_runtime;

#[cfg(target_arch = "wasm32")]
pub mod web_runtime;
