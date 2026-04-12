/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::types::PhysicalSize;

#[cfg(not(target_arch = "wasm32"))]
use std::sync::Arc;
#[cfg(all(test, not(target_arch = "wasm32")))]
use std::sync::Mutex;

#[cfg(target_arch = "wasm32")]
pub(crate) type WasmCanvas = web_sys::HtmlCanvasElement;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) type NativeWindow = Arc<winit::window::Window>;

pub(crate) type PlatformInstant = web_time::Instant;

pub(crate) enum SurfaceHost {
    #[cfg(target_arch = "wasm32")]
    Canvas(WasmCanvas),
    #[cfg(not(target_arch = "wasm32"))]
    Window(NativeWindow),
    #[cfg(all(test, not(target_arch = "wasm32")))]
    TestSize(Mutex<PhysicalSize<u32>>),
}

impl SurfaceHost {
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn create_for_wasm(
        canvas: WasmCanvas,
    ) -> (
        SurfaceHost,
        wgpu::Instance,
        wgpu::Surface<'static>,
        PhysicalSize<u32>,
    ) {
        let size = PhysicalSize::new(canvas.width(), canvas.height());

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .expect("Failed to create surface");

        (SurfaceHost::Canvas(canvas), instance, surface, size)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn create_for_native(
        window: NativeWindow,
    ) -> (
        SurfaceHost,
        wgpu::Instance,
        wgpu::Surface<'static>,
        PhysicalSize<u32>,
    ) {
        let size = PhysicalSize::new(window.inner_size().width, window.inner_size().height);

        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window.clone())
            .expect("Failed to create surface");

        (SurfaceHost::Window(window), instance, surface, size)
    }

    pub(crate) fn prepare_resize(&self, new_size: PhysicalSize<u32>) {
        #[cfg(target_arch = "wasm32")]
        {
            match self {
                SurfaceHost::Canvas(canvas) => {
                    canvas.set_width(new_size.width);
                    canvas.set_height(new_size.height);
                }
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = new_size;
        }
    }

    pub(crate) fn current_size(&self) -> PhysicalSize<u32> {
        #[cfg(target_arch = "wasm32")]
        {
            match self {
                SurfaceHost::Canvas(canvas) => PhysicalSize::new(canvas.width(), canvas.height()),
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            match self {
                SurfaceHost::Window(window) => {
                    let size = window.inner_size();
                    PhysicalSize::new(size.width, size.height)
                }
                #[cfg(all(test, not(target_arch = "wasm32")))]
                SurfaceHost::TestSize(size) => {
                    let guard = size.lock().unwrap_or_else(|error| error.into_inner());
                    *guard
                }
            }
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use std::sync::Mutex;

    use super::{PhysicalSize, SurfaceHost};

    #[test]
    fn current_size_returns_configured_size_for_test_host() {
        let host = SurfaceHost::TestSize(Mutex::new(PhysicalSize::new(1280, 720)));

        let size = host.current_size();

        assert_eq!(size.width, 1280);
        assert_eq!(size.height, 720);
    }

    #[test]
    fn prepare_resize_is_noop_for_native_surface_host() {
        let host = SurfaceHost::TestSize(Mutex::new(PhysicalSize::new(640, 360)));

        host.prepare_resize(PhysicalSize::new(1920, 1080));

        let size_after_prepare = host.current_size();
        assert_eq!(size_after_prepare.width, 640);
        assert_eq!(size_after_prepare.height, 360);
    }
}
