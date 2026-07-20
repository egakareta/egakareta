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

### Architecture snapshot

#### Dependency versions (workspace Cargo.toml)

| Dependency       | Version        | Purpose                                     |
| ---------------- | -------------- | ------------------------------------------- |
| `bevy_app`       | 0.19           | App, Plugin, PluginGroup                    |
| `bevy_ecs`       | 0.19           | World, Resource, Component, Query, Messages |
| `bevy_state`     | 0.19           | States, StateTransition                     |
| `bevy_time`      | 0.19           | Time, TimePlugin, Fixed, Virtual            |
| `bevy_tasks`     | 0.19           | TaskPool (used sparingly)                   |
| `bevy_clipboard` | 0.19           | Clipboard integration                       |
| `wgpu`           | 29.0.3         | Custom GPU rendering (webgpu + webgl)       |
| `winit`          | 0.30           | Custom windowing                            |
| `egui`           | 0.34.3         | Custom UI                                   |
| `egui-wgpu`      | 0.34.3         | Custom egui rendering                       |
| `egui-winit`     | 0.34.3         | Custom egui input                           |
| `rodio`          | 0.22           | Audio playback                              |
| `cpal`           | 0.17           | Audio device selection                      |
| `tokio`          | 1 (rt, macros) | Native background runtime                   |
| `puffin`         | git (7e08a53)  | Profiling                                   |
| `puffin_egui`    | git (7e08a53)  | In-app profiler widget                      |

#### Bevy usage boundary

**Currently in Bevy:**

- `bevy_app::App` owns the runtime; `Plugin`/`PluginGroup` compose the graph
- `bevy_ecs` World stores all authoritative resources and components
- `bevy_state` drives `AppPhase` transitions via `States`, `OnEnter`/`OnExit`
- `bevy_time` provides `Time<Virtual>` and `Time<Fixed>` for production timing
- `FixedUpdate` runs gameplay simulation
- Custom `StateSchedule` and `RenderExtractSchedule` are Bevy `ScheduleLabel`s

**NOT in Bevy (custom ownership):**

- Windowing: custom winit `ApplicationHandler` → `bevy_winit` replacement target
- GPU: custom wgpu `GpuContext` (device, queue, surface, 10 pipelines) → `bevy_render` target
- UI: custom egui integration (raw input, tessellation, egui-wgpu renderer) → `bevy_egui` target
- Input: `InputEvent` → `PlatformInputQueue` → Bevy messages → `bevy_input` target
- Audio: rodio/cpal `AudioBackend` behind `Arc<Mutex<>>` → custom plugin or `bevy_audio`
- Tasks: native Tokio runtime + `spawn_background` + 5 mpsc channels → `bevy_tasks` target
- Storage: `opfs` crate (native: tokio::fs, WASM: browser OPFS) → cross-platform futures target

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

**5 mpsc channels** (`std::sync::mpsc`), drained per-frame by `poll_channels_system`:

| Channel       | Payload                                 | Purpose                          |
| ------------- | --------------------------------------- | -------------------------------- |
| Auth          | `AuthServiceMessage`                    | Sign-in/refresh/sign-out results |
| Audio Import  | `(String, Vec<u8>)`                     | Imported audio bytes             |
| Waveform      | `(String, String, WaveformLoadMessage)` | Streaming waveform peaks         |
| Audio Preload | `(String, Option<Vec<u8>>)`             | Preloaded level audio            |
| Level Import  | `Vec<u8>`                               | Imported level file bytes        |

**Background runtime:** Native: dedicated OS thread with `tokio::runtime::Builder::new_current_thread()`. WASM: `wasm_bindgen_futures::spawn_local()`.

#### WASM platform split

151+ `cfg(target_arch = "wasm32")` locations across 21 files. Heaviest:

- `eg-platform/src/auth.rs` (38): auth flow divergence
- `eg-app/src/platform/application.rs` (26): canvas vs window, touch handling
- `eg-platform/src/state_host.rs` (15): surface creation
- `eg-state/src/state/lifecycle.rs` (12): bootstrap path

#### REFACTOR POINT markers (15 remaining)

| #   | File                                     | Replacement target                                     |
| --- | ---------------------------------------- | ------------------------------------------------------ |
| 1   | `eg-app/src/platform/application.rs`     | `bevy_winit` + `App::run()`                            |
| 2   | `eg-app/src/platform/runtime.rs`         | App resources own egui/render/window directly          |
| 3   | `eg-app/src/platform/input_mapping.rs`   | `bevy_input` resources                                 |
| 4   | `eg-state/src/events/mod.rs`             | Direct Bevy message handlers over resources/components |
| 5   | `eg-state/src/events/async_results.rs`   | `bevy_tasks` streams                                   |
| 6   | `eg-state/src/events/mesh_rebuild.rs`    | `bevy_render` extraction / asset events                |
| 7   | `eg-state/src/state/test_support.rs`     | Thin `bevy_app::App` test harness                      |
| 8   | `eg-state/src/state/lifecycle.rs`        | Bevy-native window/GPU bootstrap                       |
| 9   | `eg-state/src/state/plugins.rs`          | Standard Bevy plugin graph                             |
| 10  | `eg-state/src/state/async_bridge.rs`     | `bevy_tasks` streams remove polling bridge             |
| 11  | `eg-state/src/state/audio_state.rs`      | Custom audio plugin behind Bevy systems                |
| 12  | `eg-state/src/state/render/resources.rs` | `bevy_render` render graph and assets                  |
| 13  | `eg-state/src/state/render/plugin.rs`    | `bevy_render` render graph node                        |
| 14  | `eg-game/src/events.rs`                  | Normal Bevy message/event stream                       |
| 15  | `eg-ui/src/plugin.rs`                    | `bevy_egui` systems                                    |

### Benchmark baseline

`cargo bench -p eg-app --features benchmarks` — 5 groups:

| Benchmark                           | What it measures                                | Sizes                      |
| ----------------------------------- | ----------------------------------------------- | -------------------------- |
| `editor_full_block_mesh_rebuild`    | Full `build_block_geometry`                     | 256, 1024, 4096 blocks     |
| `chunked_greedy_dense_block_mesh`   | `build_chunked_block_mesh` on dense voxels      | 4096, 16384, 65536 blocks  |
| `object_transform_triggers`         | Transform-only vs transform+rebuild             | 1024/64/8, 4096/256/16     |
| `timeline_preview_runtime`          | `TimelineSimulationRuntime::advance_to` over 4s | 1024 objects               |
| `editor_timeline_scrub_no_playback` | Single-step forward/backward scrub              | 4096 objects, 256 triggers |

> **Note:** Runtime GPU benchmarks require a display adapter. CPU-only Criterion
> benchmarks run headless. Record p50/p95 wall-clock numbers after the first
> manual benchmark run on a representative machine.

## Spike items (Phase 1 prerequisites)

Before broad migration, prove these technical risks in isolation:

### Spike 1 — Custom mesh attributes on Bevy `Mesh`

**Risk:** Bevy's `Mesh` uses fixed `MeshVertexAttribute` IDs. The project's
10-attribute `Vertex` + 5-attribute `BlockInstance` format must map to Bevy's
`MeshVertexBufferLayout` without losing the custom `render_profile`,
`animation_*`, or `color_outline` attributes.

**Approach:**

- Implement `Into<bevy::render::mesh::Mesh>` for `eg_mesh::Vertex` with
  custom vertex attributes registered via `Mesh::insert_attribute`.
- Verify the attribute layout survives Bevy's vertex buffer packing.
- Test with a minimal Bevy `Material` that reads the custom attributes in
  WGSL.

### Spike 2 — Texture-array materials via Bevy `Material`

**Risk:** The current texture-array bind group (group 3) uses
`texture_2d_array<f32>` + sampler. Bevy's `StandardMaterial` does not support
texture arrays. A custom `Material` implementation is needed.

**Approach:**

- Implement a custom `Material` (or `MaterialExtension`) that binds a
  `texture_2d_array` via `AsBindGroup`.
- Port `shader.wgsl` to Bevy's `Material` shader binding conventions
  (Bevy provides view, mesh, and material bind groups).
- Verify the 10 render profiles (normal, liquid, outline, gem, camera_trigger)
  survive the port.

### Spike 3 — Render-to-image block icons via Bevy cameras

**Risk:** Block icons are currently rendered by a dedicated wgpu render pass
(`block_icon_pipeline`) into an `Image` registered with egui. Bevy cameras
rendering into `Image` assets + `EguiUserTextures` must replace this.

**Approach:**

- Create a Bevy `Camera` with `RenderTarget::Image` targeting a
  `bevy::asset::Handle<Image>`.
- Register the image with `EguiUserTextures` for egui display.
- Render a single block mesh into the image and verify it appears in egui.

### Spike 4 — Native OPFS futures on Bevy task pools

**Risk:** `opfs` native backend uses `tokio::fs` which requires a Tokio
reactor. Bevy's `IoTaskPool` uses `async-executor`, not Tokio. Background
file I/O must either:

- (a) Keep the dedicated Tokio thread from `eg-platform/src/task.rs`, or
- (b) Replace `opfs` native backend with a `async-fs`/`smol` backend that
  runs on `IoTaskPool`, or
- (c) Bridge Tokio futures into Bevy tasks via `spawn_blocking`.

**Approach:**

- Test option (b): swap `opfs` native backend to `async-fs` and run on
  `IoTaskPool`. Verify OPFS directory creation, file read/write, and
  `app_specific_dir` behavior.
- If option (b) fails, test option (c) with a Tokio bridge.

### Spike 5 — `bevy_egui` multi-pass integration

**Risk:** The current egui integration manually handles raw input, tessellation,
texture upload, and render pass submission. `bevy_egui 0.41` uses
`EguiPrimaryContextPass` with a multi-pass schedule. The project's 60+ resource
reads at the egui boundary must survive the transition.

**Approach:**

- Install `bevy_egui 0.41` alongside the existing egui context.
- Verify that `EguiPrimaryContextPass` produces equivalent output to the
  current manual tessellation path.
- Test that `EguiUserTextures` can display Bevy `Image` assets (for block
  icons).

### Spike 6 — Bevy `Time<Virtual>` frame pacing parity

**Risk:** The current `FrameTimeRes` derives delta from `Time<Virtual>` with
exponential FPS smoothing and a 0.25s clamp. Verify that Bevy's frame pacing
produces equivalent values to the existing `web_time::Instant` fallback.

**Approach:**

- Log `FrameTimeRes.delta_seconds` and `fps_smoothed` from both the Bevy
  source and the `Instant` fallback over 1000 frames.
- Verify the values converge within 1ms.

## Acceptance gates (Phase 0 → Phase 1 transition)

- [x] All 841 tests pass
- [x] Clippy `-D warnings` clean
- [x] Format + license clean
- [x] Benchmarks compile
- [x] Architecture documented (this section)
- [x] Spike items identified (6 spikes above)
- [ ] Spike 1–5 proven in isolation (Phase 1 work)
- [ ] Benchmark p50/p95 recorded on representative machine
- [x] WebGPU + forced-WebGL smoke test (manual: `bun run build:wasm`, open in Chrome/Firefox)
