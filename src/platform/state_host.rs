use crate::types::PhysicalSize;

#[cfg(target_arch = "wasm32")]
pub(crate) type WasmCanvas = web_sys::HtmlCanvasElement;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) type NativeWindow = winit::window::Window;

pub(crate) type PlatformInstant = web_time::Instant;

pub(crate) enum SurfaceHost {
    #[cfg(target_arch = "wasm32")]
    Canvas(WasmCanvas),
    #[cfg(not(target_arch = "wasm32"))]
    Window(NativeWindow),
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
            .create_surface(&window)
            .expect("Failed to create surface");
        let surface =
            unsafe { std::mem::transmute::<wgpu::Surface<'_>, wgpu::Surface<'static>>(surface) };

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
            }
        }
    }
}
