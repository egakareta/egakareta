/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
pub(crate) mod block_icon;
pub(crate) mod draw;

use crate::mesh::MeshGeometry;
use crate::platform::state_host::SurfaceHost;
use crate::types::{PhysicalSize, Vertex};
use wgpu::util::DeviceExt;

use super::State;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EditorOutlineInstance {
    pub(crate) mask_vertices: std::ops::Range<u32>,
    pub(crate) outline_vertices: std::ops::Range<u32>,
}

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

pub(crate) const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24PlusStencil8;

pub(crate) enum MeshSlot {
    Empty,
    VertexData {
        buffer: wgpu::Buffer,
        count: u32,
    },
    IndexedData {
        vertex_buffer: wgpu::Buffer,
        index_buffer: wgpu::Buffer,
        count: u32,
    },
    IndexedStreaming {
        vertex_buffer: wgpu::Buffer,
        index_buffer: wgpu::Buffer,
        vertex_count: u32,
        index_count: u32,
        capacity_vertices: u32,
        capacity_indices: u32,
    },
    Streaming {
        buffer: wgpu::Buffer,
        count: u32,
        capacity_vertices: u32,
    },
}

#[derive(Clone, Copy)]
pub(crate) enum MeshDrawData<'a> {
    Vertices {
        buffer: &'a wgpu::Buffer,
        count: u32,
    },
    Indexed {
        vertex_buffer: &'a wgpu::Buffer,
        index_buffer: &'a wgpu::Buffer,
        count: u32,
    },
}

impl MeshDrawData<'_> {
    #[cfg(test)]
    pub(crate) fn count(&self) -> u32 {
        match self {
            Self::Vertices { count, .. } | Self::Indexed { count, .. } => *count,
        }
    }
}

impl MeshSlot {
    #[cfg(test)]
    fn destroy(&mut self) {
        let slot = std::mem::replace(self, Self::Empty);
        match slot {
            Self::Empty => {}
            Self::VertexData { buffer, .. } | Self::Streaming { buffer, .. } => {
                buffer.destroy();
            }
            Self::IndexedData {
                vertex_buffer,
                index_buffer,
                ..
            }
            | Self::IndexedStreaming {
                vertex_buffer,
                index_buffer,
                ..
            } => {
                vertex_buffer.destroy();
                index_buffer.destroy();
            }
        }
    }

    pub(crate) fn from_vertices(
        device: &wgpu::Device,
        label: &'static str,
        vertices: &[Vertex],
    ) -> Self {
        puffin::profile_scope!("MeshSlotFromVertices");
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

    pub(crate) fn from_geometry(
        device: &wgpu::Device,
        label: &'static str,
        geometry: &MeshGeometry,
    ) -> Self {
        puffin::profile_scope!("MeshSlotFromGeometry");
        if geometry.is_empty() {
            return Self::Empty;
        }

        let Some(indices) = geometry.indices.as_ref() else {
            return Self::from_vertices(device, label, &geometry.vertices);
        };

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: bytemuck::cast_slice(&geometry.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self::IndexedData {
            vertex_buffer,
            index_buffer,
            count: indices.len() as u32,
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

    fn indexed_streaming(
        device: &wgpu::Device,
        label: &'static str,
        capacity_vertices: u32,
        capacity_indices: u32,
    ) -> Self {
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: (std::mem::size_of::<Vertex>() * capacity_vertices as usize) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: (std::mem::size_of::<u32>() * capacity_indices as usize) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self::IndexedStreaming {
            vertex_buffer,
            index_buffer,
            vertex_count: 0,
            index_count: 0,
            capacity_vertices,
            capacity_indices,
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

    pub(crate) fn replace_with_geometry(
        &mut self,
        device: &wgpu::Device,
        label: &'static str,
        geometry: &MeshGeometry,
    ) {
        *self = Self::from_geometry(device, label, geometry);
    }

    pub(crate) fn replace_with_streaming_geometry(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &'static str,
        geometry: &MeshGeometry,
        spare_capacity_vertices: u32,
        spare_capacity_indices: u32,
    ) {
        puffin::profile_scope!("MeshSlotStreamingReplace");
        if geometry.is_empty() {
            *self = Self::Empty;
            return;
        }

        let capacity_vertices = geometry
            .vertex_count()
            .saturating_add(spare_capacity_vertices as usize)
            .max(geometry.vertex_count()) as u32;
        if let Some(indices) = geometry.indices.as_ref() {
            let capacity_indices = indices
                .len()
                .saturating_add(spare_capacity_indices as usize)
                .max(indices.len()) as u32;
            *self = Self::indexed_streaming(device, label, capacity_vertices, capacity_indices);
        } else {
            *self = Self::streaming(device, label, capacity_vertices);
        }

        if !self.append_streaming_geometry(queue, 0, 0, geometry) {
            *self = Self::from_geometry(device, label, geometry);
        }
    }

    pub(crate) fn append_streaming_geometry(
        &mut self,
        queue: &wgpu::Queue,
        start_vertex: usize,
        start_index: usize,
        geometry: &MeshGeometry,
    ) -> bool {
        puffin::profile_scope!("MeshSlotStreamingAppend");
        match self {
            Self::Streaming {
                buffer,
                count,
                capacity_vertices,
            } => {
                if geometry.indices.is_some()
                    || *count as usize != start_vertex
                    || start_vertex.saturating_add(geometry.vertices.len())
                        > *capacity_vertices as usize
                {
                    return false;
                }

                if !geometry.vertices.is_empty() {
                    let offset = (std::mem::size_of::<Vertex>() * start_vertex) as u64;
                    queue.write_buffer(buffer, offset, bytemuck::cast_slice(&geometry.vertices));
                }
                *count = start_vertex.saturating_add(geometry.vertices.len()) as u32;
                true
            }
            Self::IndexedStreaming {
                vertex_buffer,
                index_buffer,
                vertex_count,
                index_count,
                capacity_vertices,
                capacity_indices,
            } => {
                let append_index_count = geometry.draw_count();
                if *vertex_count as usize != start_vertex
                    || *index_count as usize != start_index
                    || start_vertex.saturating_add(geometry.vertices.len())
                        > *capacity_vertices as usize
                    || start_index.saturating_add(append_index_count) > *capacity_indices as usize
                {
                    return false;
                }

                if !geometry.vertices.is_empty() {
                    let vertex_offset = (std::mem::size_of::<Vertex>() * start_vertex) as u64;
                    queue.write_buffer(
                        vertex_buffer,
                        vertex_offset,
                        bytemuck::cast_slice(&geometry.vertices),
                    );
                }

                let appended_indices = if let Some(indices) = geometry.indices.as_ref() {
                    indices
                        .iter()
                        .map(|index| start_vertex as u32 + *index)
                        .collect::<Vec<_>>()
                } else {
                    let end_vertex = start_vertex.saturating_add(geometry.vertices.len()) as u32;
                    (start_vertex as u32..end_vertex).collect::<Vec<_>>()
                };
                if !appended_indices.is_empty() {
                    let index_offset = (std::mem::size_of::<u32>() * start_index) as u64;
                    queue.write_buffer(
                        index_buffer,
                        index_offset,
                        bytemuck::cast_slice(&appended_indices),
                    );
                }

                *vertex_count = start_vertex.saturating_add(geometry.vertices.len()) as u32;
                *index_count = start_index.saturating_add(appended_indices.len()) as u32;
                true
            }
            _ => false,
        }
    }

    pub(crate) fn write_streaming_vertices(&mut self, queue: &wgpu::Queue, vertices: &[Vertex]) {
        puffin::profile_scope!("MeshSlotStreamingWriteVertices");
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

    pub(crate) fn clear(&mut self) {
        match self {
            Self::Empty => {}
            Self::VertexData { .. } | Self::IndexedData { .. } => *self = Self::Empty,
            Self::Streaming { count, .. } => *count = 0,
            Self::IndexedStreaming {
                vertex_count,
                index_count,
                ..
            } => {
                *vertex_count = 0;
                *index_count = 0;
            }
        }
    }

    pub(crate) fn draw_data(&self) -> Option<MeshDrawData<'_>> {
        match self {
            Self::Empty => None,
            Self::VertexData { buffer, count } | Self::Streaming { buffer, count, .. } => {
                (*count > 0).then_some(MeshDrawData::Vertices {
                    buffer,
                    count: *count,
                })
            }
            Self::IndexedData {
                vertex_buffer,
                index_buffer,
                count,
            } => (*count > 0).then_some(MeshDrawData::Indexed {
                vertex_buffer,
                index_buffer,
                count: *count,
            }),
            Self::IndexedStreaming {
                vertex_buffer,
                index_buffer,
                index_count,
                ..
            } => (*index_count > 0).then_some(MeshDrawData::Indexed {
                vertex_buffer,
                index_buffer,
                count: *index_count,
            }),
        }
    }
}

pub(crate) struct SceneMeshes {
    pub(crate) floor: MeshSlot,
    pub(crate) grid: MeshSlot,
    pub(crate) trail: MeshSlot,
    pub(crate) blocks: MeshSlot,
    pub(crate) blocks_static: MeshSlot,
    pub(crate) blocks_selected: MeshSlot,
    pub(crate) editor_cursor: MeshSlot,
    pub(crate) editor_hover_stencil: MeshSlot,
    pub(crate) editor_hover_outline: MeshSlot,
    pub(crate) editor_selection_stencil: MeshSlot,
    pub(crate) editor_selection_outline: MeshSlot,
    pub(crate) editor_selection_outline_instances: Vec<EditorOutlineInstance>,
    pub(crate) editor_gizmo: MeshSlot,
    pub(crate) tap_indicators: MeshSlot,
    pub(crate) spawn_marker: MeshSlot,
    pub(crate) camera_trigger_markers: MeshSlot,
    pub(crate) editor_preview_player: MeshSlot,
}

#[cfg(test)]
impl SceneMeshes {
    fn destroy(&mut self) {
        self.floor.destroy();
        self.grid.destroy();
        self.trail.destroy();
        self.blocks.destroy();
        self.blocks_static.destroy();
        self.blocks_selected.destroy();
        self.editor_cursor.destroy();
        self.editor_hover_stencil.destroy();
        self.editor_hover_outline.destroy();
        self.editor_selection_stencil.destroy();
        self.editor_selection_outline.destroy();
        self.editor_gizmo.destroy();
        self.tap_indicators.destroy();
        self.spawn_marker.destroy();
        self.camera_trigger_markers.destroy();
        self.editor_preview_player.destroy();
    }
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
    pub(crate) editor_outline_occlusion_depth_texture: wgpu::Texture,
    pub(crate) editor_outline_occlusion_depth_view: wgpu::TextureView,
    pub(crate) render_pipeline: wgpu::RenderPipeline,
    pub(crate) block_icon_pipeline: wgpu::RenderPipeline,
    pub(crate) editor_ghost_trail_pipeline: wgpu::RenderPipeline,
    pub(crate) gizmo_overlay_pipeline: wgpu::RenderPipeline,
    pub(crate) editor_outline_occlusion_depth_pipeline: wgpu::RenderPipeline,
    pub(crate) editor_outline_mask_pipeline: wgpu::RenderPipeline,
    pub(crate) editor_outline_pipeline: wgpu::RenderPipeline,
    pub(crate) editor_hover_outline_pipeline: wgpu::RenderPipeline,
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
        let (editor_outline_occlusion_depth_texture, editor_outline_occlusion_depth_view) =
            Self::create_depth_texture(&self.device, &self.config);
        self.editor_outline_occlusion_depth_texture = editor_outline_occlusion_depth_texture;
        self.editor_outline_occlusion_depth_view = editor_outline_occlusion_depth_view;
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
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
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
impl Drop for RenderSubsystem {
    fn drop(&mut self) {
        self.meshes.destroy();
        self.gpu.line_uniform_buffer.destroy();
        self.gpu.camera_uniform_buffer.destroy();
        self.gpu.color_space_uniform_buffer.destroy();
        let _ = self.gpu.device.poll(wgpu::PollType::Poll);
    }
}

#[cfg(test)]
mod tests {
    use crate::mesh::MeshGeometry;
    use crate::state::render::{MeshDrawData, MeshSlot};
    use crate::state::State;
    use crate::types::{PhysicalSize, Vertex};

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn mesh_slot_draw_data_reports_indexed_geometry() {
        pollster::block_on(async {
            let state = State::new_test().await;
            let geometry = MeshGeometry {
                vertices: vec![
                    Vertex::untextured([0.0, 0.0, 0.0], [1.0, 1.0, 1.0, 1.0]),
                    Vertex::untextured([1.0, 0.0, 0.0], [1.0, 1.0, 1.0, 1.0]),
                    Vertex::untextured([0.0, 1.0, 0.0], [1.0, 1.0, 1.0, 1.0]),
                ],
                indices: Some(vec![0, 1, 2]),
            };

            let slot = MeshSlot::from_geometry(state.device(), "Indexed Test Mesh", &geometry);
            let draw_data = slot.draw_data().expect("indexed geometry should draw");
            assert_eq!(draw_data.count(), 3);
            assert!(matches!(draw_data, MeshDrawData::Indexed { .. }));
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn streaming_geometry_appends_unindexed_vertices_without_rebuild() {
        pollster::block_on(async {
            let state = State::new_test().await;
            let first = MeshGeometry::from_vertices(vec![
                Vertex::untextured([0.0, 0.0, 0.0], [1.0, 1.0, 1.0, 1.0]),
                Vertex::untextured([1.0, 0.0, 0.0], [1.0, 1.0, 1.0, 1.0]),
            ]);
            let second = MeshGeometry::from_vertices(vec![Vertex::untextured(
                [2.0, 0.0, 0.0],
                [1.0, 1.0, 1.0, 1.0],
            )]);
            let mut slot = MeshSlot::Empty;

            slot.replace_with_streaming_geometry(
                state.device(),
                &state.render.gpu.queue,
                "Streaming Test Mesh",
                &first,
                4,
                4,
            );

            assert!(matches!(slot, MeshSlot::Streaming { .. }));
            assert!(slot.append_streaming_geometry(&state.render.gpu.queue, 2, 2, &second));
            assert_eq!(slot.draw_data().map(|draw_data| draw_data.count()), Some(3));
        });
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn streaming_geometry_appends_indexed_data_without_rebuild() {
        pollster::block_on(async {
            let state = State::new_test().await;
            let first = MeshGeometry {
                vertices: vec![
                    Vertex::untextured([0.0, 0.0, 0.0], [1.0, 1.0, 1.0, 1.0]),
                    Vertex::untextured([1.0, 0.0, 0.0], [1.0, 1.0, 1.0, 1.0]),
                    Vertex::untextured([0.0, 1.0, 0.0], [1.0, 1.0, 1.0, 1.0]),
                ],
                indices: Some(vec![0, 1, 2]),
            };
            let second = MeshGeometry::from_vertices(vec![Vertex::untextured(
                [2.0, 0.0, 0.0],
                [1.0, 1.0, 1.0, 1.0],
            )]);
            let mut slot = MeshSlot::Empty;

            slot.replace_with_streaming_geometry(
                state.device(),
                &state.render.gpu.queue,
                "Indexed Streaming Test Mesh",
                &first,
                4,
                4,
            );

            assert!(matches!(slot, MeshSlot::IndexedStreaming { .. }));
            assert!(slot.append_streaming_geometry(&state.render.gpu.queue, 3, 3, &second));
            assert_eq!(slot.draw_data().map(|draw_data| draw_data.count()), Some(4));
            assert!(matches!(
                slot.draw_data(),
                Some(MeshDrawData::Indexed { .. })
            ));
        });
    }

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
