/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
pub(crate) mod block_icon;
pub(crate) mod draw;

use crate::platform::state_host::SurfaceHost;
use crate::types::{PhysicalSize, Vertex};
use wgpu::util::DeviceExt;

use super::State;

impl State {
    pub(crate) fn surface_width(&self) -> u32 {
        self.render.gpu.surface_width()
    }

    pub(crate) fn surface_height(&self) -> u32 {
        self.render.gpu.surface_height()
    }

    pub(crate) fn handle_surface_lost(&mut self) {
        let size = self.render.gpu.current_size();
        self.resize_surface(size);
    }
}

pub(crate) const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

pub(crate) enum MeshSlot {
    Empty,
    VertexData {
        buffer: wgpu::Buffer,
        count: u32,
    },
    Streaming {
        buffer: wgpu::Buffer,
        count: u32,
        capacity_vertices: u32,
    },
}

impl MeshSlot {
    pub(crate) fn from_vertices(
        device: &wgpu::Device,
        label: &'static str,
        vertices: &[Vertex],
    ) -> Self {
        if vertices.is_empty() {
            return Self::Empty;
        }

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        Self::VertexData {
            buffer,
            count: vertices.len() as u32,
        }
    }

    pub(crate) fn streaming(
        device: &wgpu::Device,
        label: &'static str,
        capacity_vertices: u32,
    ) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: (std::mem::size_of::<Vertex>() * capacity_vertices as usize) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self::Streaming {
            buffer,
            count: 0,
            capacity_vertices,
        }
    }

    pub(crate) fn replace_with_vertices(
        &mut self,
        device: &wgpu::Device,
        label: &'static str,
        vertices: &[Vertex],
    ) {
        *self = Self::from_vertices(device, label, vertices);
    }

    pub(crate) fn write_streaming_vertices(&mut self, queue: &wgpu::Queue, vertices: &[Vertex]) {
        match self {
            Self::Streaming {
                buffer,
                count,
                capacity_vertices,
            } => {
                let write_count = vertices.len().min(*capacity_vertices as usize);
                *count = write_count as u32;
                if write_count > 0 {
                    queue.write_buffer(buffer, 0, bytemuck::cast_slice(&vertices[..write_count]));
                }
            }
            _ => {
                *self = Self::Empty;
            }
        }
    }

    pub(crate) fn write_streaming_vertices_with_growth(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &'static str,
        vertices: &[Vertex],
    ) {
        if vertices.is_empty() {
            self.clear();
            return;
        }

        let required_vertices = vertices.len() as u32;
        let requires_realloc = !matches!(
            self,
            Self::Streaming {
                capacity_vertices,
                ..
            } if *capacity_vertices >= required_vertices
        );

        if requires_realloc {
            let capacity_vertices = required_vertices.max(512).next_power_of_two();
            *self = Self::streaming(device, label, capacity_vertices);
        }

        self.write_streaming_vertices(queue, vertices);
    }

    pub(crate) fn append_streaming_vertices_with_growth(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &'static str,
        vertices: &[Vertex],
    ) -> bool {
        if vertices.is_empty() {
            return true;
        }

        if matches!(self, Self::Empty) {
            let capacity_vertices = (vertices.len() as u32).max(512).next_power_of_two();
            *self = Self::streaming(device, label, capacity_vertices);
        }

        let Self::Streaming {
            buffer,
            count,
            capacity_vertices,
        } = self
        else {
            return false;
        };

        let append_count = vertices.len() as u32;
        let next_count = count.saturating_add(append_count);
        if next_count > *capacity_vertices {
            return false;
        }

        let offset_bytes = (*count as usize * std::mem::size_of::<Vertex>()) as u64;
        queue.write_buffer(buffer, offset_bytes, bytemuck::cast_slice(vertices));
        *count = next_count;
        true
    }

    pub(crate) fn clear(&mut self) {
        match self {
            Self::Empty => {}
            Self::VertexData { .. } => *self = Self::Empty,
            Self::Streaming { count, .. } => *count = 0,
        }
    }

    pub(crate) fn draw_data(&self) -> Option<(&wgpu::Buffer, u32)> {
        match self {
            Self::Empty => None,
            Self::VertexData { buffer, count } | Self::Streaming { buffer, count, .. }
                if *count > 0 =>
            {
                Some((buffer, *count))
            }
            _ => None,
        }
    }
}

pub(crate) struct SceneMeshes {
    pub(crate) floor: MeshSlot,
    pub(crate) grid: MeshSlot,
    pub(crate) trail: MeshSlot,
    pub(crate) blocks: MeshSlot,
    pub(crate) blocks_static: MeshSlot,
    pub(crate) blocks_static_chunks: Vec<MeshSlot>,
    pub(crate) blocks_selected: MeshSlot,
    pub(crate) editor_cursor: MeshSlot,
    pub(crate) editor_hover_outline: MeshSlot,
    pub(crate) editor_selection_outline: MeshSlot,
    pub(crate) editor_gizmo: MeshSlot,
    pub(crate) tap_indicators: MeshSlot,
    pub(crate) spawn_marker: MeshSlot,
    pub(crate) camera_trigger_markers: MeshSlot,
    pub(crate) editor_preview_player: MeshSlot,
}

pub(crate) struct GpuContext {
    pub(crate) surface_host: Option<SurfaceHost>,
    pub(crate) surface: Option<wgpu::Surface<'static>>,
    pub(crate) adapter_info: wgpu::AdapterInfo,
    pub(crate) device: wgpu::Device,
    pub(crate) queue: wgpu::Queue,
    pub(crate) config: wgpu::SurfaceConfiguration,
    pub(crate) size: PhysicalSize<u32>,
    pub(crate) depth_texture: wgpu::Texture,
    pub(crate) depth_view: wgpu::TextureView,
    pub(crate) render_pipeline: wgpu::RenderPipeline,
    pub(crate) block_icon_pipeline: wgpu::RenderPipeline,
    pub(crate) editor_ghost_trail_pipeline: wgpu::RenderPipeline,
    pub(crate) gizmo_overlay_pipeline: wgpu::RenderPipeline,
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) line_bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) line_uniform_buffer: wgpu::Buffer,
    pub(crate) zero_line_bind_group: wgpu::BindGroup,
    pub(crate) camera_uniform_buffer: wgpu::Buffer,
    pub(crate) camera_bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) camera_bind_group: wgpu::BindGroup,
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) color_space_bind_group_layout: wgpu::BindGroupLayout,
    pub(crate) color_space_uniform_buffer: wgpu::Buffer,
    pub(crate) color_space_bind_group: wgpu::BindGroup,
    pub(crate) block_texture_bind_group: wgpu::BindGroup,
    pub(crate) apply_gamma_correction: bool,
}

impl GpuContext {
    pub(crate) fn surface_width(&self) -> u32 {
        self.config.width
    }

    pub(crate) fn surface_height(&self) -> u32 {
        self.config.height
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn window(&self) -> Option<&crate::platform::state_host::NativeWindow> {
        match &self.surface_host {
            Some(SurfaceHost::Window(w)) => Some(w),
            _ => None,
        }
    }

    pub(crate) fn current_size(&self) -> PhysicalSize<u32> {
        self.surface_host
            .as_ref()
            .map(|h| h.current_size())
            .unwrap_or(self.size)
    }

    pub(crate) fn apply_resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width == 0 || new_size.height == 0 {
            return;
        }

        self.size = new_size;
        self.config.width = new_size.width;
        self.config.height = new_size.height;
        if let Some(surface) = &self.surface {
            surface.configure(&self.device, &self.config);
        }
        let (depth_texture, depth_view) = Self::create_depth_texture(&self.device, &self.config);
        self.depth_texture = depth_texture;
        self.depth_view = depth_view;
    }

    pub(crate) fn create_depth_texture(
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let size = wgpu::Extent3d {
            width: config.width.max(1),
            height: config.height.max(1),
            depth_or_array_layers: 1,
        };
        let desc = wgpu::TextureDescriptor {
            label: Some("Depth Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        let texture = device.create_texture(&desc);
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        (texture, view)
    }
}

pub(crate) struct RenderSubsystem {
    pub(crate) gpu: GpuContext,
    pub(crate) meshes: SceneMeshes,
}

#[cfg(test)]
mod tests {
    use crate::state::State;
    use crate::types::PhysicalSize;

    #[test]
    fn test_apply_resize_zero_size() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            // Save old size and config
            let old_size = state.render.gpu.size;
            let old_config_width = state.render.gpu.config.width;
            let old_config_height = state.render.gpu.config.height;

            // Apply 0 size width
            state.render.gpu.apply_resize(PhysicalSize {
                width: 0,
                height: 100,
            });

            // Assert it returned early and didn't change size
            assert_eq!(state.render.gpu.size.width, old_size.width);
            assert_eq!(state.render.gpu.config.width, old_config_width);

            // Apply 0 size height
            state.render.gpu.apply_resize(PhysicalSize {
                width: 100,
                height: 0,
            });

            // Assert it returned early and didn't change size
            assert_eq!(state.render.gpu.size.height, old_size.height);
            assert_eq!(state.render.gpu.config.height, old_config_height);

            // Apply non-zero resize
            state.render.gpu.apply_resize(PhysicalSize {
                width: 800,
                height: 600,
            });

            assert_eq!(state.render.gpu.size.width, 800);
            assert_eq!(state.render.gpu.config.width, 800);
        });
    }
}
