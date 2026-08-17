/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Render resource primitives shared by the state integration layer.

pub mod block_icon;
pub mod block_icon_cache;

#[cfg(test)]
mod mesh_slot_tests;
#[cfg(test)]
mod shader_tests;

use eg_mesh::{ChunkAabb, ChunkedBlockMesh, MeshGeometry, RenderChunkKey, Vertex};
use eg_types::{AppPhase, EditorMode, GridUniform, PhysicalSize};
use wgpu::util::DeviceExt;

pub const SHADER_WGSL: &str = include_str!("shader.wgsl");
pub const GRID_WGSL: &str = include_str!("grid.wgsl");

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderFrameInput {
    pub phase: AppPhase,
    pub editor_mode: EditorMode,
    pub game_over: bool,
    pub sky_color: [f32; 3],
    pub grid_center: [f32; 2],
    pub apply_gamma_correction: bool,
}

impl RenderFrameInput {
    pub fn clear_color(self) -> wgpu::Color {
        apply_gamma_correction_if_enabled(
            clear_color_for_phase(self.phase, self.game_over, self.sky_color),
            self.apply_gamma_correction,
        )
    }

    pub fn grid_uniform(self, half_extent: f32) -> GridUniform {
        GridUniform {
            center: self.grid_center,
            half_extent,
            darkening: editor_grid_darkening(self.phase, self.sky_color),
        }
    }

    pub fn should_draw_floor_and_grid(self) -> bool {
        should_draw_floor_and_grid(self.phase, self.editor_mode)
    }

    pub fn should_skip_world(self) -> bool {
        should_skip_world(self.phase, self.editor_mode)
    }

    pub fn should_draw_editor_overlays(self) -> bool {
        should_draw_editor_overlays(self.phase, self.should_skip_world())
    }

    pub fn should_draw_editor_cursor(self) -> bool {
        should_draw_editor_cursor(self.editor_mode)
    }
}

pub fn linear_to_srgb(linear: f32) -> f32 {
    if linear <= 0.0031308 {
        linear * 12.92
    } else {
        linear.powf(1.0 / 2.4) * 1.055 - 0.055
    }
}

pub fn color_from_rgb(rgb: [f32; 3]) -> wgpu::Color {
    wgpu::Color {
        r: rgb[0].clamp(0.0, 1.0) as f64,
        g: rgb[1].clamp(0.0, 1.0) as f64,
        b: rgb[2].clamp(0.0, 1.0) as f64,
        a: 1.0,
    }
}

pub fn clear_color_for_phase(phase: AppPhase, game_over: bool, sky_color: [f32; 3]) -> wgpu::Color {
    match phase {
        AppPhase::Playing if game_over => wgpu::Color {
            r: 0.15,
            g: 0.05,
            b: 0.05,
            a: 1.0,
        },
        AppPhase::Menu | AppPhase::Editor | AppPhase::Playing => color_from_rgb(sky_color),
        _ => wgpu::Color {
            r: 0.05,
            g: 0.05,
            b: 0.08,
            a: 1.0,
        },
    }
}

pub fn apply_gamma_correction_if_enabled(
    color: wgpu::Color,
    apply_gamma_correction: bool,
) -> wgpu::Color {
    if apply_gamma_correction {
        wgpu::Color {
            r: linear_to_srgb(color.r as f32) as f64,
            g: linear_to_srgb(color.g as f32) as f64,
            b: linear_to_srgb(color.b as f32) as f64,
            a: color.a,
        }
    } else {
        color
    }
}

pub fn should_draw_floor_and_grid(phase: AppPhase, mode: EditorMode) -> bool {
    phase == AppPhase::Editor && mode != EditorMode::Timing
}

pub fn smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn editor_grid_darkening(phase: AppPhase, sky_color: [f32; 3]) -> f32 {
    if phase != AppPhase::Editor {
        return 0.0;
    }

    let luminance = sky_color[0].clamp(0.0, 1.0) * 0.2126
        + sky_color[1].clamp(0.0, 1.0) * 0.7152
        + sky_color[2].clamp(0.0, 1.0) * 0.0722;
    0.10 + 0.35 * smoothstep(0.35, 0.80, luminance)
}

pub fn should_skip_world(phase: AppPhase, mode: EditorMode) -> bool {
    phase == AppPhase::Editor && mode == EditorMode::Timing
}

pub fn should_draw_editor_overlays(phase: AppPhase, skip_world: bool) -> bool {
    phase == AppPhase::Editor && !skip_world
}

pub fn should_draw_editor_cursor(mode: EditorMode) -> bool {
    mode == EditorMode::Place || mode == EditorMode::Tapping
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderSurfaceError {
    Outdated,
    Lost,
    Validation,
}

pub fn current_surface_output(
    texture: wgpu::CurrentSurfaceTexture,
) -> Result<Option<wgpu::SurfaceTexture>, RenderSurfaceError> {
    match texture {
        wgpu::CurrentSurfaceTexture::Success(output)
        | wgpu::CurrentSurfaceTexture::Suboptimal(output) => Ok(Some(output)),
        wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => Ok(None),
        wgpu::CurrentSurfaceTexture::Outdated => Err(RenderSurfaceError::Outdated),
        wgpu::CurrentSurfaceTexture::Lost => Err(RenderSurfaceError::Lost),
        wgpu::CurrentSurfaceTexture::Validation => Err(RenderSurfaceError::Validation),
    }
}

pub fn draw_mesh(render_pass: &mut wgpu::RenderPass<'_>, draw_data: MeshDrawData<'_>) {
    match draw_data {
        MeshDrawData::Vertices { buffer, count } => {
            render_pass.set_vertex_buffer(0, buffer.slice(..));
            render_pass.draw(0..count, 0..1);
        }
        MeshDrawData::Indexed {
            vertex_buffer,
            index_buffer,
            count,
        } => {
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..count, 0, 0..1);
        }
    }
}

/// Draw every batch in an [`InstancedMeshSlot`], binding template geometry on
/// vertex buffer slot 0 and per-instance data on slot 1.
pub fn draw_instanced_blocks(render_pass: &mut wgpu::RenderPass<'_>, slot: &InstancedMeshSlot) {
    for batch in slot.draw_batches() {
        render_pass.set_vertex_buffer(0, batch.template_buffer.slice(..));
        render_pass.set_vertex_buffer(1, batch.instance_buffer.slice(..));
        render_pass.draw(0..batch.template_vertex_count, 0..batch.instance_count);
    }
}

pub fn draw_chunk_instanced_blocks(
    render_pass: &mut wgpu::RenderPass<'_>,
    chunk: &RenderedBlockChunk,
) {
    draw_instanced_blocks(render_pass, &chunk.instanced);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorOutlineInstance {
    pub mask_vertices: std::ops::Range<u32>,
    pub outline_vertices: std::ops::Range<u32>,
}

pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth24PlusStencil8;

#[derive(Default)]
pub enum MeshSlot {
    #[default]
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
pub enum MeshDrawData<'a> {
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
    pub fn count(&self) -> u32 {
        match self {
            Self::Vertices { count, .. } | Self::Indexed { count, .. } => *count,
        }
    }
}

impl MeshSlot {
    pub fn destroy(&mut self) {
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

    pub fn from_vertices(device: &wgpu::Device, label: &'static str, vertices: &[Vertex]) -> Self {
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

    pub fn from_geometry(
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

    pub fn streaming(device: &wgpu::Device, label: &'static str, capacity_vertices: u32) -> Self {
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

    pub fn replace_with_vertices(
        &mut self,
        device: &wgpu::Device,
        label: &'static str,
        vertices: &[Vertex],
    ) {
        *self = Self::from_vertices(device, label, vertices);
    }

    pub fn replace_with_geometry(
        &mut self,
        device: &wgpu::Device,
        label: &'static str,
        geometry: &MeshGeometry,
    ) {
        *self = Self::from_geometry(device, label, geometry);
    }

    pub fn replace_with_streaming_geometry(
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

    /// Replace dynamic vertex data while retaining a reusable GPU buffer.
    ///
    /// This is for presentation meshes that change often (trails, gizmos,
    /// overlays). It grows capacity only when needed and otherwise updates the
    /// existing buffer in place, avoiding one GPU allocation per frame.
    pub fn replace_with_streaming_vertices(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &'static str,
        vertices: &[Vertex],
    ) {
        let capacity = match self {
            Self::Streaming {
                capacity_vertices, ..
            } => *capacity_vertices as usize,
            _ => 0,
        };
        if vertices.len() > capacity {
            let grown_capacity = vertices
                .len()
                .saturating_mul(2)
                .max(1)
                .min(u32::MAX as usize) as u32;
            *self = Self::streaming(device, label, grown_capacity);
        }
        self.write_streaming_vertices(queue, vertices);
    }

    pub fn append_streaming_geometry(
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

    pub fn write_streaming_vertices(&mut self, queue: &wgpu::Queue, vertices: &[Vertex]) {
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

    pub fn clear(&mut self) {
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

    /// Return the vertex buffer used by unindexed meshes, including dynamic
    /// streaming meshes. Custom passes that issue their own vertex ranges
    /// (such as editor selection silhouettes) use this instead of draw_data.
    pub fn vertex_buffer(&self) -> Option<&wgpu::Buffer> {
        match self {
            Self::VertexData { buffer, .. } | Self::Streaming { buffer, .. } => Some(buffer),
            Self::Empty | Self::IndexedData { .. } | Self::IndexedStreaming { .. } => None,
        }
    }

    pub fn draw_data(&self) -> Option<MeshDrawData<'_>> {
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

/// A single instanced batch on the GPU: one template vertex buffer shared by
/// `instance_count` per-instance records.
pub struct InstancedBatch {
    pub block_id: String,
    pub template_buffer: wgpu::Buffer,
    pub template_vertex_count: u32,
    pub instance_buffer: wgpu::Buffer,
    pub instance_count: u32,
}

pub struct RenderedBlockChunk {
    pub key: RenderChunkKey,
    pub aabb: ChunkAabb,
    pub greedy: MeshSlot,
    pub fallback: MeshSlot,
    pub instanced: InstancedMeshSlot,
}

impl RenderedBlockChunk {
    fn destroy(&mut self) {
        self.greedy.destroy();
        self.fallback.destroy();
        self.instanced.destroy();
    }

    pub fn has_draws(&self) -> bool {
        self.greedy.draw_data().is_some()
            || self.fallback.draw_data().is_some()
            || !self.instanced.is_empty()
    }
}

#[derive(Default)]
pub struct ChunkedBlockMeshSlot {
    chunks: Vec<RenderedBlockChunk>,
}

impl ChunkedBlockMeshSlot {
    pub fn destroy(&mut self) {
        for chunk in &mut self.chunks {
            chunk.destroy();
        }
        self.chunks.clear();
    }

    pub fn clear(&mut self) {
        self.destroy();
    }

    pub fn is_empty(&self) -> bool {
        self.chunks.iter().all(|chunk| !chunk.has_draws())
    }

    pub fn chunk_count(&self) -> usize {
        self.chunks.len()
    }

    pub fn chunks(&self) -> impl Iterator<Item = &RenderedBlockChunk> {
        self.chunks.iter().filter(|chunk| chunk.has_draws())
    }

    pub fn replace_with_chunked_mesh(
        &mut self,
        device: &wgpu::Device,
        label: &'static str,
        mesh: &ChunkedBlockMesh,
    ) {
        puffin::profile_scope!("ChunkedBlockMeshSlotReplace");
        self.destroy();
        self.chunks.reserve(mesh.chunks.len());
        for chunk in &mesh.chunks {
            let mut instanced = InstancedMeshSlot::default();
            instanced.replace_with_batches(device, label, &chunk.fallback);
            self.chunks.push(RenderedBlockChunk {
                key: chunk.key,
                aabb: chunk.aabb,
                greedy: MeshSlot::from_geometry(device, label, &chunk.greedy_geometry),
                fallback: MeshSlot::from_geometry(device, label, &chunk.fallback.fallback_geometry),
                instanced,
            });
        }
    }
}

/// GPU-side storage for instanced block rendering. Holds one [`InstancedBatch`]
/// per block template. Replaces a single giant expanded mesh with compact
/// per-object instance records, keeping memory bounded for large levels.
#[derive(Default)]
pub struct InstancedMeshSlot {
    batches: Vec<InstancedBatch>,
}

impl InstancedMeshSlot {
    pub fn destroy(&mut self) {
        for batch in self.batches.drain(..) {
            batch.template_buffer.destroy();
            batch.instance_buffer.destroy();
        }
    }

    pub fn clear(&mut self) {
        self.destroy();
    }

    pub fn is_empty(&self) -> bool {
        self.batches.iter().all(|batch| batch.instance_count == 0)
    }

    pub fn instance_count(&self) -> u32 {
        self.batches.iter().map(|batch| batch.instance_count).sum()
    }

    pub fn batch_count(&self) -> usize {
        self.batches.len()
    }

    pub fn draw_batches(&self) -> impl Iterator<Item = &InstancedBatch> {
        self.batches
            .iter()
            .filter(|batch| batch.instance_count > 0 && batch.template_vertex_count > 0)
    }

    /// Rebuild all GPU batches from a freshly computed instance build result.
    pub fn replace_with_batches(
        &mut self,
        device: &wgpu::Device,
        label: &'static str,
        result: &eg_mesh::BlockInstanceBuildResult,
    ) {
        puffin::profile_scope!("InstancedMeshSlotReplace");
        self.destroy();
        self.batches.reserve(result.batches.len());
        for batch in &result.batches {
            if batch.instances.is_empty() || batch.template.vertices.is_empty() {
                continue;
            }
            let template_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(&batch.template.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let instance_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(&batch.instances),
                usage: wgpu::BufferUsages::VERTEX,
            });
            self.batches.push(InstancedBatch {
                block_id: batch.block_id.clone(),
                template_buffer,
                template_vertex_count: batch.template.vertices.len() as u32,
                instance_buffer,
                instance_count: batch.instances.len() as u32,
            });
        }
    }
}

pub struct SceneMeshes {
    pub floor: MeshSlot,
    pub grid: MeshSlot,
    pub trail: MeshSlot,
    pub gem_shatter_effects: MeshSlot,
    pub practice_checkpoints: MeshSlot,
    pub blocks: MeshSlot,
    pub blocks_chunked: ChunkedBlockMeshSlot,
    pub blocks_instanced: InstancedMeshSlot,
    pub blocks_static: MeshSlot,
    pub blocks_static_chunked: ChunkedBlockMeshSlot,
    pub blocks_static_instanced: InstancedMeshSlot,
    pub blocks_selected: MeshSlot,
    pub editor_cursor: MeshSlot,
    pub editor_hover_stencil: MeshSlot,
    pub editor_hover_outline: MeshSlot,
    pub editor_selection_stencil: MeshSlot,
    pub editor_selection_outline: MeshSlot,
    pub editor_selection_outline_instances: Vec<EditorOutlineInstance>,
    pub editor_gizmo: MeshSlot,
    pub tap_indicators: MeshSlot,
    pub transform_trigger_markers: MeshSlot,
    pub spawn_marker: MeshSlot,
    pub camera_trigger_markers: MeshSlot,
    pub editor_preview_player: MeshSlot,
    pub editor_hitbox_visualization: MeshSlot,
    pub camera_arrow: MeshSlot,
}

impl Default for SceneMeshes {
    fn default() -> Self {
        Self {
            floor: MeshSlot::Empty,
            grid: MeshSlot::Empty,
            trail: MeshSlot::Empty,
            gem_shatter_effects: MeshSlot::Empty,
            practice_checkpoints: MeshSlot::Empty,
            blocks: MeshSlot::Empty,
            blocks_chunked: ChunkedBlockMeshSlot::default(),
            blocks_instanced: InstancedMeshSlot::default(),
            blocks_static: MeshSlot::Empty,
            blocks_static_chunked: ChunkedBlockMeshSlot::default(),
            blocks_static_instanced: InstancedMeshSlot::default(),
            blocks_selected: MeshSlot::Empty,
            editor_cursor: MeshSlot::Empty,
            editor_hover_stencil: MeshSlot::Empty,
            editor_hover_outline: MeshSlot::Empty,
            editor_selection_stencil: MeshSlot::Empty,
            editor_selection_outline: MeshSlot::Empty,
            editor_selection_outline_instances: Vec::new(),
            editor_gizmo: MeshSlot::Empty,
            tap_indicators: MeshSlot::Empty,
            transform_trigger_markers: MeshSlot::Empty,
            spawn_marker: MeshSlot::Empty,
            camera_trigger_markers: MeshSlot::Empty,
            editor_preview_player: MeshSlot::Empty,
            editor_hitbox_visualization: MeshSlot::Empty,
            camera_arrow: MeshSlot::Empty,
        }
    }
}

impl SceneMeshes {
    pub fn destroy(&mut self) {
        self.floor.destroy();
        self.grid.destroy();
        self.trail.destroy();
        self.gem_shatter_effects.destroy();
        self.practice_checkpoints.destroy();
        self.blocks.destroy();
        self.blocks_chunked.destroy();
        self.blocks_instanced.destroy();
        self.blocks_static.destroy();
        self.blocks_static_chunked.destroy();
        self.blocks_static_instanced.destroy();
        self.blocks_selected.destroy();
        self.editor_cursor.destroy();
        self.editor_hover_stencil.destroy();
        self.editor_hover_outline.destroy();
        self.editor_selection_stencil.destroy();
        self.editor_selection_outline.destroy();
        self.editor_gizmo.destroy();
        self.tap_indicators.destroy();
        self.transform_trigger_markers.destroy();
        self.spawn_marker.destroy();
        self.camera_trigger_markers.destroy();
        self.editor_preview_player.destroy();
        self.editor_hitbox_visualization.destroy();
        self.camera_arrow.destroy();
    }
}

pub struct GpuContext {
    pub surface: Option<wgpu::Surface<'static>>,
    pub adapter_info: wgpu::AdapterInfo,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub size: PhysicalSize<u32>,
    pub depth_texture: wgpu::Texture,
    pub depth_view: wgpu::TextureView,
    pub editor_outline_occlusion_depth_texture: wgpu::Texture,
    pub editor_outline_occlusion_depth_view: wgpu::TextureView,
    pub render_pipeline: wgpu::RenderPipeline,
    pub instanced_block_pipeline: wgpu::RenderPipeline,
    pub grid_pipeline: wgpu::RenderPipeline,
    pub block_icon_pipeline: wgpu::RenderPipeline,
    pub editor_ghost_trail_pipeline: wgpu::RenderPipeline,
    pub gizmo_overlay_pipeline: wgpu::RenderPipeline,
    pub editor_outline_occlusion_depth_pipeline: wgpu::RenderPipeline,
    pub editor_outline_mask_pipeline: wgpu::RenderPipeline,
    pub editor_outline_pipeline: wgpu::RenderPipeline,
    pub editor_hover_outline_pipeline: wgpu::RenderPipeline,
    #[cfg_attr(not(test), allow(dead_code))]
    pub line_bind_group_layout: wgpu::BindGroupLayout,
    pub line_uniform_buffer: wgpu::Buffer,
    pub zero_line_bind_group: wgpu::BindGroup,
    pub camera_uniform_buffer: wgpu::Buffer,
    pub camera_bind_group_layout: wgpu::BindGroupLayout,
    pub camera_bind_group: wgpu::BindGroup,
    #[cfg_attr(not(test), allow(dead_code))]
    pub color_space_bind_group_layout: wgpu::BindGroupLayout,
    pub color_space_uniform_buffer: wgpu::Buffer,
    pub color_space_bind_group: wgpu::BindGroup,
    #[cfg_attr(not(test), allow(dead_code))]
    pub grid_bind_group_layout: wgpu::BindGroupLayout,
    pub grid_uniform_buffer: wgpu::Buffer,
    pub grid_bind_group: wgpu::BindGroup,
    pub block_texture_bind_group: wgpu::BindGroup,
    pub apply_gamma_correction: bool,
}

impl GpuContext {
    pub fn surface_width(&self) -> u32 {
        self.config.width
    }

    pub fn surface_height(&self) -> u32 {
        self.config.height
    }

    pub fn apply_resize(&mut self, new_size: PhysicalSize<u32>) {
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

    pub fn create_depth_texture(
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

#[cfg(test)]
mod tests {
    use eg_types::{AppPhase, EditorMode};

    use super::{RenderFrameInput, linear_to_srgb};

    #[test]
    fn render_frame_input_owns_frame_clear_grid_and_gate_policy() {
        let input = RenderFrameInput {
            phase: AppPhase::Editor,
            editor_mode: EditorMode::Place,
            game_over: false,
            sky_color: [0.18, 0.4, 0.8],
            grid_center: [12.0, -4.0],
            apply_gamma_correction: true,
        };

        assert!(input.should_draw_floor_and_grid());
        assert!(!input.should_skip_world());
        assert!(input.should_draw_editor_overlays());
        assert!(input.should_draw_editor_cursor());

        let clear = input.clear_color();
        approx::assert_abs_diff_eq!(clear.r, linear_to_srgb(0.18) as f64, epsilon = 1e-7);
        approx::assert_abs_diff_eq!(clear.g, linear_to_srgb(0.4) as f64, epsilon = 1e-7);
        approx::assert_abs_diff_eq!(clear.b, linear_to_srgb(0.8) as f64, epsilon = 1e-7);
        approx::assert_abs_diff_eq!(clear.a, 1.0, epsilon = 1e-7);

        let grid = input.grid_uniform(2048.0);
        assert_eq!(grid.center, [12.0, -4.0]);
        assert_eq!(grid.half_extent, 2048.0);
        assert!(grid.darkening > 0.0);
    }

    #[test]
    fn render_frame_input_hides_world_for_editor_timing_mode() {
        let input = RenderFrameInput {
            phase: AppPhase::Editor,
            editor_mode: EditorMode::Timing,
            game_over: false,
            sky_color: [0.0, 0.0, 0.0],
            grid_center: [0.0, 0.0],
            apply_gamma_correction: false,
        };

        assert!(!input.should_draw_floor_and_grid());
        assert!(input.should_skip_world());
        assert!(!input.should_draw_editor_overlays());
        assert!(!input.should_draw_editor_cursor());
    }

    #[test]
    fn render_frame_input_hides_grid_outside_editor() {
        let input = RenderFrameInput {
            phase: AppPhase::Playing,
            editor_mode: EditorMode::Place,
            game_over: false,
            sky_color: [0.25, 0.5, 0.75],
            grid_center: [0.0, 0.0],
            apply_gamma_correction: false,
        };

        assert!(!input.should_draw_floor_and_grid());
        assert!(!input.should_skip_world());
        assert!(!input.should_draw_editor_overlays());
    }
}
