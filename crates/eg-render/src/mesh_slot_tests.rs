/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use eg_mesh::{
    BlockInstance, BlockInstanceBatch, BlockInstanceBuildResult, BlockMeshChunk, ChunkAabb,
    ChunkBuildStats, ChunkedBlockMesh, MeshGeometry, RenderChunkKey, Vertex,
};

use super::{
    ChunkedBlockMeshSlot, InstanceBatchKey, InstancedMeshSlot, MeshDrawData, MeshSlot,
    RetainedMeshSlot,
};

#[cfg(not(target_arch = "wasm32"))]
static TEST_GPU: std::sync::OnceLock<(wgpu::Device, wgpu::Queue)> = std::sync::OnceLock::new();

#[cfg(not(target_arch = "wasm32"))]
fn test_device_queue() -> (&'static wgpu::Device, &'static wgpu::Queue) {
    let (device, queue) = TEST_GPU.get_or_init(|| {
        pollster::block_on(async {
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
        })
    });

    // Keep the software driver's TLS state alive until process exit. Mesa's
    // llvmpipe backend can crash when a test worker drops its device.
    (device, queue)
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn mesh_slot_draw_data_reports_indexed_geometry() {
    let (device, _queue) = test_device_queue();
    let geometry = MeshGeometry {
        vertices: vec![
            Vertex::untextured([0.0, 0.0, 0.0], [1.0, 1.0, 1.0, 1.0]),
            Vertex::untextured([1.0, 0.0, 0.0], [1.0, 1.0, 1.0, 1.0]),
            Vertex::untextured([0.0, 1.0, 0.0], [1.0, 1.0, 1.0, 1.0]),
        ],
        indices: Some(vec![0, 1, 2]),
    };

    let slot = MeshSlot::from_geometry(device, "Indexed Test Mesh", &geometry);
    let draw_data = slot.draw_data().expect("indexed geometry should draw");
    assert_eq!(draw_data.count(), 3);
    assert!(matches!(draw_data, MeshDrawData::Indexed { .. }));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn streaming_geometry_appends_unindexed_vertices_without_rebuild() {
    let (device, queue) = test_device_queue();
    let first = MeshGeometry::from_vertices(vec![
        Vertex::untextured([0.0, 0.0, 0.0], [1.0, 1.0, 1.0, 1.0]),
        Vertex::untextured([1.0, 0.0, 0.0], [1.0, 1.0, 1.0, 1.0]),
    ]);
    let second = MeshGeometry::from_vertices(vec![Vertex::untextured(
        [2.0, 0.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
    )]);
    let mut slot = MeshSlot::Empty;

    slot.replace_with_streaming_geometry(device, queue, "Streaming Test Mesh", &first, 4, 4);

    assert!(matches!(slot, MeshSlot::Streaming { .. }));
    assert!(slot.append_streaming_geometry(queue, 2, 2, &second));
    assert_eq!(slot.draw_data().map(|draw_data| draw_data.count()), Some(3));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn streaming_vertex_replacement_reuses_allocation_and_exposes_outline_buffer() {
    let (device, queue) = test_device_queue();
    let vertices = vec![
        Vertex::untextured([0.0, 0.0, 0.0], [1.0, 1.0, 1.0, 1.0]),
        Vertex::untextured([1.0, 0.0, 0.0], [1.0, 1.0, 1.0, 1.0]),
        Vertex::untextured([0.0, 1.0, 0.0], [1.0, 1.0, 1.0, 1.0]),
    ];
    let mut slot = MeshSlot::Empty;

    slot.replace_with_streaming_vertices(device, queue, "Outline Streaming Mesh", &vertices);
    let initial_capacity = match &slot {
        MeshSlot::Streaming {
            capacity_vertices, ..
        } => *capacity_vertices,
        _ => panic!("replacement must create a streaming mesh"),
    };
    assert!(slot.vertex_buffer().is_some());

    slot.replace_with_streaming_vertices(device, queue, "Outline Streaming Mesh", &vertices[..2]);
    assert_eq!(
        match &slot {
            MeshSlot::Streaming {
                capacity_vertices, ..
            } => *capacity_vertices,
            _ => panic!("replacement must retain the streaming mesh"),
        },
        initial_capacity
    );
    assert_eq!(slot.draw_data().map(|draw_data| draw_data.count()), Some(2));

    let grown = vec![Vertex::untextured([2.0, 0.0, 0.0], [1.0, 1.0, 1.0, 1.0]); 7];
    slot.replace_with_streaming_vertices(device, queue, "Outline Streaming Mesh", &grown);
    assert!(matches!(
        &slot,
        MeshSlot::Streaming {
            capacity_vertices,
            ..
        } if *capacity_vertices > initial_capacity
    ));
    assert_eq!(slot.draw_data().map(|draw_data| draw_data.count()), Some(7));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn streaming_geometry_appends_indexed_data_without_rebuild() {
    let (device, queue) = test_device_queue();
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
        device,
        queue,
        "Indexed Streaming Test Mesh",
        &first,
        4,
        4,
    );

    assert!(matches!(slot, MeshSlot::IndexedStreaming { .. }));
    assert!(slot.append_streaming_geometry(queue, 3, 3, &second));
    assert_eq!(slot.draw_data().map(|draw_data| draw_data.count()), Some(4));
    assert!(matches!(
        slot.draw_data(),
        Some(MeshDrawData::Indexed { .. })
    ));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn instanced_mesh_slot_replaces_and_clears_batches() {
    let (device, _queue) = test_device_queue();
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
        trigger_params: [0.0, 0.0, 0.0, 0.0],
    };
    let result = BlockInstanceBuildResult {
        batches: vec![BlockInstanceBatch {
            block_id: "core/stone".to_string(),
            template,
            chunk_key: None,
            aabb: None,
            object_ids: vec!["first".to_string(), "second".to_string()],
            object_aabbs: vec![None, None],
            instances: vec![instance, instance],
        }],
        fallback_geometry: MeshGeometry::default(),
    };
    let mut slot = InstancedMeshSlot::default();

    slot.replace_with_batches(device, "Instanced Test Mesh", &result);

    assert_eq!(slot.batch_count(), 1);
    assert_eq!(slot.instance_count(), 2);
    assert_eq!(slot.draw_batches().count(), 1);

    let mut moved_instance = instance;
    moved_instance.model_rows[0][3] = 3.0;
    let updated_result = BlockInstanceBuildResult {
        batches: vec![BlockInstanceBatch {
            block_id: "core/stone".to_string(),
            template: result.batches[0].template.clone(),
            chunk_key: None,
            aabb: None,
            object_ids: vec!["first".to_string()],
            object_aabbs: vec![None],
            instances: vec![moved_instance],
        }],
        fallback_geometry: MeshGeometry::default(),
    };
    slot.update_retained_batches(device, _queue, &updated_result);
    assert_eq!(slot.batch_count(), 1);
    assert_eq!(slot.instance_count(), 1);
    assert_eq!(slot.draw_batches().count(), 1);

    slot.clear();

    assert!(slot.is_empty());
    assert_eq!(slot.batch_count(), 0);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn retained_instances_swap_remove_and_share_templates() {
    let (device, queue) = test_device_queue();
    let template = MeshGeometry::from_vertices(vec![Vertex::untextured(
        [0.0, 0.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
    )]);
    let instance = BlockInstance {
        model_rows: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
        ],
        color_tint_and_noise: [1.0, 1.0, 1.0, 0.0],
        animation_anchor: [0.0, 0.0, 0.0, 0.0],
        trigger_params: [0.0, 0.0, 0.0, 0.0],
    };
    let key_a = InstanceBatchKey {
        chunk: Some(RenderChunkKey { x: 0, y: 0, z: 0 }),
        block_id: "core/stone".to_string(),
    };
    let key_b = InstanceBatchKey {
        chunk: Some(RenderChunkKey { x: 1, y: 0, z: 0 }),
        block_id: "core/stone".to_string(),
    };
    let aabb = Some(ChunkAabb::from_min_max([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]));
    let mut slot = InstancedMeshSlot::default();

    slot.upsert_instance(device, "first", &key_a, &template, instance, aabb);
    slot.upsert_instance(device, "second", &key_a, &template, instance, aabb);
    slot.upsert_instance(device, "third", &key_b, &template, instance, aabb);
    slot.sync_dirty_ranges(queue);

    assert_eq!(slot.batch_count(), 2);
    assert!(std::sync::Arc::ptr_eq(
        &slot.batches[0].template_buffer,
        &slot.batches[1].template_buffer
    ));
    assert_eq!(slot.batches[0].capacity_instances, 2);

    let instance_buffer = &slot.batches[0].instance_buffer as *const wgpu::Buffer;
    let mut changed_instance = instance;
    changed_instance.model_rows[0][3] = 2.0;
    slot.upsert_instance(device, "first", &key_a, &template, changed_instance, aabb);
    assert_eq!(slot.batches[0].dirty_ranges, vec![0..1]);
    slot.sync_dirty_ranges(queue);
    assert_eq!(
        instance_buffer,
        &slot.batches[0].instance_buffer as *const wgpu::Buffer
    );

    let old_template = std::sync::Arc::clone(&slot.batches[0].template_buffer);
    let changed_template = MeshGeometry::from_vertices(vec![Vertex::untextured(
        [2.0, 0.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
    )]);
    slot.upsert_instance(device, "first", &key_a, &changed_template, instance, aabb);
    slot.sync_dirty_ranges(queue);
    assert!(!std::sync::Arc::ptr_eq(
        &old_template,
        &slot.batches[0].template_buffer
    ));

    let replaced_template = std::sync::Arc::clone(&slot.batches[0].template_buffer);
    slot.invalidate_template("core/stone");
    slot.upsert_instance(device, "first", &key_a, &changed_template, instance, aabb);
    slot.upsert_instance(device, "second", &key_a, &changed_template, instance, aabb);
    slot.upsert_instance(device, "third", &key_b, &changed_template, instance, aabb);
    slot.sync_dirty_ranges(queue);
    assert!(!std::sync::Arc::ptr_eq(
        &replaced_template,
        &slot.batches[0].template_buffer
    ));
    assert!(std::sync::Arc::ptr_eq(
        &slot.batches[0].template_buffer,
        &slot.batches[1].template_buffer
    ));

    assert!(slot.remove_instance("first"));
    assert_eq!(slot.batches[0].dirty_ranges, vec![0..1]);
    slot.sync_dirty_ranges(queue);
    assert_eq!(slot.object_locations["second"].slot, 0);
    assert_eq!(slot.instance_count(), 2);

    slot.upsert_instance(device, "second", &key_b, &template, instance, aabb);
    slot.sync_dirty_ranges(queue);
    assert_eq!(slot.batch_count(), 1);
    assert_eq!(
        slot.object_locations["second"].batch_index,
        slot.object_locations["third"].batch_index
    );
    assert_eq!(slot.object_locations["second"].slot, 1);
    assert_eq!(slot.instance_count(), 2);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn retained_sidecars_reuse_unchanged_key_buffers() {
    let (device, queue) = test_device_queue();
    let first = MeshGeometry::from_vertices(vec![Vertex::untextured(
        [0.0, 0.0, 0.0],
        [1.0, 1.0, 1.0, 1.0],
    )]);
    let changed = MeshGeometry::from_vertices(vec![
        Vertex::untextured([0.0, 0.0, 0.0], [1.0, 1.0, 1.0, 1.0]),
        Vertex::untextured([1.0, 0.0, 0.0], [1.0, 1.0, 1.0, 1.0]),
    ]);
    let mut slot = RetainedMeshSlot::default();

    slot.update_with_streaming_geometry(device, queue, "Sidecar Test", "first", &first);
    slot.update_with_streaming_geometry(device, queue, "Sidecar Test", "second", &first);
    let first_buffer = match slot.batch("first").expect("first sidecar") {
        MeshSlot::Streaming { buffer, .. } => buffer as *const wgpu::Buffer,
        _ => panic!("expected streaming sidecar"),
    };
    let second_buffer = match slot.batch("second").expect("second sidecar") {
        MeshSlot::Streaming { buffer, .. } => buffer as *const wgpu::Buffer,
        _ => panic!("expected streaming sidecar"),
    };

    slot.update_with_streaming_geometry(device, queue, "Sidecar Test", "first", &changed);
    let second_buffer_after = match slot.batch("second").expect("second sidecar") {
        MeshSlot::Streaming { buffer, .. } => buffer as *const wgpu::Buffer,
        _ => panic!("expected streaming sidecar"),
    };

    assert_eq!(
        first_buffer,
        match slot.batch("first").expect("first sidecar") {
            MeshSlot::Streaming { buffer, .. } => buffer as *const wgpu::Buffer,
            _ => panic!("expected streaming sidecar"),
        }
    );
    assert_eq!(second_buffer, second_buffer_after);
    assert_eq!(
        slot.draw_batches()
            .filter(|slot| slot.draw_data().is_some())
            .count(),
        2
    );
    assert!(slot.remove("first"));
    assert_eq!(
        slot.draw_batches()
            .filter(|slot| slot.draw_data().is_some())
            .count(),
        1
    );
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn chunked_block_mesh_slot_replaces_and_clears_chunks() {
    let (device, _queue) = test_device_queue();
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

    slot.replace_with_chunked_mesh(device, "Chunked Test Mesh", &mesh);

    assert_eq!(slot.chunk_count(), 1);
    assert!(!slot.is_empty());
    assert_eq!(slot.chunks().count(), 1);

    slot.clear();

    assert_eq!(slot.chunk_count(), 0);
    assert!(slot.is_empty());
}
