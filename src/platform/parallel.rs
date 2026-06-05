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

pub(crate) fn spawn_cpu_bound<F>(job: F)
where
    F: FnOnce() + Send + 'static,
{
    #[cfg(target_arch = "wasm32")]
    {
        request_rayon_init();
        if rayon_is_ready() {
            rayon::spawn(job);
        } else {
            schedule_cpu_bound_when_rayon_ready(Box::new(job), 0);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        rayon::spawn(job);
    }
}

#[cfg(target_arch = "wasm32")]
fn request_rayon_init() {
    use wasm_bindgen::JsCast;

    let Some(window) = web_sys::window() else {
        return;
    };

    let Ok(init_rayon) = js_sys::Reflect::get(
        &window,
        &wasm_bindgen::JsValue::from_str("__EGAKARETA_INIT_RAYON"),
    ) else {
        log::warn!("Rayon worker pool initializer is unavailable");
        return;
    };

    if let Some(init_rayon) = init_rayon.dyn_ref::<js_sys::Function>() {
        if let Err(err) = init_rayon.call0(&wasm_bindgen::JsValue::NULL) {
            log::warn!("Failed to start Rayon worker pool: {:?}", err);
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn schedule_cpu_bound_when_rayon_ready(job: Box<dyn FnOnce() + Send>, attempt: u32) {
    use wasm_bindgen::JsCast;

    if rayon_is_ready() {
        rayon::spawn(move || job());
        return;
    }

    const MAX_RAYON_READY_ATTEMPTS: u32 = 120;
    if attempt >= MAX_RAYON_READY_ATTEMPTS {
        wasm_bindgen_futures::spawn_local(async move {
            job();
        });
        return;
    }

    let Some(window) = web_sys::window() else {
        wasm_bindgen_futures::spawn_local(async move {
            job();
        });
        return;
    };

    let callback = wasm_bindgen::closure::Closure::<dyn FnMut()>::once(move || {
        schedule_cpu_bound_when_rayon_ready(job, attempt + 1);
    });

    if let Err(err) = window.set_timeout_with_callback_and_timeout_and_arguments_0(
        callback.as_ref().unchecked_ref(),
        16,
    ) {
        log::warn!("Failed to schedule CPU job for Rayon readiness: {:?}", err);
    }

    callback.forget();
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
/// Marks the browser Rayon worker pool as initialized.
pub fn mark_rayon_ready() {
    RAYON_READY.store(true, Ordering::Release);
}
