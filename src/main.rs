/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
#[cfg(not(target_arch = "wasm32"))]
fn main() {
    egakareta_lib::run_native_app();
}

#[cfg(target_arch = "wasm32")]
fn main() {}
