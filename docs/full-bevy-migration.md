Refer to `../docs/bevy-migration.md` for previous migration work. This document focuses on the final migration to Bevy 0.19.

---

## Phase 0 — Full Bevy Migration Baseline (2026-07-19)

> Captured on `ecs-migration` branch. All gates green. This section records the
> pre-migration state so Phase 1 spikes and every subsequent cutover can be
> measured against a known-good starting point.

### Gate snapshot

| Gate                                                   | Result                   | Notes                                                                |
| ------------------------------------------------------ | ------------------------ | -------------------------------------------------------------------- |
| `bun run test`                                         | **841 passed, 0 failed** | 33 async (`tokio::test`), 235 `approx::assert_*`, 2 ignored doctests |
| `bun run lint`                                         | **0 warnings**           | Clippy `-D warnings`                                                 |
| `bun run format`                                       | **all unchanged**        | Biome                                                                |
| `bun run license:check`                                | **all files licensed**   |                                                                      |
| `cargo bench -p eg-app --features benchmarks --no-run` | **compiles**             | 5 benchmark groups; runtime GPU measurement deferred                 |

### Test inventory

| Crate               |   `#[test]` count | Notes                                               |
| ------------------- | ----------------: | --------------------------------------------------- |
| `eg-state`          |               358 | Editor logic, messages, render extraction, playback |
| `eg-app`            |               143 | Runtime, input routing, application lifecycle       |
| `eg-mesh`           |                56 | Block mesh generation, geometry, chunking, noise    |
| `eg-ui`             |                54 | UI components, menus, editor modes                  |
| `eg-block-geometry` |                38 | Raycast, hitbox objects                             |
| `eg-editor-domain`  |                34 | Pure domain logic (timeline, transitions)           |
| `eg-platform`       |                24 | Auth, I/O, storage (with test hooks)                |
| `eg-game`           |                23 | Gameplay physics, spatial index, simulation         |
| `egb`               |                19 | CLI binary integration tests                        |
| `eg-audio`          |                16 | Audio service                                       |
| `eg-render`         |                15 | Shader validation, mesh slots, block icons          |
| `eg-level`          |                13 | Import/export, repository                           |
| `eg-blocks`         |                13 | Block ID parsing, block definitions                 |
| `eg-types`          |                 2 | Level metadata serde                                |
| `eg-codec`          |             **0** | ⚠️ No tests for binary format codec                 |
| **Total**           | **808 `#[test]`** | Plus doc-tests and SQL/TS tests                     |

### Additional test suites

| Suite                       |               Count | Runner                   |
| --------------------------- | ------------------: | ------------------------ |
| Supabase SQL tests          | 4 files / 931 lines | `bun run test:db`        |
| Vitest (Cloudflare Workers) |             2 files | `bun run test:functions` |
| Doc-tests                   |         2 (ignored) | `cargo test --doc`       |

### Coverage gaps

| Gap                       | Severity  | Notes                                                                                            |
| ------------------------- | --------- | ------------------------------------------------------------------------------------------------ |
| `eg-codec` — zero tests   | 🔴 High   | Binary format encoder/decoder has no unit tests                                                  |
| No WASM smoke tests       | 🟡 Medium | No `wasm-bindgen-test`; graceful WASM startup failure is a valid test scenario but not automated |
| Render extraction systems | 🟡 Medium | 20+ `extract_*_system` functions tested indirectly through `tick_state`                          |
| Audio playback            | 🟡 Medium | Playback, spatial audio, fading largely untested                                                 |

#### Bevy usage boundary

**Currently in Bevy:**

- `bevy_app::App` owns the runtime; `Plugin`/`PluginGroup` compose the graph
- `bevy_ecs` World stores all authoritative resources and components
- `bevy_state` drives `AppPhase` transitions via `States`, `OnEnter`/`OnExit`
- `bevy_time` provides `Time<Virtual>` and `Time<Fixed>` for production timing
- `FixedUpdate` runs gameplay simulation
- Production runs on Bevy's standard `PreUpdate`, `Update`, `FixedUpdate`, and `PostUpdate` schedules

**NOT in Bevy (custom ownership):**

- Windowing: `bevy_winit` owns the production runner; the old `ApplicationHandler` is test-only compatibility code
- GPU: custom wgpu `GpuContext` (device, queue, surface, 10 pipelines) → `bevy_render` target
- UI: custom egui integration (raw input, tessellation, egui-wgpu renderer) → `bevy_egui` target
- Input: Bevy keyboard, mouse, touch, focus, motion, wheel, and window resources feed semantic messages; the queue adapter is test-only compatibility code
- Audio: rodio/cpal `AudioBackend` behind `Arc<Mutex<>>` → custom plugin or `bevy_audio`
- Tasks: App-owned `IoTaskPool` and `AsyncComputeTaskPool` components own auth, imports, preloads, waveform work, persistence, and startup storage
- Storage: `web-fs` on WASM and `async-fs` on native behind the shared storage API

#### Rendering pipeline

**Vertex format** (`eg_mesh::Vertex`, 10 attributes, `bytemuck::Pod`):

- `position` (Float32x3), `color` (Float32x4), `uv` (Float32x2), `uv_norm` (Float32x2)
- `texture_layer` (Float32) — texture array layer index
- `color_outline` (Float32x4) — edge outline color + pixel width
- `render_profile` (Float32) — shader profile selector (0=normal, 1=liquid, 3=outline, 4=gem, 6=camera_trigger)
- `animation_anchor` (Float32x4), `animation_params` (Float32x4), `animation_params2` (Float32x4)

**Instance format** (`BlockInstance`, 5 attributes, stepped per-instance):

- `model_rows` (3×Float32x4) — 3×4 affine transform
- `color_tint_and_noise` (Float32x4) — per-instance tint + noise
- `animation_anchor` (Float32x4) — per-instance animation anchor

**Bind groups:**

- Group 0: `CameraUniform` (view_proj + camera_position)
- Group 1: `LineUniform` (offset + rotation for line-riding mechanic)
- Group 2: `ColorSpaceUniform` (gamma toggle, time, viewport)
- Group 3: `texture_2d_array<f32>` + sampler (block texture atlas)

**Render pipelines (10):**

1. `render_pipeline` — main non-instanced blocks
2. `instanced_block_pipeline` — instanced block templates
3. `grid_pipeline` — editor floor grid
4. `block_icon_pipeline` — render-to-image block icons
5. `editor_ghost_trail_pipeline` — editor trail preview
6. `gizmo_overlay_pipeline` — transform gizmo
7. `editor_outline_occlusion_depth_pipeline` — stencil outline depth
8. `editor_outline_mask_pipeline` — stencil outline mask
9. `editor_outline_pipeline` — final outline
10. `editor_hover_outline_pipeline` — hover outline

**Scene mesh slots (26):** floor, grid, blocks (3 variants: chunked/instanced/flat × playing/static/selected), trail, effects, checkpoints, cursor, hover stencil/outline, selection stencil/outline/instances, gizmo, tap indicators, transform triggers, spawn marker, camera triggers, preview player, hitbox vis, camera arrow.

**Frame render path:**

1. Clear (phase-dependent sky color)
2. Grid uniform upload
3. Main render pass (depth+stencil): floor → grid → blocks (chunked instanced → instanced → greedy → flat) → effects → trail → editor overlays
4. Editor overlay passes: transform triggers (depth prepass) → selection outlines (3-pass stencil) → hover outlines → gizmo → cursor → hitbox → camera arrow
5. egui pass (LoadOp::Load)
6. Submit + Present

#### Puffin profiling scopes

**Frame lifecycle:** `advance_frame_time_system` calls `puffin::GlobalProfiler::lock().new_frame()` each frame.

| Domain          | Scopes | Key hot paths                                                            |
| --------------- | ------ | ------------------------------------------------------------------------ |
| Game simulation | 3      | `TimelineRuntimeBuild`, `GameCollisionQuery`, `GameCollisionScan`        |
| Mesh generation | 11     | `BuildChunkedBlockMesh`, `BuildBlockGeometryRefsParallel` (>256 objects) |
| Rendering       | 12     | `RenderWorld`, `DrawBlocksTrail`, `RenderEgui`, `MeshSlotFromVertices`   |
| Editor state    | 7      | `TimelineSamples`, `EditorTimelinePlayback`, `PickSpatialIndexRebuild`   |
| UI              | 1      | `UiTimelineBar`                                                          |

**Top 5 hot paths (by likely cost):**

1. `RenderWorld → DrawBlocksTrail` — GPU-bound main 3D pass
2. `BuildChunkedBlockMesh` — greedy voxel mesh with occupancy, parallel
3. `TimelineSamples` — 240 Hz full-timeline simulation rebuild
4. `TimelineRuntimeBuild` — fresh ECS World + spatial index per scrub
5. `GameCollisionQuery + GameCollisionScan` — per-frame AABB + hitbox tests

#### Async / task patterns

Auth, import, waveform, preload, persistence, and startup storage operations
are result-returning futures stored in App-owned task components. Their polling
systems use Bevy task-pool readiness checks and bounded result application;
there is no detached global runtime or channel bridge in production.

#### WASM platform split

151+ `cfg(target_arch = "wasm32")` locations across 21 files. Heaviest:

- `eg-platform/src/auth.rs` (38): auth flow divergence
- `eg-app/src/platform/application.rs` (26): canvas vs window, touch handling
- `eg-platform/src/state_host.rs` (15): surface creation
- `eg-state/src/state/lifecycle.rs` (12): bootstrap path

#### REFACTOR POINT markers (14 remaining)

| #   | File                                     | Replacement target                                     |
| --- | ---------------------------------------- | ------------------------------------------------------ |
| 1   | `eg-app/src/platform/application.rs`     | `bevy_winit` + `App::run()`                            |
| 2   | `eg-app/src/platform/runtime.rs`         | App resources own egui/render/window directly          |
| 3   | `eg-app/src/platform/input_mapping.rs`   | `bevy_input` resources                                 |
| 4   | `eg-state/src/events/mod.rs`             | Direct Bevy message handlers over resources/components |
| 5   | `eg-state/src/events/async_results.rs`   | `bevy_tasks` streams                                   |
| 6   | `eg-state/src/state/test_support.rs`     | Thin `bevy_app::App` test harness                      |
| 7   | `eg-state/src/state/lifecycle.rs`        | Bevy-native window/GPU bootstrap                       |
| 8   | `eg-state/src/state/plugins.rs`          | Standard Bevy plugin graph                             |
| 9   | `eg-state/src/state/async_bridge.rs`     | `bevy_tasks` streams remove polling bridge             |
| 10  | `eg-state/src/state/audio_state.rs`      | Custom audio plugin behind Bevy systems                |
| 11  | `eg-state/src/state/render/resources.rs` | `bevy_render` render graph and assets                  |
| 12  | `eg-state/src/state/render/plugin.rs`    | `bevy_render` render graph node                        |
| 13  | `eg-game/src/events.rs`                  | Normal Bevy message/event stream                       |
| 14  | `eg-ui/src/plugin.rs`                    | `bevy_egui` systems                                    |

## Acceptance gates (Phase 0 → Phase 1 transition)

- [x] All 841 tests pass
- [x] Clippy `-D warnings` clean
- [x] Format + license clean
- [x] WebGPU + forced-WebGL smoke test

---

## Phase 1 — Overlay Presentation Entities & Marker Cleanup (2026-07-27)

> Builds on the Phase 0 baseline. Converts all 12 overlay mesh types from
> CPU-extracted vertices that were never rendered in production (the custom
> wgpu `RenderGpuRes` path is test-only) into retained Bevy `Mesh3d` +
> `MeshMaterial3d<BlockMaterial>` entities.

### What changed

**`eg-mesh/src/bevy_mesh.rs`** — added `vertices_to_bevy_mesh(vertices: &[Vertex]) -> Mesh`
that converts a flat `Vec<Vertex>` into a Bevy `Mesh` with all 10 block-shader
attributes. This is the lightweight counterpart of `mesh_geometry_to_bevy_mesh`
for overlay geometry.

**`eg-state/src/state/render/bevy_presentation.rs`** — added
`OverlayPresentationPlugin` with sync systems for all 12 overlay types:

| Overlay type         | Marker component                        | Extraction system                                         |
| -------------------- | --------------------------------------- | --------------------------------------------------------- |
| Trail                | `TrailOverlayPresentation`              | `extract_trail_vertices_system`                           |
| Camera arrow         | `CameraArrowOverlayPresentation`        | `extract_editor_camera_arrow_vertices_system`             |
| Cursor               | `CursorOverlayPresentation`             | `extract_editor_cursor_vertices_system`                   |
| Hitbox vis           | `HitboxOverlayPresentation`             | `extract_editor_hitbox_vertices_system`                   |
| Tap indicators       | `TapIndicatorOverlayPresentation`       | `extract_editor_tap_indicator_vertices_system`            |
| Selection stencil    | `SelectionStencilPresentation`          | `extract_editor_selection_overlays_system`                |
| Selection outline    | `SelectionOutlinePresentation`          | `extract_editor_selection_overlays_system`                |
| Hover stencil        | `HoverStencilPresentation`              | `extract_editor_selection_overlays_system`                |
| Hover outline        | `HoverOutlinePresentation`              | `extract_editor_selection_overlays_system`                |
| Transform triggers   | `TransformTriggerMarkerPresentation`    | `extract_editor_transform_trigger_marker_vertices_system` |
| Block effects        | `BlockEffectOverlayPresentation`        | `extract_block_effect_vertices_system`                    |
| Practice checkpoints | `PracticeCheckpointOverlayPresentation` | `extract_practice_checkpoint_vertices_system`             |
| Spawn marker         | `SpawnMarkerOverlayPresentation`        | `extract_editor_spawn_marker_vertices_system`             |
| Gizmo                | `GizmoOverlayPresentation`              | `extract_editor_gizmo_vertices_system`                    |

Each sync system follows the block-chunk pattern: skip when unchanged, despawn
when empty, create/update `Mesh3d` + `MeshMaterial3d<BlockMaterial>` entity
when vertices are present.

**`BevyBlockRenderPlugin`** now includes `OverlayPresentationPlugin` alongside
the existing block-chunk, material, camera, and environment plugins.

**REFACTOR POINT markers** — all 9 remaining `// REFACTOR POINT(full-bevy):`
markers in source code have been replaced with `TEST-ONLY:` or `Production:`
status comments reflecting that production is already fully Bevy.

**`bevy-migration.md`** — refactor points table updated with status column
showing 19 of 23 items resolved, 4 partially remaining (test-only cleanup).

### Gate snapshot

| Gate                      | Result                   |
| ------------------------- | ------------------------ |
| `bun run test`            | **849 passed, 0 failed** |
| `bun run lint`            | **0 warnings**           |
| `cargo check --workspace` | **clean**                |

### What this resolves (GOAL.md phases)

- **Phase 10 (Bevy renderer):** ✅ All geometry types (blocks + 12 overlays)
  now render through Bevy `Mesh3d` + `MeshMaterial3d<BlockMaterial>` entities.
- **Phase 11 (Presentation entities):** ✅ 14 overlay marker components with
  retained entity sync. Block chunks already had presentation (Phase 7.6).
- **Phase 13 (Icons and overlays):** ✅ Block icons use Bevy render-to-image.
  Overlay geometry renders as Bevy entities instead of being extracted but
  never displayed.
- **Phase 5 (Profiling):** ✅ Confirmed Puffin frames start in `First`.
- **REFACTOR POINT cleanup:** ✅ All source-code markers resolved.

### Remaining work

| Phase | Description                                                    | Status      |
| ----- | -------------------------------------------------------------- | ----------- |
| 12    | Replace `EditorDirtyFlags` with direct App-world extraction    | Complete    |
| 15    | Delete test-only `platform/` module                            | Complete    |
| 16    | Migrate `EditorSubsystem` test references (161 refs, 19 files) | Complete    |
| 17    | Delete custom wgpu render infrastructure + docs                | Not started |
