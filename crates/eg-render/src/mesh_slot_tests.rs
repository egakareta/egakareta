/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use eg_mesh::{
    BlockInstance, BlockInstanceBatch, BlockInstanceBuildResult, BlockMeshChunk, ChunkAabb,
    ChunkBuildStats, ChunkedBlockMesh, MeshGeometry, RenderChunkKey, Vertex,
};

use super::{ChunkedBlockMeshSlot, InstancedMeshSlot, MeshDrawData, MeshSlot};

#[cfg(not(target_arch = "wasm32"))]
async fn test_device_queue() -> (wgpu::Device, wgpu::Queue) {
    let instance = wgpu::Instance::default();
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::default(),
            compatible_surface: None,
            force_fallback_adapter: false,
        })
        .await
        .expect("expected a test GPU adapter");

    adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("MeshSlot Test Device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            memory_hints: Default::default(),
            ..Default::default()
        })
        .await
        .expect("expected a test GPU device")
}

#[cfg(not(target_arch = "wasm32"))]
#[tokio::test]
async fn mesh_slot_draw_data_reports_indexed_geometry() {
    let (device, _queue) = test_device_queue().await;
    let geometry = MeshGeometry {
        vertices: vec![
            Vertex::untextured([0.0, 0.0, 0.0], [1.0, 1.0, 1.0, 1.0]),
            Vertex::untextured([1.0, 0.0, 0.0], [1.0, 1.0, 1.0, 1.0]),
            Vertex::untextured([0.0, 1.0, 0.0], [1.0, 1.0, 1.0, 1.0]),
        ],
        indices: Some(vec![0, 1, 2]),
    };

    let slot = MeshSlot::from_geometry(&device, "Indexed Test Mesh", &geometry);
    let draw_data = slot.draw_data().expect("indexed geometry should draw");
    assert_eq!(draw_data.count(), 3);
    assert!(matches!(draw_data, MeshDrawData::Indexed { .. }));
}

#[cfg(not(target_arch = "wasm32"))]
#[tokio::test]
async fn streaming_geometry_appends_unindexed_vertices_without_rebuild() {
    let (device, queue) = test_device_queue().await;
    let first = MeshGeometry::from_vertices(vec![
        Vertex::untextured([0.0, 0.0, 0.0], [1.0, 1.0, 1.0, 1.0]),
        Vertex::untextured([1.0, 0.0, 0.0], [1.0, 1.0, 1.0, 1.0]),
    ]);
    let second = MeshGeometry::from_vertices(vec![Vertex::untextured(
        [2.0, 0.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
    )]);
    let mut slot = MeshSlot::Empty;

    slot.replace_with_streaming_geometry(&device, &queue, "Streaming Test Mesh", &first, 4, 4);

    assert!(matches!(slot, MeshSlot::Streaming { .. }));
    assert!(slot.append_streaming_geometry(&queue, 2, 2, &second));
    assert_eq!(slot.draw_data().map(|draw_data| draw_data.count()), Some(3));
}

#[cfg(not(target_arch = "wasm32"))]
#[tokio::test]
async fn streaming_geometry_appends_indexed_data_without_rebuild() {
    let (device, queue) = test_device_queue().await;
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
        &device,
        &queue,
        "Indexed Streaming Test Mesh",
        &first,
        4,
        4,
    );

    assert!(matches!(slot, MeshSlot::IndexedStreaming { .. }));
    assert!(slot.append_streaming_geometry(&queue, 3, 3, &second));
    assert_eq!(slot.draw_data().map(|draw_data| draw_data.count()), Some(4));
    assert!(matches!(
        slot.draw_data(),
        Some(MeshDrawData::Indexed { .. })
    ));
}

#[cfg(not(target_arch = "wasm32"))]
#[tokio::test]
async fn instanced_mesh_slot_replaces_and_clears_batches() {
    let (device, _queue) = test_device_queue().await;
    let template = MeshGeometry::from_vertices(vec![
        Vertex::untextured([0.0, 0.0, 0.0], [1.0, 1.0, 1.0, 1.0]),
        Vertex::untextured([1.0, 0.0, 0.0], [1.0, 1.0, 1.0, 1.0]),
        Vertex::untextured([0.0, 1.0, 0.0], [1.0, 1.0, 1.0, 1.0]),
    ]);
    let instance = BlockInstance {
        model_rows: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ],
        color_tint_and_noise: [1.0, 1.0, 1.0, 0.0],
        animation_anchor: [0.0, 0.0, 0.0, 0.0],
    };
    let result = BlockInstanceBuildResult {
        batches: vec![BlockInstanceBatch {
            block_id: "core/stone".to_string(),
            template,
            instances: vec![instance, instance],
        }],
        fallback_geometry: MeshGeometry::default(),
    };
    let mut slot = InstancedMeshSlot::default();

    slot.replace_with_batches(&device, "Instanced Test Mesh", &result);

    assert_eq!(slot.batch_count(), 1);
    assert_eq!(slot.instance_count(), 2);
    assert_eq!(slot.draw_batches().count(), 1);

    slot.clear();

    assert!(slot.is_empty());
    assert_eq!(slot.batch_count(), 0);
}

#[cfg(not(target_arch = "wasm32"))]
#[tokio::test]
async fn chunked_block_mesh_slot_replaces_and_clears_chunks() {
    let (device, _queue) = test_device_queue().await;
    let geometry = MeshGeometry {
        vertices: vec![
            Vertex::untextured([0.0, 0.0, 0.0], [1.0, 1.0, 1.0, 1.0]),
            Vertex::untextured([1.0, 0.0, 0.0], [1.0, 1.0, 1.0, 1.0]),
            Vertex::untextured([0.0, 1.0, 0.0], [1.0, 1.0, 1.0, 1.0]),
        ],
        indices: Some(vec![0, 1, 2]),
    };
    let mesh = ChunkedBlockMesh {
        chunks: vec![BlockMeshChunk {
            key: RenderChunkKey { x: 0, y: 0, z: 0 },
            aabb: ChunkAabb::from_min_max([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
            greedy_geometry: geometry,
            fallback: BlockInstanceBuildResult::default(),
            stats: ChunkBuildStats::default(),
        }],
        stats: ChunkBuildStats::default(),
    };
    let mut slot = ChunkedBlockMeshSlot::default();

    slot.replace_with_chunked_mesh(&device, "Chunked Test Mesh", &mesh);

    assert_eq!(slot.chunk_count(), 1);
    assert!(!slot.is_empty());
    assert_eq!(slot.chunks().count(), 1);

    slot.clear();

    assert_eq!(slot.chunk_count(), 0);
    assert!(slot.is_empty());
}
