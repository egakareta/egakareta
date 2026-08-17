/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    env_logger::init();
    let mut app = egakareta_lib::build_bevy_app();
    let _ = app.run();
}

#[cfg(target_arch = "wasm32")]
fn main() {}
