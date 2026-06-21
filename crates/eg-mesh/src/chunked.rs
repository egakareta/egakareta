/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::blocks::{
    build_block_instance_batches_from_refs, build_block_instance_batches_from_refs_at_time,
    BlockInstanceBuildResult,
};
use crate::geometry::MeshGeometry;
use crate::shapes::{PrismFaceColors, PrismTextureLayers};
use crate::Vertex;
use eg_blocks::{resolve_block_definition, resolve_block_texture_layers};
use eg_types::{BlockCuboid, BlockDefinition, BlockRenderProfile, LevelObject};

pub const DEFAULT_RENDER_CHUNK_SIZE: i32 = 16;

const GRID_EPSILON: f32 = 1e-4;
const UNIT_QUAD_INDICES: [u32; 6] = [0, 1, 2, 0, 2, 3];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RenderChunkKey {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChunkAabb {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl ChunkAabb {
    pub fn from_min_max(min: [f32; 3], max: [f32; 3]) -> Self {
        Self { min, max }
    }

    fn include_point(&mut self, point: [f32; 3]) {
        for (axis, value) in point.into_iter().enumerate() {
            self.min[axis] = self.min[axis].min(value);
            self.max[axis] = self.max[axis].max(value);
        }
    }

    fn include_object(&mut self, object: &LevelObject) {
        self.include_point(object.position);
        self.include_point([
            object.position[0] + object.size[0],
            object.position[1] + object.size[1],
            object.position[2] + object.size[2],
        ]);
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChunkBuildStats {
    pub voxel_cells: usize,
    pub hidden_faces: usize,
    pub visible_faces_before_greedy: usize,
    pub merged_quads: usize,
    pub fallback_objects: usize,
}

#[derive(Clone)]
pub struct BlockMeshChunk {
    pub key: RenderChunkKey,
    pub aabb: ChunkAabb,
    pub greedy_geometry: MeshGeometry,
    pub fallback: BlockInstanceBuildResult,
    pub stats: ChunkBuildStats,
}

#[derive(Clone, Default)]
pub struct ChunkedBlockMesh {
    pub chunks: Vec<BlockMeshChunk>,
    pub stats: ChunkBuildStats,
}

impl ChunkedBlockMesh {
    pub fn fallback_geometry(&self) -> MeshGeometry {
        let mut geometry = MeshGeometry::default();
        for chunk in &self.chunks {
            geometry.append_geometry(chunk.greedy_geometry.clone());
            geometry.append_geometry(chunk.fallback.fallback_geometry.clone());
        }
        geometry
    }

    pub fn fallback_instances(&self) -> BlockInstanceBuildResult {
        let mut result = BlockInstanceBuildResult::default();
        for chunk in &self.chunks {
            for batch in &chunk.fallback.batches {
                result.batches.push(batch.clone());
            }
            result
                .fallback_geometry
                .append_geometry(chunk.fallback.fallback_geometry.clone());
        }
        result
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VoxelFastLaneRejection {
    Trigger,
    Rotated,
    NonFiniteBounds,
    NonGridAligned,
    EmptyOrNegativeSize,
    CustomGeometry,
    RenderProfile,
    Animated,
    Noisy,
    /// The block has been resized away from a unit cube. Resized blocks are
    /// routed to the instanced/fallback path so their texture stretches across
    /// the single block instead of tiling per voxel cell like placed unit blocks.
    NonUnitSize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct VoxelCell {
    x: i32,
    y: i32,
    z: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct MaterialKey {
    block_id: u32,
    tint: [u32; 3],
}

#[derive(Clone, Copy)]
struct VoxelMaterial {
    colors: PrismFaceColors,
    layers: PrismTextureLayers,
}

#[derive(Clone, Copy)]
struct VoxelColors {
    top: [f32; 4],
    side: [f32; 4],
    bottom: [f32; 4],
    outline: [f32; 4],
}

impl VoxelColors {
    fn apply_tint(&mut self, tint_rgb: [f32; 3]) {
        for color in [&mut self.top, &mut self.side, &mut self.bottom] {
            color[0] *= tint_rgb[0];
            color[1] *= tint_rgb[1];
            color[2] *= tint_rgb[2];
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum FaceDirection {
    Top,
    Bottom,
    PosX,
    NegX,
    PosZ,
    NegZ,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct FaceBucketKey {
    direction: FaceDirection,
    plane: i32,
    material: MaterialKey,
}

#[derive(Clone, Copy)]
struct GreedyQuadRect {
    direction: FaceDirection,
    plane: i32,
    u_start: i32,
    v_start: i32,
    width: i32,
    height: i32,
}

#[derive(Default)]
struct ChunkBuildAccumulator {
    aabb: Option<ChunkAabb>,
    faces: BTreeMap<FaceBucketKey, BTreeSet<(i32, i32)>>,
    fallback_objects: Vec<usize>,
    stats: ChunkBuildStats,
}

impl ChunkBuildAccumulator {
    fn include_aabb(&mut self, min: [f32; 3], max: [f32; 3]) {
        match self.aabb.as_mut() {
            Some(aabb) => {
                aabb.include_point(min);
                aabb.include_point(max);
            }
            None => self.aabb = Some(ChunkAabb::from_min_max(min, max)),
        }
    }

    fn include_object(&mut self, object: &LevelObject) {
        match self.aabb.as_mut() {
            Some(aabb) => aabb.include_object(object),
            None => {
                self.aabb = Some(ChunkAabb::from_min_max(
                    object.position,
                    [
                        object.position[0] + object.size[0],
                        object.position[1] + object.size[1],
                        object.position[2] + object.size[2],
                    ],
                ));
            }
        }
    }
}

pub fn classify_voxel_fast_lane(object: &LevelObject) -> Result<(), VoxelFastLaneRejection> {
    if object.trigger.is_some() {
        return Err(VoxelFastLaneRejection::Trigger);
    }
    if object
        .rotation_degrees
        .iter()
        .any(|value| value.abs() > GRID_EPSILON)
    {
        return Err(VoxelFastLaneRejection::Rotated);
    }
    if object
        .position
        .iter()
        .chain(object.size.iter())
        .any(|value| !value.is_finite())
    {
        return Err(VoxelFastLaneRejection::NonFiniteBounds);
    }
    if object.size.iter().any(|value| *value <= GRID_EPSILON) {
        return Err(VoxelFastLaneRejection::EmptyOrNegativeSize);
    }
    if object
        .position
        .iter()
        .chain(object.size.iter())
        .any(|value| !is_grid_aligned(*value))
    {
        return Err(VoxelFastLaneRejection::NonGridAligned);
    }

    let block = resolve_block_definition(&object.block_id);
    if block.render.profile != BlockRenderProfile::Solid {
        return Err(VoxelFastLaneRejection::RenderProfile);
    }
    if block.animation.idle.has_motion() {
        return Err(VoxelFastLaneRejection::Animated);
    }
    if block.render.noise.abs() > f32::EPSILON {
        return Err(VoxelFastLaneRejection::Noisy);
    }
    if !is_full_cube_geometry(block) {
        return Err(VoxelFastLaneRejection::CustomGeometry);
    }
    // Only unit cubes go through the greedy voxel mesher (which tiles the texture
    // per cell). A resized block falls through to the instanced path so its
    // texture stretches across the single block instead of tiling.
    if !is_unit_size(object.size) {
        return Err(VoxelFastLaneRejection::NonUnitSize);
    }

    Ok(())
}

pub fn build_chunked_block_mesh(objects: &[LevelObject]) -> ChunkedBlockMesh {
    build_chunked_block_mesh_from_refs_at_time(
        &objects.iter().collect::<Vec<_>>(),
        None,
        DEFAULT_RENDER_CHUNK_SIZE,
    )
}

pub fn build_chunked_block_mesh_at_time(
    objects: &[LevelObject],
    current_time_seconds: f32,
) -> ChunkedBlockMesh {
    build_chunked_block_mesh_from_refs_at_time(
        &objects.iter().collect::<Vec<_>>(),
        Some(current_time_seconds),
        DEFAULT_RENDER_CHUNK_SIZE,
    )
}

pub fn build_chunked_block_mesh_from_refs(objects: &[&LevelObject]) -> ChunkedBlockMesh {
    build_chunked_block_mesh_from_refs_at_time(objects, None, DEFAULT_RENDER_CHUNK_SIZE)
}

pub fn build_chunked_block_mesh_from_refs_at_time(
    objects: &[&LevelObject],
    current_time_seconds: Option<f32>,
    chunk_size: i32,
) -> ChunkedBlockMesh {
    puffin::profile_scope!("BuildChunkedBlockMesh");
    let chunk_size = chunk_size.max(1);
    let mut block_ids = BTreeMap::<String, u32>::new();
    let mut materials = HashMap::<MaterialKey, VoxelMaterial>::new();
    let mut occupancy = HashMap::<VoxelCell, MaterialKey>::new();
    let mut accumulators = BTreeMap::<RenderChunkKey, ChunkBuildAccumulator>::new();
    let mut stats = ChunkBuildStats::default();

    for (object_index, object) in objects.iter().enumerate() {
        if classify_voxel_fast_lane(object).is_ok() {
            let material_key = material_key_for_object(object, &mut block_ids);
            materials
                .entry(material_key)
                .or_insert_with(|| material_for_object(object));
            let min = [
                rounded_grid_coord(object.position[0]),
                rounded_grid_coord(object.position[1]),
                rounded_grid_coord(object.position[2]),
            ];
            let max = [
                rounded_grid_coord(object.position[0] + object.size[0]),
                rounded_grid_coord(object.position[1] + object.size[1]),
                rounded_grid_coord(object.position[2] + object.size[2]),
            ];
            for x in min[0]..max[0] {
                for y in min[1]..max[1] {
                    for z in min[2]..max[2] {
                        occupancy.insert(VoxelCell { x, y, z }, material_key);
                        stats.voxel_cells += 1;
                    }
                }
            }
        } else {
            let key = chunk_key_for_point(object.position, chunk_size);
            let accumulator = accumulators.entry(key).or_default();
            accumulator.fallback_objects.push(object_index);
            accumulator.include_object(object);
            accumulator.stats.fallback_objects += 1;
            stats.fallback_objects += 1;
        }
    }

    for (&cell, &material) in &occupancy {
        add_visible_faces(
            cell,
            material,
            &occupancy,
            &mut accumulators,
            chunk_size,
            &mut stats,
        );
    }

    let mut chunks = Vec::with_capacity(accumulators.len());
    for (key, accumulator) in accumulators {
        let (greedy_geometry, merged_quads) = build_greedy_geometry(&accumulator.faces, &materials);
        let fallback_refs = accumulator
            .fallback_objects
            .iter()
            .filter_map(|index| objects.get(*index).copied())
            .collect::<Vec<_>>();
        let fallback = match current_time_seconds {
            Some(time_seconds) => {
                build_block_instance_batches_from_refs_at_time(&fallback_refs, Some(time_seconds))
            }
            None => build_block_instance_batches_from_refs(&fallback_refs),
        };
        let aabb = accumulator
            .aabb
            .unwrap_or_else(|| chunk_world_aabb(key, chunk_size));
        let mut chunk_stats = accumulator.stats;
        chunk_stats.merged_quads = merged_quads;
        stats.merged_quads += merged_quads;
        chunks.push(BlockMeshChunk {
            key,
            aabb,
            greedy_geometry,
            fallback,
            stats: chunk_stats,
        });
    }

    ChunkedBlockMesh { chunks, stats }
}

fn add_visible_faces(
    cell: VoxelCell,
    material: MaterialKey,
    occupancy: &HashMap<VoxelCell, MaterialKey>,
    accumulators: &mut BTreeMap<RenderChunkKey, ChunkBuildAccumulator>,
    chunk_size: i32,
    total_stats: &mut ChunkBuildStats,
) {
    for (direction, neighbor) in [
        (
            FaceDirection::Top,
            VoxelCell {
                x: cell.x,
                y: cell.y + 1,
                z: cell.z,
            },
        ),
        (
            FaceDirection::Bottom,
            VoxelCell {
                x: cell.x,
                y: cell.y - 1,
                z: cell.z,
            },
        ),
        (
            FaceDirection::PosX,
            VoxelCell {
                x: cell.x + 1,
                y: cell.y,
                z: cell.z,
            },
        ),
        (
            FaceDirection::NegX,
            VoxelCell {
                x: cell.x - 1,
                y: cell.y,
                z: cell.z,
            },
        ),
        (
            FaceDirection::PosZ,
            VoxelCell {
                x: cell.x,
                y: cell.y,
                z: cell.z + 1,
            },
        ),
        (
            FaceDirection::NegZ,
            VoxelCell {
                x: cell.x,
                y: cell.y,
                z: cell.z - 1,
            },
        ),
    ] {
        if occupancy.contains_key(&neighbor) {
            total_stats.hidden_faces += 1;
            continue;
        }

        let (plane, u, v) = face_coords(direction, cell);
        let chunk_key = chunk_key_for_cell(cell, chunk_size);
        let accumulator = accumulators.entry(chunk_key).or_default();
        accumulator
            .faces
            .entry(FaceBucketKey {
                direction,
                plane,
                material,
            })
            .or_default()
            .insert((u, v));
        accumulator.include_aabb(
            [cell.x as f32, cell.y as f32, cell.z as f32],
            [
                (cell.x + 1) as f32,
                (cell.y + 1) as f32,
                (cell.z + 1) as f32,
            ],
        );
        accumulator.stats.visible_faces_before_greedy += 1;
        total_stats.visible_faces_before_greedy += 1;
    }
}

fn build_greedy_geometry(
    faces: &BTreeMap<FaceBucketKey, BTreeSet<(i32, i32)>>,
    materials: &HashMap<MaterialKey, VoxelMaterial>,
) -> (MeshGeometry, usize) {
    let mut geometry = MeshGeometry::default();
    let mut merged_quads = 0;
    for (bucket, cells) in faces {
        let Some(material) = materials.get(&bucket.material) else {
            continue;
        };
        let mut remaining = cells.clone();
        while let Some(&(u_start, v_start)) = remaining.iter().next() {
            let mut width = 1;
            while remaining.contains(&(u_start + width, v_start)) {
                width += 1;
            }

            let mut height = 1;
            'height: loop {
                for u_offset in 0..width {
                    if !remaining.contains(&(u_start + u_offset, v_start + height)) {
                        break 'height;
                    }
                }
                height += 1;
            }

            for u_offset in 0..width {
                for v_offset in 0..height {
                    remaining.remove(&(u_start + u_offset, v_start + v_offset));
                }
            }

            append_greedy_quad(
                &mut geometry,
                GreedyQuadRect {
                    direction: bucket.direction,
                    plane: bucket.plane,
                    u_start,
                    v_start,
                    width,
                    height,
                },
                *material,
            );
            merged_quads += 1;
        }
    }
    (geometry, merged_quads)
}

fn append_greedy_quad(geometry: &mut MeshGeometry, rect: GreedyQuadRect, material: VoxelMaterial) {
    let u0 = rect.u_start as f32;
    let u1 = (rect.u_start + rect.width) as f32;
    let v0 = rect.v_start as f32;
    let v1 = (rect.v_start + rect.height) as f32;
    let plane = rect.plane as f32;
    let width_f = rect.width as f32;
    let height_f = rect.height as f32;

    let (positions, color, texture_layer) = match rect.direction {
        FaceDirection::Top => (
            [
                [u0, plane, v1],
                [u0, plane, v0],
                [u1, plane, v0],
                [u1, plane, v1],
            ],
            material.colors.top,
            material.layers.top,
        ),
        FaceDirection::Bottom => (
            [
                [u0, plane, v0],
                [u0, plane, v1],
                [u1, plane, v1],
                [u1, plane, v0],
            ],
            material.colors.bottom,
            material.layers.bottom,
        ),
        FaceDirection::PosX => (
            [
                [plane, v0, u1],
                [plane, v1, u1],
                [plane, v1, u0],
                [plane, v0, u0],
            ],
            material.colors.side,
            material.layers.side,
        ),
        FaceDirection::NegX => (
            [
                [plane, v0, u0],
                [plane, v1, u0],
                [plane, v1, u1],
                [plane, v0, u1],
            ],
            material.colors.side,
            material.layers.side,
        ),
        FaceDirection::PosZ => (
            [
                [u0, v0, plane],
                [u0, v1, plane],
                [u1, v1, plane],
                [u1, v0, plane],
            ],
            material.colors.side,
            material.layers.side,
        ),
        FaceDirection::NegZ => (
            [
                [u1, v0, plane],
                [u1, v1, plane],
                [u0, v1, plane],
                [u0, v0, plane],
            ],
            material.colors.side,
            material.layers.side,
        ),
    };

    // Tile the texture per world unit across the merged quad. The greedy mesher
    // only ever receives unit-sized voxel cells (resized blocks are routed to the
    // instanced path by the classifier so they stretch instead), so a row of
    // placed blocks tiles once per block rather than stretching across the merge.
    let face_size = [width_f, height_f];

    let vertices = vec![
        Vertex::textured_with_outline(
            positions[0],
            color,
            [0.0, height_f],
            face_size,
            texture_layer,
            material.colors.outline,
        ),
        Vertex::textured_with_outline(
            positions[1],
            color,
            [0.0, 0.0],
            face_size,
            texture_layer,
            material.colors.outline,
        ),
        Vertex::textured_with_outline(
            positions[2],
            color,
            [width_f, 0.0],
            face_size,
            texture_layer,
            material.colors.outline,
        ),
        Vertex::textured_with_outline(
            positions[3],
            color,
            [width_f, height_f],
            face_size,
            texture_layer,
            material.colors.outline,
        ),
    ];
    geometry.append_indexed(vertices, &UNIT_QUAD_INDICES);
}

fn face_coords(direction: FaceDirection, cell: VoxelCell) -> (i32, i32, i32) {
    match direction {
        FaceDirection::Top => (cell.y + 1, cell.x, cell.z),
        FaceDirection::Bottom => (cell.y, cell.x, cell.z),
        FaceDirection::PosX => (cell.x + 1, cell.z, cell.y),
        FaceDirection::NegX => (cell.x, cell.z, cell.y),
        FaceDirection::PosZ => (cell.z + 1, cell.x, cell.y),
        FaceDirection::NegZ => (cell.z, cell.x, cell.y),
    }
}

fn material_key_for_object(
    object: &LevelObject,
    block_ids: &mut BTreeMap<String, u32>,
) -> MaterialKey {
    let next_id = block_ids.len() as u32;
    let block_id = *block_ids.entry(object.block_id.clone()).or_insert(next_id);
    MaterialKey {
        block_id,
        tint: object.color_tint.map(f32::to_bits),
    }
}

fn material_for_object(object: &LevelObject) -> VoxelMaterial {
    let block = resolve_block_definition(&object.block_id);
    let texture_layers = resolve_block_texture_layers(&object.block_id);
    let mut colors = VoxelColors {
        top: block.render.color_top,
        side: block.render.color_side,
        bottom: block.render.color_bottom,
        outline: block.render.color_outline,
    };
    colors.apply_tint(object.color_tint);
    VoxelMaterial {
        colors: PrismFaceColors::new_with_outline(
            colors.top,
            colors.side,
            colors.bottom,
            colors.outline,
        ),
        layers: PrismTextureLayers::new(
            texture_layers.top,
            texture_layers.side,
            texture_layers.bottom,
        ),
    }
}

fn is_full_cube_geometry(block: &BlockDefinition) -> bool {
    block.geometry.elements.is_empty()
        || (block.geometry.elements.len() == 1 && is_full_cube_cuboid(block.geometry.elements[0]))
}

fn is_full_cube_cuboid(cuboid: BlockCuboid) -> bool {
    cuboid.color_tint.is_none()
        && approx_eq(cuboid.from[0], 0.0)
        && approx_eq(cuboid.from[1], 0.0)
        && approx_eq(cuboid.from[2], 0.0)
        && approx_eq(cuboid.to[0], 16.0)
        && approx_eq(cuboid.to[1], 16.0)
        && approx_eq(cuboid.to[2], 16.0)
}

fn chunk_key_for_point(point: [f32; 3], chunk_size: i32) -> RenderChunkKey {
    RenderChunkKey {
        x: floor_div(point[0].floor() as i32, chunk_size),
        y: floor_div(point[1].floor() as i32, chunk_size),
        z: floor_div(point[2].floor() as i32, chunk_size),
    }
}

fn chunk_key_for_cell(cell: VoxelCell, chunk_size: i32) -> RenderChunkKey {
    RenderChunkKey {
        x: floor_div(cell.x, chunk_size),
        y: floor_div(cell.y, chunk_size),
        z: floor_div(cell.z, chunk_size),
    }
}

fn chunk_world_aabb(key: RenderChunkKey, chunk_size: i32) -> ChunkAabb {
    let min = [
        (key.x * chunk_size) as f32,
        (key.y * chunk_size) as f32,
        (key.z * chunk_size) as f32,
    ];
    ChunkAabb::from_min_max(
        min,
        [
            min[0] + chunk_size as f32,
            min[1] + chunk_size as f32,
            min[2] + chunk_size as f32,
        ],
    )
}

fn floor_div(value: i32, divisor: i32) -> i32 {
    let quotient = value / divisor;
    let remainder = value % divisor;
    if remainder != 0 && (remainder < 0) != (divisor < 0) {
        quotient - 1
    } else {
        quotient
    }
}

fn rounded_grid_coord(value: f32) -> i32 {
    value.round() as i32
}

fn is_grid_aligned(value: f32) -> bool {
    approx_eq(value, value.round())
}

fn is_unit_size(size: [f32; 3]) -> bool {
    approx_eq(size[0], 1.0) && approx_eq(size[1], 1.0) && approx_eq(size[2], 1.0)
}

fn approx_eq(a: f32, b: f32) -> bool {
    (a - b).abs() <= GRID_EPSILON
}

#[cfg(test)]
mod tests {
    use super::{build_chunked_block_mesh, classify_voxel_fast_lane, VoxelFastLaneRejection};
    use eg_types::LevelObject;

    #[test]
    fn classifier_accepts_grid_aligned_solid_cube() {
        let object = LevelObject {
            block_id: "core/stone".to_string(),
            ..LevelObject::default()
        };

        assert!(classify_voxel_fast_lane(&object).is_ok());
    }

    #[test]
    fn classifier_rejects_custom_geometry() {
        let object = LevelObject {
            block_id: "core/wooden_fence".to_string(),
            ..LevelObject::default()
        };

        assert!(classify_voxel_fast_lane(&object).is_err());
    }

    #[test]
    fn greedy_mesh_removes_hidden_faces_and_merges_rows() {
        let objects = vec![
            LevelObject {
                block_id: "core/stone".to_string(),
                position: [0.0, 0.0, 0.0],
                ..LevelObject::default()
            },
            LevelObject {
                block_id: "core/stone".to_string(),
                position: [1.0, 0.0, 0.0],
                ..LevelObject::default()
            },
        ];

        let mesh = build_chunked_block_mesh(&objects);

        assert_eq!(mesh.stats.voxel_cells, 2);
        assert_eq!(mesh.stats.hidden_faces, 2);
        assert_eq!(mesh.stats.visible_faces_before_greedy, 10);
        assert!(mesh.stats.merged_quads < mesh.stats.visible_faces_before_greedy);
        assert_eq!(mesh.chunks.len(), 1);
        assert!(mesh.chunks[0].greedy_geometry.vertex_count() < 10 * 4);
        assert!(mesh.chunks[0].fallback.batches.is_empty());
    }

    #[test]
    fn cross_chunk_neighbors_hide_boundary_faces() {
        let objects = vec![
            LevelObject {
                block_id: "core/stone".to_string(),
                position: [15.0, 0.0, 0.0],
                ..LevelObject::default()
            },
            LevelObject {
                block_id: "core/stone".to_string(),
                position: [16.0, 0.0, 0.0],
                ..LevelObject::default()
            },
        ];

        let mesh = build_chunked_block_mesh(&objects);

        assert_eq!(mesh.chunks.len(), 2);
        assert_eq!(mesh.stats.hidden_faces, 2);
        assert_eq!(mesh.stats.visible_faces_before_greedy, 10);
    }

    #[test]
    fn classifier_rejects_resized_grid_aligned_block() {
        // A solid full-cube block resized to an integer size > 1 is grid aligned
        // but no longer a unit cube. It must be rejected so it renders through the
        // instanced path (stretching its texture) instead of the greedy mesher
        // (which would tile per voxel cell).
        let object = LevelObject {
            block_id: "core/stone".to_string(),
            size: [2.0, 1.0, 1.0],
            ..LevelObject::default()
        };

        assert_eq!(
            classify_voxel_fast_lane(&object),
            Err(VoxelFastLaneRejection::NonUnitSize)
        );
    }

    #[test]
    fn classifier_accepts_unit_cube_after_resize_check() {
        // A default unit cube is still accepted by the greedy fast lane.
        let object = LevelObject {
            block_id: "core/stone".to_string(),
            ..LevelObject::default()
        };

        assert!(classify_voxel_fast_lane(&object).is_ok());
    }

    #[test]
    fn resized_block_routes_to_fallback_instanced_path() {
        // A resized block should not contribute voxel cells to the greedy mesh;
        // instead it lands in the fallback (instanced) batches so its texture
        // stretches across the single block.
        let objects = vec![LevelObject {
            block_id: "core/stone".to_string(),
            size: [3.0, 1.0, 1.0],
            ..LevelObject::default()
        }];

        let mesh = build_chunked_block_mesh(&objects);

        assert_eq!(mesh.stats.voxel_cells, 0);
        assert_eq!(mesh.stats.fallback_objects, 1);
        assert!(mesh.chunks[0].greedy_geometry.vertex_count() == 0);
        assert_eq!(mesh.chunks[0].fallback.batches.len(), 1);
    }
}
