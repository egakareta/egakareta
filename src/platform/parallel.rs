/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
#[cfg(target_arch = "wasm32")]
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::wasm_bindgen;

#[cfg(target_arch = "wasm32")]
static RAYON_READY: AtomicBool = AtomicBool::new(false);

pub(crate) fn rayon_is_ready() -> bool {
    #[cfg(target_arch = "wasm32")]
    {
        RAYON_READY.load(Ordering::Acquire)
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        true
    }
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
/// Marks the browser Rayon worker pool as initialized.
pub fn mark_rayon_ready() {
    RAYON_READY.store(true, Ordering::Release);
}
