/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Cross-platform service adapters shared by the egakareta app crates.

#![allow(missing_docs)]

pub mod auth;
pub mod io;
pub mod services;
pub mod storage;

#[cfg(all(test, not(target_arch = "wasm32")))]
pub(crate) mod test_hooks;
