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
- [ ] **Phase 4b–4e** — move `GpuContext` / `SceneMeshes` / `SurfaceHost` into resources; replace `rebuild_*_mesh` with systems; add `present_frame_system`.
- [ ] **Phase 5** — UI as Bevy systems.
- [ ] **Phase 6** — platform/audio/auth async bridging.
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

- 15 event submodules under vents/ (
  avigation, uth,
  ditor_mode, snap, lock, selection, imeline, iming,
  pm, spawn, history, camera, misc, session,
  input_state, pointer).
- One Message struct per semantic command (~85 distinct message
  types).
- A ToLegacy trait on each message type that converts the message to
  a LegacyCommand (an AppCommand or EditorCommand enum variant).
- A per-message drain system (one n handle\_\*(MessageReader<T>,
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
per-message drain systems in [vents::CommandSet], then a single
drain into [vents::PendingCommands] at the end of the frame. The
full Bevy 0.19 pattern is:

- #[derive(Message)] for the message type.
- Messages<T> for the resource (added by pp.add_message::<T>()).
- MessageReader<T> / MessageWriter<T> for system params.
- world.write_message(event) to send.
- world.resource_mut::<Messages<T>>() to inspect the buffer.

### Event taxonomy table

| Module | Count | Examples |
| ------ | ----- | -------- |

|
avigation | 10 | TurnRight, ToggleEditor, GameSetPracticeMode |
| uth | 5 | AuthSubmitSignIn, AuthSignOut, ResizeSurface |
| ditor_mode | 5 | SetEditorMode, SetBlockId, PickBlockAt |
| snap | 4 | SetSnapToGrid, SetSnapStep |
| lock | 5 | RemoveBlock, CopyBlock, UpdateSelectedBlock |
| selection | 3 | SnapSelectionToGrid, AddTransformTrigger |
| imeline | 14 | ToggleTimelinePlayback, SetTimelineTime, Playtest|
| iming | 6 | AddTimingPoint, SetTimingPointTime |
| pm | 2 | BpmTap, BpmTapReset |
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
disappear in the full-Bevy migration; the corresponding evy_input
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
14 new unit tests in vents::tests cover ToLegacy conversion +
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

### What Phase 4b / 4c / 4d / 4e do (next)

- **Phase 4b** moves `State::render.gpu` ownership into `RenderGpuRes` and replaces `self.render.gpu.X` reads with `world.resource::<RenderGpuRes>().0.X` (440 call sites). Adds `camera_uniform_scratch` / `line_uniform_scratch` / `color_space_scratch` fields on `GpuContext` so the per-frame uniform values can be read back from the GPU-side state.
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
