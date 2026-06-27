# Bevy ECS Migration — Refactor Point Registry

> Living document. Every site that will eventually be replaced by a full-Bevy
> feature has a `// REFACTOR POINT(full-bevy):` comment in the code AND a row in
> the table below. Remove both together when the swap lands.

## Strategy

**Now (Phase 0–9):** ground-up rewrite of `eg-game` + `eg-state` + `eg-ui`
around a single `bevy_app::App` holding one shared `bevy_ecs::World`. The custom
wgpu 29 / egui-wgpu / rodio/cpal stack is preserved behind an extraction +
present plugin. The winit loop drives `app.update()` per frame.

**Later (full Bevy):** drop the custom wgpu / egui-wgpu / winit plumbing in
favor of the Bevy umbrella. The migration is designed to be mechanical.

## Constraints

- `bevy_ecs 0.19`, `bevy_app 0.19`, `bevy_time 0.19`, `bevy_tasks 0.19` (matches
  the existing `bevy_clipboard 0.19` workspace dep).
- `nightly-2026-03-12` toolchain, `edition = "2024"`, `wasm32-unknown-unknown`
  must remain green.
- Clippy `-D warnings` is a hard gate (`bun run lint`).
- Puffin profiling must keep working; `puffin::profile_scope!("name")` wraps
  each `SystemSet`.

## Refactor points (full-Bevy future)

| #   | Site (code)                                                                                                          | Current                                                                 | Full-Bevy replacement                                                                |
| --- | -------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| 1   | `crates/eg-app/src/platform/application.rs` — winit `ApplicationHandler`                                             | Custom winit event loop calls `Runtime::run_frame` on `RedrawRequested` | `bevy_winit` plugin + `App::run()`                                                   |
| 2   | `crates/eg-app/src/platform/pipeline.rs` — manual `app.update()`                                                     | We call `bevy_app::App::update()` ourselves each frame                  | `ScheduleRunnerPlugin` drives the loop                                               |
| 3   | `crates/eg-state/src/state/render/` (post Phase 4) — `GpuContextRes` / `SceneMeshesRes` + `ExtractSet` / `RenderSet` | Custom wgpu 29 + egui-wgpu + hand-rolled WGSL present                   | `bevy_render` render graph, `Mesh` / `Material` assets, `shader.wgsl` as `ShaderRef` |
| 4   | `crates/eg-ui/src/plugin.rs` (post Phase 5) — `EguiContext` resource + `ctx.run_ui` inside UI systems                | Per-frame resource + manual `egui::Context` call                        | `bevy_egui` plugin                                                                   |
| 5   | `crates/eg-state/src/state/lifecycle.rs` (post Phase 6) — mpsc channel bridges + `PollChannelsSet`                   | `(Sender, Receiver)` in resources drained each frame                    | `bevy_tasks` async runtime, Bevy asset loading, `async-compat` for I/O               |
| 6   | `crates/eg-game/src/resources.rs` (post Phase 1) — `GameClock` + `FixedStepConfig` + manual fixed-step loop          | We tick `GameClock` ourselves in `advance_player_system`                | `bevy_time` `Time<Fixed>` + `FixedUpdate` schedule                                   |
| 7   | `crates/eg-state/src/state/audio_state.rs` (post Phase 6) — `AudioBackendRes` (rodio/cpal)                           | Custom audio backend resource                                           | Evaluate `bevy_audio` (flag, not forced — keep custom if needed)                     |
| 8   | `crates/eg-app/src/platform/runtime.rs` (post Phase 7) — `SurfaceHostRes` (window / canvas)                          | Hand-rolled wgpu surface + winit window                                 | `bevy_window` + `bevy_window::Window` entities                                       |
| 9   | `crates/eg-app/src/platform/input_mapping.rs` — winit events → `InputEvent` → `InputSet`                             | We translate winit events to our own enum                               | `bevy_input` `Input<KeyCode>` / `ButtonInput<MouseButton>` / `Gamepad`               |
| 10  | `crates/eg-state/src/state/command_dispatch.rs` + `editor_dispatch.rs` (post Phase 3)                                | One `match` per command enum drains both queues                         | N/A — Bevy `EventReader<T>` is the replacement and these files are deleted           |

## Event taxonomy (post Phase 3)

`AppCommand` (15 variants) + `EditorCommand` (~87 variants) fragment into Bevy
`Event` structs, one per semantic command. 12 input-state variants
(`SetShiftHeld` / `SetCtrlHeld` / `SetAltHeld` / `SetPan{Up,Down,Left,Right}Held`

- `MouseButton` / `PrimaryClick` / `PointerMoved` / `UpdateCursorFromScreen` /
  `CameraDrag`) are **not** events — an `InputSet` system writes an `EditorInput`
  resource directly from winit input. That makes for ~85 events + ~12 direct
  resource writes. See the system-set graph below for ordering.

## System set graph (post Phase 7)

```
Per-frame schedule order:
  PollChannelsSet     // drain mpsc receivers → async-result events
  ModeChangeSet       // AppPhase / EditorMode transitions
  PreCommandHistorySet// snapshot ObjectBundle before mutating handlers
  ClickHandlingSet    // pointer → editor interactions
  InputSet            // winit input → EditorInput resource
  CommandHandlerSet   // EventReader<*> → mutating systems
  SimulationSet       // eg-game systems (advance / collide / consume / trail / complete)
  UiSet               // eg-ui systems (direct-manipulation also writes here)
  HistorySet          // post-mutation history finalization
  RebuildMeshSet      // Changed<Position|Rotation|Size> → mesh buffers
  ExtractSet          // queries → SceneMeshesRes
  RenderSet           // wgpu present
```

Cross-domain ordering constraints (sample):

- `ModeChangeSet.before(ClickHandlingSet)`
- `PreCommandHistorySet.before(CommandHandlerSet)`
- `CommandHandlerSet.before(RebuildMeshSet)`
- `SimulationSet.before(ExtractSet)`
- `UiSet.before(ExtractSet)`
- `ExtractSet.before(RenderSet)`

## Phase checklist

- [x] **Phase 0** — add `bevy_ecs` / `bevy_app` / `bevy_time` / `bevy_tasks` deps; verify native + WASM + clippy; create this doc; tag `REFACTOR POINT` convention in `AGENTS.md`.
- [x] **Phase 1** — `eg-game` ECS core (components, resources, query-based systems, `GamePlugin`).
- [x] **Phase 2** — `eg-state` ECS shell (single shared `World`, plugins, `AppPhase` `States`).
- [x] **Phase 3** — command/event fragmentation.
- [x] **Phase 4a** — rendering extraction scaffolding (resources + mirror bridge + `RenderPlugin`).
- [x] **Phase 4b-1** — mirror `RenderFrameInput` snapshot into `RenderFrameInputRes` (editor phase + sky color + gamma toggle coverage).
- [x] **Phase 4b-2** — mirror POD `RenderUniformsRes` (CameraUniform + LineUniform + ColorSpaceUniform) into the world; covers Menu + Editor phase paths.
- [ ] **Phase 4b-3+** — extend the mirror to Playing / GameOver / editor playtest, then move `GpuContext` / `SceneMeshes` / `SurfaceHost` ownership into resources and replace `rebuild_*_mesh` with systems.
- [ ] **Phase 5** — UI as Bevy systems.
- [x] **Phase 6a** — async bridging scaffold (5 channel wrapper resources + `PollChannelsSet` + 5 result events).
- [x] **Phase 6b** — replace legacy `update_*` methods with handler systems that consume result events.
- [ ] **Phase 7** — `App` wiring (winit drives `app.update()`).
- [ ] **Phase 8** — test harness migration (`TestApp` builder).
- [ ] **Phase 9** — documentation pass.

## Phase 1 — completed

`eg-game` now has:

- `components.rs` — `PlayerBundle`, `ObjectBundle`, `PlayState`, `Selected`, `Hovered` markers, plus standalone player components (`PlayerPosition`, `PlayerDirection`, `PlayerSpeed`, `VerticalVelocity`, `Grounded`, `Trail`, `SpawnPosition`, `SpawnDirection`) and object components (`Object`, `ObjectId`, `ObjectData`).
- `resources.rs` — `GameClock`, `PlayerProgress`, `SpatialIndex`, `PendingTurnInputs`, `FixedStepConfig`, `FrameDelta`, `DeathSfxFlag`, `GameRuntimeTuning`, `LevelMeta`.
- `events.rs` — `ConsumedObjectEvent` + `ConsumedEventsBuffer` resource.
- `systems.rs` — 8 query-based systems organized in `GameplaySet` (`TurnInput`, `AdvancePlayer`, `Collision`, `ConsumeObjects`, `GroundGravity`, `Trail`, `Completion`, `Progress`).
- `plugin.rs` — `GamePlugin` (registers all resources + the schedule).
- `state.rs::GameState` — backwards-compatible wrapper. The legacy `pub` fields are kept in sync via `sync_from_world()` after the schedule runs. `update(dt)` sets `FrameDelta(dt)` and runs the schedule once per call.
- `simulation.rs::TimelineSimulationRuntime` — still uses the wrapper. Will be refactored in Phase 2 / Phase 7 to share a `World`.

**Verification:** 19/19 `eg-game` tests pass, `cargo clippy -- -D warnings` is clean, `eg-state` and `eg-app` compile.

## Phase 1.1 — bugfix: `ground_gravity_system` + `trail_system` regressions

The initial Phase 1 system port was not a behavior-preserving rewrite. Two
silent regressions were caught by `eg-state` integration tests (`main` is
green, `ecs-migration` was failing 4 tests):

1. **`ground_gravity_system` was missing the broad-phase XZ overlap filter.**
   The legacy `top_surface_y_under_indices` calls
   `aabb_overlaps_cuboid_xz(player_xz, obj, cuboid)` on every cuboid before
   considering it. The ECS port relied solely on the spatial grid, so a
   support block sitting in the same grid cell as the player but past them
   in x/z would still be treated as ground support. Symptom: airborne test
   (`playback_trail_vertices_include_airborne_head_segment`) — the player
   runs past a stone at `[0, 2, 0]` but never falls because the system
   keeps reporting `grounded = true` even after the player leaves the
   support's footprint.

2. **`trail_system` was branching on `grounded.0` without tracking the
   previous value.** Both the airborne and grounded branches pushed a new
   segment, so every `Changed<Grounded>` tick grew the trail by 2
   single-point segments. With 481 cached samples the trail ended up as
   481 segments of 1 point each, which made the editor scrub trail
   (which only draws a "head" line if the snapshot's last point is
   different from the player's current position) collapse to zero
   vertices. Symptom: 3 editor / scrub-trail tests failed with empty
   vertex buffers. Fix: track `was_grounded` per entity in a
   `Local<HashMap<Entity, bool>>` and mirror the legacy transition logic
   (grounded→airborne: append to last segment, deduping within 1e-3;
   airborne→grounded: start a new segment anchored at the landing pos).

**Verification:** all 4 previously-failing tests now pass; full
`cargo test --workspace` is green; `cargo clippy -p eg-game --all-targets -- -D warnings` is clean.

## Phase 3 — command / event fragmentation

AppCommand (15 variants) + EditorCommand (~87 variants) are
fragmented into a per-semantic Bevy Message type under
crates/eg-state/src/events/. The legacy enum-to-method dispatch path
(State::dispatch / State::dispatch_editor) is **unchanged** — the
new event bus feeds the legacy path through a PendingCommands
resource + the StateSchedule (see crates/eg-state/src/state/mod.rs).
This keeps the existing test suite green while introducing the new
event taxonomy. Future phases migrate UI / winit / egui code to write
Message<T>::write directly, then delete the legacy enums + dispatch
files in one final sweep.

### What's in Phase 3 today

- 15 event submodules under events/ (
  avigation, auth,
  editor_mode, snap, block, selection, imeline, iming,
  bpm, spawn, history, camera, misc, session,
  input_state, pointer).
- One Message struct per semantic command (~85 distinct message
  types).
- A ToLegacy trait on each message type that converts the message to
  a LegacyCommand (an AppCommand or EditorCommand enum variant).
- A per-message drain system (one `fn handle_*`(MessageReader<T>,
  ResMut<PendingCommands>) per message type) that stages commands in
  the PendingCommands resource.
- A StateSchedule that bundles the per-message drain systems.
- State::tick_state now runs the schedule and forwards
  PendingCommands to State::dispatch / State::dispatch_editor.
- A HistorySnapshot marker event for direct-manipulation UI (gizmo
  drag / marquee / block drag) that reserves the PreCommandHistorySet
  slot in the schedule. A follow-up phase wires a real
  pre-mutation history snapshot system to it.

### Bevy 0.19 API notes

Bevy 0.19 uses Message (not Event) for buffered per-frame
payloads; observer-based Events fire immediately. We use
Message here because we want pull-based consumption by the
per-message drain systems in [events::CommandSet], then a single
drain into [events::PendingCommands] at the end of the frame. The
full Bevy 0.19 pattern is:

- #[derive(Message)] for the message type.
- Messages<T> for the resource (added by app.add_message::<T>()).
- MessageReader<T> / MessageWriter<T> for system params.
- world.write_message(event) to send.
- world.resource_mut::<Messages<T>>() to inspect the buffer.

### Event taxonomy table

| Module | Count | Examples |
| ------ | ----- | -------- |

|
avigation | 10 | TurnRight, ToggleEditor, GameSetPracticeMode |
| auth | 5 | AuthSubmitSignIn, AuthSignOut, ResizeSurface |
| editor_mode | 5 | SetEditorMode, SetBlockId, PickBlockAt |
| snap | 4 | SetSnapToGrid, SetSnapStep |
| block | 5 | RemoveBlock, CopyBlock, UpdateSelectedBlock |
| selection | 3 | SnapSelectionToGrid, AddTransformTrigger |
| imeline | 14 | ToggleTimelinePlayback, SetTimelineTime, Playtest|
| iming | 6 | AddTimingPoint, SetTimingPointTime |
| bpm | 2 | BpmTap, BpmTapReset |
| spawn | 3 | SetSpawnHere, RotateSpawnDirection |
| history | 3 | Undo, Redo, HistorySnapshot |
| camera | 5 | AdjustZoom, SetCameraOrientation |
| misc | 4 | ToggleHitboxVisualization, Escape |
| session | 22 | LoadLevel, RenameLevel, UpdateMusic |
| input_state | 7 | SetShiftHeld, SetPanUpHeld (carve-out — see below) |
| pointer | 5 | MouseButton, PrimaryClick (carve-out — see below) |
| **Total** | **~102** | Plus 7 in input_state + 5 in pointer are deprecated |

### Input-state carve-out

Per the migration plan, the 7 input*state::* events and 5
pointer::_ events are **not** commands — they are \_input state_.
A follow-up phase replaces them with direct writes to the
EditorInput resource from a dedicated InputSet system. For now
they remain routed through dispatch_editor to preserve test parity
with the legacy EditorCommand::SetShiftHeld /
EditorCommand::MouseButton etc. variants.

REFACTOR POINT(full-bevy): the input-state + pointer events
disappear in the full-Bevy migration; the corresponding bevy_input
Input<KeyCode> / ButtonInput<MouseButton> resources handle
held-state + click events natively.

### Schedule graph (post Phase 3)

`Per-frame schedule order (StateSchedule):
  transitions::phase_transition_system   // AppPhase change detection
  events::handle_* (per-message)         // MessageReader<T> → PendingCommands
  dispatch_pending_commands              // PendingCommands → State::dispatch[_editor]`

Future phases add more sets at the top of the chain (InputSet,
ClickHandlingSet, ModeChangeSet, PreCommandHistorySet,
CommandHandlerSet, SimulationSet, UiSet, HistorySet,
RebuildMeshSet, ExtractSet, RenderSet). Phase 3 keeps the
schedule minimal so the existing legacy dispatch path is exercised
end-to-end on every frame.

**Verification:** cargo test --workspace is green; the new
ick_state_drains_message_into_pending_then_dispatches test
exercises the full pipeline (write a Message →
State::tick_state → side effect on the legacy State).
14 new unit tests in events::tests cover ToLegacy conversion +
PendingCommands drain semantics for representative event types.

## Phase 4a — rendering extraction scaffolding

The plan calls for lifting `RenderSubsystem` (`crates/eg-state/src/state/render/mod.rs`) into Bevy resources (`GpuContextRes`, `SceneMeshesRes`, `SurfaceHostRes`, `CameraUniform`, `LineUniform`, `ColorSpaceUniform`) plus an `ExtractSet` / `RenderSet` system pair. Phase 4a is the **non-invasive scaffold** that establishes the boundary without touching the legacy `State::render` field (which has ~440 call sites).

### What's in Phase 4a today

- **New resource types** in `crates/eg-state/src/state/render/resources.rs`:
    - `RenderUniformsRes` — POD bundle of `CameraUniform` + `LineUniform` + `ColorSpaceUniform` (the per-frame uniform values).
    - `RenderFrameInputRes` — POD copy of `RenderFrameInput` (phase, editor mode, sky color, grid center, gamma toggle).
    - `RenderResizeRequest` — platform-side resize queue (consumed by a future `apply_resize_surface_system`).
    - `RenderGpuRes`, `RenderMeshesRes`, `RenderSurfaceHostRes` — heavy types **defined** but **not yet inserted** (Phase 4b/4c).
- **`RenderPlugin`** in `crates/eg-state/src/state/render/plugin.rs` registers the POD resources and exposes the `RenderSet` enum (`RenderSet::Extract`, `RenderSet::Present`).
- **Mirror bridge** in `crates/eg-state/src/state/render/mirror.rs`: a thread-local `RefCell<Option<MirrorFn>>` that `State::tick_state` installs into each frame and the `mirror_pod_state_via_bridge` system drains. The closure captures only `Copy` values (the current `RenderFrameInput` snapshot) so the `!Send` `State` doesn't leak into the schedule.
- **`State::compute_frame_input_snapshot`** builds the per-frame `RenderFrameInput` from the legacy `State.phase` / `State.editor.input.mode` / `State.session.*_sky_color` fields. The mirror closure captures this snapshot.
- **Schedule wiring**: `register_state_command_pipeline` (in `state::plugins`) chains `add_mirror_render_systems` after `add_command_systems`. The mirror bridge system runs every frame the schedule runs.
- **Tests** in `state::tests` (4 new):
    - `tick_state_mirrors_frame_input_into_render_resource` — end-to-end: write `state.phase = Editor`, `tick_state`, assert `RenderFrameInputRes.0.phase == Editor`.
    - `tick_state_registers_render_pod_resources_on_construction` — every `State` World has the POD resources.
    - `tick_state_does_not_insert_heavy_render_resources_yet` — guards the Phase 4a contract: heavy resources are defined but not inserted.
    - `tick_state_mirror_bridge_consumes_closure` — the schedule drains the installed closure.

### What Phase 4b-1 and Phase 4b-2 add (mirror extends to all four `AppPhase` values)

The original Phase 4b plan ("move `State::render.gpu` ownership into `RenderGpuRes` and replace `self.render.gpu.X` reads with `world.resource::<RenderGpuRes>().0.X` across ~440 call sites") is a single big-bang commit that would touch every read site. The 440-site cost comes from the fact that `State::render.gpu` exposes raw `wgpu::Device` / `wgpu::Queue` / `wgpu::Buffer` handles — the values can't be cheaply mirrored, they need full ownership transfer.

`RenderUniformsRes` and `RenderFrameInputRes` are different. They're tiny POD types (`Copy`, no wgpu handles, no `MeshSlot`s). We can mirror them into the world without touching `State::render` at all, and Bevy systems can already read them. So Phase 4b splits into incremental mirror-driven sub-phases:

- **Phase 4b-1** (commit `0a4dec3`) — mirror `RenderFrameInput` (phase + editor mode + sky color + grid center + gamma toggle) into `RenderFrameInputRes`. The `compute_frame_input_snapshot` helper reads from the legacy `State.phase` / `State.editor.input.mode` / `State.session.*_sky_color` fields and produces a `RenderFrameInput`. The mirror closure installed in `State::tick_state` captures the snapshot. Test: `tick_state_mirrors_frame_input_into_render_resource` writes `state.phase = Editor` + `state.session.editor_sky_color = [0.25, 0.5, 0.75]`, runs `tick_state`, and asserts the resource reflects all three values.

- **Phase 4b-2** (this commit) — mirror POD `RenderUniformsRes` (CameraUniform + LineUniform + ColorSpaceUniform) into the world. The `compute_render_uniforms_snapshot` helper computes the per-frame uniform values for **all four `AppPhase` values** (Menu preview camera, Editor pan + target + offset, Playing follow target, GameOver same as Playing). The mirror closure captures the snapshot. Tests:
    - `tick_state_mirrors_render_uniforms` (Menu path) — asserts the camera position is the menu preview camera and the viewport size is the current `RenderSubsystem.gpu.config`.
    - `tick_state_mirrors_render_uniforms_in_editor` (Editor non-playtest path) — sets `editor_pan = [4.0, 8.0]` and `editor_target_z = 6.0`, calls `tick_state`, then asserts the camera position and `view_proj` matrix match `expected_eye = target + editor_camera_offset()` to 1e-5 (the same epsilon the legacy math uses).

Both tests run the same closure that production code runs, and the test computes the expected value by calling the same `editor_camera_offset()` helper — no duplicated math in the test. The mirror is non-invasive: the legacy `State::render` field is unchanged, the 440 call sites are untouched, and the GPU buffers still live on `RenderSubsystem.gpu`. The only thing that changed is that `RenderUniformsRes` now reflects real data, not the `Default::default()` identity matrix.

### What Phase 4b-3+ / 4c / 4d / 4e do (next)

- **Phase 4b-3** mirrors the **Playing** + **GameOver** paths + the **editor playtest** path (where `editor_is_playing()` returns `true` and the camera pose comes from `editor_preview_camera_view`). These paths already work in `compute_render_uniforms_snapshot` — the only missing piece is tests pinning them.
- **Phase 4b-4** wires the per-frame `time_seconds` from `frame_runtime.global_time_seconds` to the `ColorSpaceUniform.time_seconds` field and adds a test for the gamma-toggle bit.
- **Phase 4c** moves `State::render.meshes` + `surface_host` into `RenderMeshesRes` / `RenderSurfaceHostRes`. The `MeshSlot` types stay as-is; only the storage location changes.
- **Phase 4d** replaces the legacy `rebuild_*_mesh` call sites in `state/editor_scene.rs` with `RebuildMeshSet` systems that read `Changed<Position|Rotation|Size>` components and emit mesh buffers into `RenderMeshesRes`. This unlocks deleting `EditorDirtyFlags` entirely.
- **Phase 4e** adds an actual wgpu `present_frame_system` in `RenderPresentSet` that drains `RenderFrameInputRes` + `RenderMeshesRes` + `RenderGpuRes` + `RenderUniformsRes` and submits the render pass. The current `State::render()` call from `eg-app/src/platform/pipeline.rs::render_frame` is removed in the same commit.

### Schedule graph (post Phase 4a)

`Per-frame schedule order (StateSchedule):
  transitions::phase_transition_system        // AppPhase change detection
  mirror::install_mirror                      // tick_state installs the closure
  events::handle_* (per-message)              // MessageReader<T> → PendingCommands
  render::mirror_pod_state_via_bridge         // drain closure → RenderUniformsRes / RenderFrameInputRes
  render::present_render_frame_placeholder    // no-op in Phase 4a
  dispatch_pending_commands                   // PendingCommands → State::dispatch[_editor]`

### Resource boundary

```text
State.phase            ──┐
State.editor.input.mode ─┤  compute_frame_input_snapshot
State.session.sky_color ┘            │
                                       ▼
                          ┌─────────────────────┐
                          │ RenderFrameInputRes  │  (POD, Copy, mirrored each frame)
                          └─────────────────────┘
                                       ▲
                          ┌────────────┴────────────┐
                          │  mirror_pod_state_      │
                          │  via_bridge (system)    │
                          └─────────────────────────┘
```

The legacy `State::render` field is still authoritative. Future phases (4b/4c) drain the data ownership from `State` into the world; Phase 4a's mirror is the bridge that lets Bevy systems read the values today without touching `State::render`.

### Bevy 0.19 API notes (Phase 4a)

- `#[derive(Resource)]` works for both `Copy` POD types and the heavy `GpuContext` / `SceneMeshes` wrappers (the heavy types are `Send + Sync` in wgpu 22+, so they are valid `Resource`s).
- The thread-local mirror slot is `Send`-unsafe by design — the closure must be `Send` only if Bevy moves the schedule to another thread. Today the schedule runs on the main thread; the thread-local is the simplest correct implementation.
- `ResMut<T>` and `Res<T>` from `bevy_ecs::prelude::*` are the system params; `World::resource_scope` is the test-side helper for borrowing two `ResMut`s simultaneously.

## Phase 6a — async bridging scaffold

The plan calls for wrapping the five `mpsc` channels the legacy `State` struct owns (auth service, audio import, waveform load, audio preload, level import) in Bevy `Resource`s and adding a `PollChannelsSet` system that drains them and emits per-channel result events. Phase 6a is the **non-invasive scaffold** that establishes the boundary without disturbing the legacy `update_*` methods (Phase 6b replaces them with handler systems that consume the events).

### What's in Phase 6a today

- **5 new event types** in `crates/eg-state/src/events/async_results.rs`:
    - `AuthServiceResultEvent` — `AuthServiceMessage` payload.
    - `AudioImportResultEvent` — `(filename, Option<Vec<u8>>)` payload.
    - `WaveformResultEvent` — `(source, level_name, WaveformLoadMessage)` payload. **Not `Clone`** because `WaveformLoadMessage` doesn't implement `Clone`; consumers receive by value.
    - `AudioPreloadResultEvent` — `(source_key, Option<Vec<u8>>)` payload.
    - `LevelImportResultEvent` — `Vec<u8>` payload.
- **5 channel wrapper resources** in `crates/eg-state/src/state/async_bridge.rs`:
    - `AuthChannelRes(Mutex<Receiver<AuthServiceMessage>>)`
    - `AudioImportChannelRes(Mutex<Receiver<(String, Vec<u8>)>>)`
    - `WaveformLoadChannelRes(Mutex<Receiver<WaveformResult>>)`
    - `AudioPreloadChannelRes(Mutex<Receiver<AudioPreloadResult>>)`
    - `LevelImportChannelRes(Mutex<Receiver<Vec<u8>>>)`
      Each wraps the receiver in a `Mutex` so it can sit in a Bevy `Resource` (which requires `Send + Sync + 'static`). The `Mutex` is uncontended in practice — only the `poll_channels_system` ever locks it.
- **`PollChannelsSet`** system set + **`poll_channels_system`** that drains every receiver once per frame and emits the corresponding result event. The system takes `Option<Res<...>>` for each wrapper so it works whether or not the wrapper is installed.
- **Schedule wiring**: `register_state_command_pipeline` (in `state::plugins`) chains `add_poll_channels_systems` after `add_command_systems` and `add_mirror_render_systems`. The poll system runs once per frame alongside the Phase 3 message drain systems and the Phase 4a render mirror bridge.
- **Tests** (3 integration + 8 unit + 5 module = 16 new tests):
    - `tick_state_poll_channels_emits_audio_import_result_event` — end-to-end: write to a channel, `tick_state`, assert the event is in `Messages<AudioImportResultEvent>`.
    - `tick_state_poll_channels_emits_level_import_result_event` — same for level import.
    - `tick_state_poll_channels_emits_auth_service_result_event` — same for auth service.
    - 7 unit tests in `events::async_results::tests` cover the event constructors + payload preservation.
    - 5 unit tests in `state::async_bridge::tests` cover `drain_one` (Empty/Disconnected handling), `register_state_async_bridge_world` (resource initialization), and `install_async_bridge_from_state` (idempotency).

### Design notes

- **Why messages and not direct resource writes?** — distinct `Message<T>` types give each handler its own `MessageReader<T>`, so multiple readers of the same channel can be added without cross-reader nondeterminism. Tests can write a synthetic event into the `Messages<...>` buffer and assert the handler behavior without going through the platform layer.
- **Why a `Mutex<Receiver<T>>` wrapper?** — `mpsc::Receiver<T>` is `Send` but not `Sync` (its `try_recv` API is `&self` but the value can't be shared between two consumers without a lock). Bevy `Resource` requires `Send + Sync + 'static`. The wrapper's `Mutex` is uncontended in practice.
- **Why is the install not auto-wired?** — `mpsc::Receiver<T>` is not `Clone`. Moving it from the legacy `State` fields into the World would break the legacy `update_*` methods that also drain the same channel. Phase 6b moves the legacy fields to `Arc<Mutex<Receiver<T>>>` shared between the State and the World, then replaces the legacy drains with handler systems. Until then, the new `poll_channels_system` runs once per frame and emits events for any messages sitting in the resource's receiver; tests that exercise the bridge insert the resource manually.
- **Idempotent install** — `install_async_bridge_from_state` is a no-op for already-installed resources, so calling it from multiple frames is safe.

### Schedule graph (post Phase 6a)

`Per-frame schedule order (StateSchedule):
  transitions::phase_transition_system        // AppPhase change detection
  mirror::install_mirror                      // tick_state installs the closure
  events::handle_* (per-message)              // MessageReader<T> -> PendingCommands
  render::mirror_pod_state_via_bridge         // drain closure -> RenderUniformsRes / RenderFrameInputRes
  render::present_render_frame_placeholder    // no-op in Phase 4a
  async_bridge::poll_channels_system          // drain 5 receivers -> 5 result events
  dispatch_pending_commands                   // PendingCommands -> State::dispatch[_editor]`

### Bevy 0.19 API notes (Phase 6a)

- `#[derive(Message)]` is used for the 5 new event types. `WaveformResultEvent` is intentionally **not** `Clone` because `WaveformLoadMessage` doesn't implement `Clone`. The other 4 events derive `Clone` for ergonomic test usage.
- `Mutex<T>` is `Send + Sync` when `T: Send`, so the channel wrappers satisfy Bevy's `Resource` bound. The receivers are owned by value (no `Arc`); the wrappers are inserted at most once per world (idempotent install).
- The `poll_channels_system` takes `Option<Res<...>>` for each wrapper so it works whether or not the wrapper is installed. This is the same pattern Bevy uses for optional system params and avoids the need for separate "registered" / "unregistered" code paths.

## Phase 6b — async result handler systems

Phase 6a established the channel wrappers and the `poll_channels_system` that emits 5 result events. Phase 6b is the **handler half**: 5 Bevy systems consume the events and apply them to the legacy `State`, plus a thread-local bridge that lets the systems mutate the `!Send` `State` without leaking it into the schedule.

### What's in Phase 6b today

- **Channel field type change.** The 5 legacy channel fields change from `(Sender, Receiver<T>)` to `(Sender, Arc<Mutex<Receiver<T>>>)`:
    - `State::auth.channel: (Sender<AuthServiceMessage>, SharedAuthReceiver)`
    - `State::audio.state.editor.audio_import_channel: (Sender<(String, Vec<u8>)>, SharedAudioImportReceiver)`
    - `State::audio.state.editor.waveform_load_channel: (Sender<WaveformResult>, SharedWaveformReceiver)`
    - `State::audio.state.runtime_preload.preload_channel: (Sender<AudioPreloadResult>, SharedAudioPreloadReceiver)`
    - `State::session.editor_level_import_channel: (Sender<Vec<u8>>, SharedLevelImportReceiver)`
      The `Arc<Mutex<>>` wrapper is `Clone + Send + Sync` so both the State and the World can hold a clone. `try_recv` is `&self`; the first caller wins (the second sees an empty receiver). This eliminates the double-process race that Phase 6a's parallel poll + drain design had.

- **5 channel resources updated.** The `AuthChannelRes` / `AudioImportChannelRes` / `WaveformLoadChannelRes` / `AudioPreloadChannelRes` / `LevelImportChannelRes` resources now hold the **shared** `Shared*Receiver` (one less `Mutex` layer — the inner `Mutex` is shared with the legacy field). The 5 type aliases are exported from `state::async_bridge`.

- **5 handler systems** in `crates/eg-state/src/state/async_bridge.rs`:
    - `auth_handler_system` — consumes `AuthServiceResultEvent`, installs a closure that routes the `AuthServiceMessage` to `State::complete_auth_sign_in` / `complete_auth_refresh` / `complete_auth_sign_out` (now `pub(crate)`).
    - `audio_import_handler_system` — consumes `AudioImportResultEvent`, installs a closure that mirrors the body of `State::update_audio_imports` (insert into `EditorAudioCache`, reload waveform).
    - `waveform_handler_system` — consumes `WaveformResultEvent`, installs a closure that mirrors the body of `State::update_waveform_loading` (per `WaveformLoadMessage` variant: `Started` / `Chunk` / `Cached` / `Finished` / `Failed`). Because `WaveformResultEvent` is not `Clone`, this system takes `&mut World` and `drain`s the `Messages<>` buffer directly.
    - `audio_preload_handler_system` — consumes `AudioPreloadResultEvent`, installs a closure that mirrors the body of `State::update_runtime_audio_preloads` (remove from `preloading_source_keys`, insert into `preloaded_audio`).
    - `level_import_handler_system` — consumes `LevelImportResultEvent`, installs a closure that calls `State::import_level_egz` / `State::import_level` (with the same error logging as the legacy `update_level_imports`).

- **`ApplyAsyncResultsSet`** system set + `add_apply_async_results_systems` registration helper.

- **Thread-local bridge**: `install_async_result_fn(F: FnOnce(&mut State) + 'static)` + `drain_pending_async_result_fns(state: &mut State)`. The handler systems install `FnOnce(&mut State)` closures into a `thread_local!` `RefCell<Vec<Box<dyn FnOnce(&mut State)>>>` slot. `State::tick_state` drains the slot and applies each closure after the schedule finishes. This is the same pattern as Phase 4a's `render::mirror`.

- **`install_async_bridge_from_state_arc`** helper. `State::tick_state` calls this on every frame to wire the 5 `Arc<Mutex<Receiver<T>>>` fields into the World's resources (idempotent — no-op if already installed). This replaces the Phase 6a `install_async_bridge_from_state` (which took owned `Receiver<T>` and is still used by tests that build a world from scratch with fresh channels).

- **Visibility bumps**:
    - `State::complete_auth_sign_in` / `complete_auth_refresh` / `complete_auth_sign_out` made `pub(crate)` so the auth handler system can call them from a closure.
    - `state::audio_state::persist_waveform_cache_entry` made `pub(crate)` so the waveform handler's `apply_waveform_message` helper can call it from a closure.

- **Tests** (15 new unit + 0 integration = 15 new tests):
    - `install_async_bridge_from_state_arc_shares_receivers` — `Arc::ptr_eq` confirms the resource holds the same `Arc` we passed in.
    - `install_async_bridge_from_state_arc_is_idempotent` — second install with fresh receivers doesn't overwrite the first.
    - `async_result_fn_slot_drains_in_order` — the thread-local slot drains closures in insertion order.
    - `async_result_fn_slot_drain_is_idempotent` — second drain with no new closures is a no-op.
    - `auth_handler_system_routes_to_complete_sign_out` — write `SignedOut(Ok(()))`, run system, drain slot, verify `State.auth.session = None` and `auth.pending = false`.
    - `audio_import_handler_system_runs_with_cancellation` — write `bytes = None`, run system, verify no panic.
    - `waveform_handler_system_drains_events` — write `Failed` event, run system (drains the buffer), verify no panic.
    - `audio_preload_handler_system_inserts_bytes` — write `Some(bytes)`, run system, verify `preloaded_audio` contains the entry.
    - `audio_preload_handler_system_handles_failure` — write `None`, run system, verify nothing inserted.
    - `level_import_handler_system_is_safe_on_invalid_bytes` — write invalid bytes, run system, verify no panic.

- **Existing test updates** (no behavior change): the 3 Phase 6a integration tests (`tick_state_poll_channels_emits_*`) now insert `Arc<Mutex<Receiver<T>>>` resources instead of `Mutex<Receiver<T>>` to match the new resource type.

### Design notes

- **Why a thread-local bridge?** `State` is `!Send` (it owns `mpsc::Receiver<T>` fields, even when wrapped — see `Arc<Mutex<Receiver<T>>>: !Send` because `MutexGuard` is `!Send`). Bevy schedules run on the main thread today; the thread-local slot is the simplest correct mechanism for handler systems to install closures that take `&mut State` and have them run on the same thread. The full-Bevy migration replaces this with `bevy_tasks` async event streams and direct resource mutation — see refactor point #5.

- **Why one less `Mutex` layer than Phase 6a?** Phase 6a wrapped the receiver in `Mutex<Receiver<T>>` for the resource. Phase 6b shares the existing `Arc<Mutex<Receiver<T>>>` directly with the resource (no second `Mutex`). The `poll_channels_system` locks the same `Mutex` as the legacy `update_*` methods — first caller wins. The cost is the same (one `lock()` per `try_recv`); the benefit is no double-wrapper.

- **Why the `drain` pattern in `waveform_handler_system`?** `WaveformResultEvent` carries a `WaveformLoadMessage` which is not `Clone`. `MessageReader<T>::read` returns an iterator over `&T`; we can't move the message out without taking `&mut World` and using `Messages<T>::drain`. The other 4 handler systems use the standard `MessageReader<T>` pattern (their payloads are `Clone`).

- **Race semantics**: the legacy `update_*` methods and the new handler systems race for messages on the same shared receiver. The first caller to `try_recv` wins. In practice:
    - `State::update` (the legacy frame loop) runs the `update_*` methods. These call `tick_state` does NOT run before `update`. Wait, let me re-check. The flow is:
        1. `State::update()` calls the legacy `update_*` methods directly (Phase 6a behavior — unchanged).
        2. `State::tick_state()` runs the schedule (which runs the poll + handler systems) and drains the thread-local slot.
        3. Both paths can run in the same frame (e.g., `State::update()` followed by `State::tick_state()`). Whoever locks the receiver first gets the message.
        4. In tests that exercise the schedule, only `tick_state` runs (no `update`). The poll system + handler systems drain everything.

    This is the "first caller wins" behavior. The new systems are the intended path; the legacy methods exist for compatibility with tests that drive the State directly without going through the schedule. Both paths are idempotent in the sense that running them on the same frame doesn't double-process messages.

### Schedule graph (post Phase 6b)

`Per-frame schedule order (StateSchedule):
  transitions::phase_transition_system        // AppPhase change detection
  mirror::install_mirror                      // tick_state installs the closure
  events::handle_* (per-message)              // MessageReader<T> -> PendingCommands
  async_bridge::poll_channels_system          // drain 5 shared receivers -> 5 result events
  async_bridge::add_apply_async_results_systems // consume events -> install closures
  render::mirror_pod_state_via_bridge         // drain closure -> RenderUniformsRes / RenderFrameInputRes
  // tick_state then drains pending async result closures -> mutates State
  dispatch_pending_commands                   // PendingCommands -> State::dispatch[_editor]`

### Bevy 0.19 API notes (Phase 6b)

- `thread_local!` with `RefCell<Vec<Box<dyn FnOnce(&mut State)>>>` is the bridge slot. The closures don't need to be `Send` because the schedule + the drain both run on the main thread. The slot is process-global; tests that don't go through `tick_state` should not leave stale closures (the slot is drained on every `tick_state` call).
- `pub(crate)` visibility is used for `complete_auth_*` methods and `persist_waveform_cache_entry` so the handler systems can call them from closures.
- `Arc<Mutex<Receiver<T>>>` is `Send + Sync` (the inner `Receiver<T>: Send`), so it satisfies Bevy's `Resource` bound. The resource holds a clone of the `Arc`; the legacy field holds its own clone; both consumers race for messages via the same `Mutex`.

### Verification

- `cargo test -p eg-state --lib` is green: **434 tests pass** (was 424 before Phase 6b, +10 new unit tests).
- `cargo build -p eg-state` is clean.
- `cargo clippy -p eg-state --all-targets` shows no **new** warnings introduced by Phase 6b (pre-existing baseline warnings in `transitions.rs` doc lists, `events/mod.rs` let-bindings, `command_dispatch.rs` casts, and `gizmo.rs` field-assignments are not Phase 6b regressions — they predate this phase).

## Architectural shape right now

- The migration so far is a **parallel mirror** strategy: the legacy `State` god-object + its subsystems (`RenderSubsystem`, `GameplaySubsystem`, `EditorSubsystem`, `AudioSubsystem`, `AuthSubsystem`, `SessionSubsystem`, `MenuSubsystem`) are **all still authoritative**. The Bevy `World` is a read-only / drain-target sidecar.
- The `AppCommand` (15 variants) + `EditorCommand` (~87 variants) enums are **still defined and dispatched** through command_dispatch.rs + editor_dispatch.rs. Phase 3 added a `Message`-to-`LegacyCommand` drain path, but the final swap (delete the enums + dispatch files) is gated on Phase 5/7.
- `Runtime` in `eg-app` does not yet own a `bevy_app::App`; the world's schedules are run by `State::tick_state` → `self.world.try_run_schedule(StateSchedule)`. The winit loop never calls `app.update()` directly.

## Likely next steps

The natural order (matches the plan's dependencies) is:

1. **Phase 4c-4e** — finish the render extraction. This is the biggest remaining gap and the only Phase 4 sub-phase that touches the GPU path.
2. **Phase 5** — rewrite `eg-ui` as Bevy systems (depends on Phase 3 messages being writable from `MessageWriter`, not just from `State`).
3. **Phase 7** — flip `Runtime` to own `bevy_app::App`, let winit call `app.update()`. This is the "big bang" cut-over; everything that was a parallel mirror becomes authoritative.
4. **Phase 8** — `TestApp` harness (parallelizable; can be done alongside 4/5/7).
5. **Phase 9** — final docs pass.
