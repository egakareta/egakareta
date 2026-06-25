/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Shared serializable types and lightweight identifiers used across egakareta crates.

#![allow(missing_docs)]

mod app;
mod auth;
mod level;
mod settings;
mod waveform;

pub use app::*;
pub use auth::*;
pub use level::*;
pub use settings::*;
pub use waveform::*;
