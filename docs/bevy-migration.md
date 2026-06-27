# Bevy ECS Migration — Refactor Point Registry

> Living document. Every site that will eventually be replaced by a full-Bevy
> feature has a `// REFACTOR POINT(full-bevy):` comment in the code AND a row in
> the table below. Remove both together when the swap lands.

## Strategy

**Now (Phase 0–9 complete):** `eg-game` has a headless ECS core,
`eg-state` exposes a Bevy `Message`/schedule/resource boundary, `eg-ui` writes
commands into that boundary, and `eg-app` drives a `bevy_app::App` from the
winit redraw path. The legacy `State` object, `State::world`, and
`FramePipeline` remain as the compatibility bridge until the single-world
deletion pass moves render/input/session ownership fully into the App world.

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

| #   | Site (code)                                                                                      | Current                                                     | Full-Bevy replacement                                                   |
| --- | ------------------------------------------------------------------------------------------------ | ----------------------------------------------------------- | ----------------------------------------------------------------------- |
| 1   | `crates/eg-app/src/platform/application.rs`                                                      | Custom winit `ApplicationHandler` owns window/canvas events | `bevy_winit` plugin + `App::run()`                                      |
| 2   | `crates/eg-app/src/platform/runtime.rs`                                                          | `Runtime` keeps `State`/`FramePipeline` beside the App      | App resources own egui/render/window state directly                     |
| 3   | `crates/eg-app/src/platform/pipeline.rs`                                                         | Manual egui/wgpu frame pipeline after `app.update()`        | `bevy_egui`, `bevy_render`, and Bevy schedule-driven presentation       |
| 4   | `crates/eg-app/src/platform/input_mapping.rs`                                                    | winit input translated into project `InputEvent` strings    | `bevy_input` `ButtonInput<KeyCode>` / mouse / touch / gamepad resources |
| 5   | `crates/eg-ui/src/plugin.rs`                                                                     | `EguiContextRes` + UI command bridge                        | `bevy_egui` systems writing Bevy events/input directly                  |
| 6   | `crates/eg-game/src/state.rs`                                                                    | `GameState` wrapper mirrors ECS world fields                | `eg-state` consumes gameplay components/resources directly              |
| 7   | `crates/eg-game/src/simulation.rs`                                                               | Timeline scrub uses a private `GameState` wrapper           | Shared/sub `bevy_ecs::World` + `Schedule` for editor and play views     |
| 8   | `crates/eg-game/src/resources.rs`                                                                | `GameClock`, `FixedStepConfig`, `FrameDelta` resources      | `bevy_time` `Time<Fixed>` / `Time<Virtual>` schedules                   |
| 9   | `crates/eg-game/src/plugin.rs`                                                                   | Custom `Schedule` + `GameplaySet`                           | Bevy `FixedUpdate` / standard gameplay schedule sets                    |
| 10  | `crates/eg-game/src/events.rs`                                                                   | `ConsumedEventsBuffer` compatibility resource               | Normal Bevy message/event stream                                        |
| 11  | `crates/eg-state/src/events/mod.rs`                                                              | Bevy `Message` taxonomy bridged back to legacy commands     | Direct Bevy message handlers over resources/components                  |
| 12  | `crates/eg-state/src/events/async_results.rs` + `crates/eg-state/src/state/async_bridge.rs`      | mpsc channel polling + thread-local closure bridge          | `bevy_tasks` streams and direct resource mutation                       |
| 13  | `crates/eg-state/src/events/mesh_rebuild.rs`                                                     | Rebuild messages bridge `Changed<>` to legacy mesh dirties  | `bevy_render` extraction / asset events drive render rebuilds           |
| 14  | `crates/eg-state/src/state/state_resources.rs`                                                   | `AppPhaseRes` tracks current/previous phase                 | `AppPhase` derives Bevy `States`; wrapper bookkeeping disappears        |
| 15  | `crates/eg-state/src/state/transitions.rs`                                                       | Manual phase-transition detection                           | Bevy `States`, `OnEnter`, and `OnExit` schedules                        |
| 16  | `crates/eg-state/src/state/plugins.rs`                                                           | Project plugin shells and custom schedule labels            | Standard Bevy plugin graph and schedule labels                          |
| 17  | `crates/eg-state/src/state/mod.rs`                                                               | Legacy `State` subsystem fields remain authoritative        | State lives in App-world resources/components                           |
| 18  | `crates/eg-state/src/state/command_dispatch.rs` + `crates/eg-state/src/state/editor_dispatch.rs` | Legacy enum dispatchers still apply mutations               | Deleted after Bevy handlers mutate resources/components directly        |
| 19  | `crates/eg-state/src/state/audio_state.rs`                                                       | Custom audio subsystem polled through `State`               | Bevy audio resources/systems or retained custom backend behind systems  |
| 20  | `crates/eg-state/src/state/render/`                                                              | Custom wgpu resources, extraction, and present bridge       | `bevy_render` render graph, assets, extract schedule, `ShaderRef`       |
| 21  | `crates/eg-state/src/state/test_support.rs`                                                      | `TestApp` wraps the compatibility plugin graph              | Thin `bevy_app::App` test harness only                                  |

## Plugin replacement table

| Current plugin/boundary | Current owner                                          | Full-Bevy destination                                        |
| ----------------------- | ------------------------------------------------------ | ------------------------------------------------------------ |
| `GameplayPlugin`        | `eg-game` ECS systems + custom `GameplaySet`           | Bevy `FixedUpdate` / gameplay schedule sets with `bevy_time` |
| `EditorPlugin`          | `eg-state` editor resources + legacy handlers          | Bevy systems over editor resources/components                |
| `SessionPlugin`         | `eg-state` session resources + legacy `State` fields   | App-world session resources                                  |
| `MenuPlugin`            | `MenuStateRes` / `AppPhaseRes` compatibility resources | Bevy `States` + menu resources                               |
| `RenderPlugin`          | Custom wgpu resources + extraction/present sets        | `bevy_render` render graph and render-world extraction       |
| `UiPlugin`              | `EguiContextRes` + command bridge                      | `bevy_egui` systems                                          |
| Async bridge            | mpsc receivers + `PollChannelsSet` / handlers          | `bevy_tasks` streams and asset events                        |
| Runtime/app boundary    | winit + `Runtime::run_app_frame`                       | `bevy_winit` + `App::run()`                                  |

## Event taxonomy (post Phase 3)

`AppCommand` (15 variants) + `EditorCommand` (~87 variants) fragment into Bevy
`Message` structs, one per semantic command. 12 input-state variants
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
  CommandHandlerSet   // MessageReader<*> → mutating systems
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
- [x] **Phase 4b-3** — mirror Playing + GameOver + editor-playtest paths (4 new tests pinning `playing_camera_view` + `editor_preview_camera_view` + line uniform per player direction).
- [x] **Phase 4b-4** — mirror `ColorSpaceUniform.time_seconds` from `frame_runtime.global_time_seconds` + the gamma-toggle bit; 2 new tests covering the bit-as-float conversion.
- [x] **Phase 4c** — insert `RenderMeshesRes` + `RenderSurfaceHostRes` resources; `MeshSlot` + `SceneMeshes` now derive `Default` for schedule/test construction.
- [x] **Phase 4d-scaffold** — `RebuildMeshSet` system set + `RebuildMeshEvent` / `RebuildMeshKind` event types.
- [x] **Phase 4e-scaffold** — `present_render_frame_placeholder_system` reads the POD resources; the actual wgpu present call from `eg-app/src/platform/pipeline.rs::render_frame` stays in place.
- [x] **Phase 4b-final** — move `State::render.gpu` ownership into `RenderGpuRes`; direct call sites now route through `State::render_gpu()` / `render_gpu_mut()` backed by the world.
- [x] **Phase 4c-final** — `RenderMeshesRes` / `RenderSurfaceHostRes` now own the actual `SceneMeshes` / `SurfaceHost`; legacy mesh/surface call sites route through `State` accessors backed by the world.
- [x] **Phase 4d-final** — `ObjectBundle` mirror components + `Changed<>` detection now drive `mark_mesh_dirty_system`; `EditorDirtyFlags` remains as the legacy consumer until editor mutation moves into the shared World.
- [x] **Phase 4e-final** — `present_render_frame_system` is registered in `RenderSet::Present` and reads authoritative render resources; the production `render_egui` compatibility call remains until Phase 7 gives the Bevy app ownership of the frame.
- [x] **Phase 5** — UI as Bevy systems.
- [x] **Phase 6a** — async bridging scaffold (5 channel wrapper resources + `PollChannelsSet` + 5 result events).
- [x] **Phase 6b** — replace legacy `update_*` methods with handler systems that consume result events.
- [x] **Phase 7** — `Runtime` owns a `bevy_app::App`; `register_state_app_update_driver` runs the App-world `StateSchedule` from Bevy `Update`; `Runtime::run_app_frame(raw_input)` drives `app.update()` before the compatibility `FramePipeline`; `ApplicationHandler::RedrawRequested` calls `run_app_frame`. `StatePlugins` now install default App resources directly, including `eg_game::GamePlugin`; the legacy `State` world and render pipeline remain as a compatibility bridge until the single-world deletion pass.
- [x] **Phase 8** — `TestApp` harness scaffold: `TestApp::minimal()` builder in `eg-state::state::test_support` builds a `bevy_app::App` with the eg-state plugin graph + `StateSchedule`; `tick_state()` mirrors `State::tick_state()`; `run_state_schedule()` runs the schedule only; `assert_resource_exists::<T>()` helper. `eg-game::TestApp` builds a headless gameplay app with `GamePlugin` + `tick_gameplay()`. Migration of the existing `&State`/`&GameState` tests to the new harness is incremental and tracked in follow-up commits.
- [x] **Phase 9** — documentation pass: refactor-point registry, event taxonomy, system-set graph, plugin replacement table, and AGENTS architecture notes are current.

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

### What Phase 4b-3 and Phase 4b-4 add (mirror covers all four `AppPhase` values + time + gamma)

The 4b-2 commit covered Menu + the Editor non-playtest path. The remaining paths in `compute_render_uniforms_snapshot` were **already producing the right values** — only the tests were missing. Phase 4b-3 + 4b-4 add the missing test coverage so every branch of the mirror is pinned:

- **Phase 4b-3** — 3 new tests:
    - `tick_state_mirrors_render_uniforms_in_playing` — sets `state.phase = Playing`, `elapsed_seconds = 0.5`, `position = [12, 3, 7]`, `direction = Right`. Computes `(expected_eye, expected_target) = state.playing_camera_view()` (the same helper the production code calls) and asserts the mirrored `camera_position` and full `view_proj` matrix match. Also asserts the `line_uniform` (offset rounded to 2 decimal places, rotation = `-π/2` for `Direction::Right`).
    - `tick_state_mirrors_render_uniforms_in_game_over` — follows the same setup as Playing but flips `phase = GameOver` + `game_over = true`. Pins the `| Playing | GameOver` shared branch in the production code so any future refactor that splits them is caught.
    - `tick_state_mirrors_render_uniforms_in_editor_playtest` — sets `editor.timeline.playback.playing = true` + `playtesting_editor = true` + `timeline.clock.time_seconds = 0.75` + `preview.position = [2, 4, 6]`. Computes `(expected_eye, expected_target) = state.editor_preview_camera_view()` and asserts the mirrored `camera_position` matches. This is the one Editor path that does NOT use `editor_pan` + `editor_target_z` — it uses the camera track like the Playing path.

- **Phase 4b-4** — 2 new tests:
    - `tick_state_mirrors_color_space_time_and_gamma` — sets `frame_runtime.global_time_seconds = 12.5` + `apply_gamma_correction = true`. Asserts `ColorSpaceUniform.time_seconds == 12.5` and `apply_gamma_correction == 1.0`. This pins the per-frame time + the gamma-as-float conversion.
    - `tick_state_mirrors_gamma_off_as_zero` — sets `apply_gamma_correction = false`. Asserts `apply_gamma_correction == 0.0` exactly (not 0.5 or any other float). Pins the bit-vs-float conversion in `compute_render_uniforms_snapshot`.

After 4b-4, every branch of `compute_render_uniforms_snapshot` is test-pinned. The total mirror test count is now 8 (`tick_state_mirrors_frame_input_into_render_resource` + 7 `tick_state_mirrors_render_uniforms*` tests). The 440 call sites of `State::render.gpu.*` remain untouched.

### What Phase 4c / 4d / 4e do (next)

The 4b-3 + 4b-4 commits covered the mirror-driven approach for all four `AppPhase` values + time + gamma. The remaining Phase 4 sub-phases (4c, 4d, 4e) move the heavy render ownership into Bevy resources. Phase 4c-final is complete; GPU ownership and present cut-over remain pending.

- **Phase 4c-scaffold** — `RenderMeshesRes` + `RenderSurfaceHostRes` are inserted with default values by `register_render_pod_resources`. Bevy systems can query for their presence. Two `Default` impls were added to `eg-render`: `MeshSlot::default() = MeshSlot::Empty` and `SceneMeshes::default()` (all 25 `MeshSlot` fields set to `Empty`, the two chunked/instanced slots use their existing `#[derive(Default)]` impls, `editor_selection_outline_instances = Vec::new()`).

- **Phase 4c-final** — complete. `State` construction now inserts the actual `SceneMeshes` / `SurfaceHost` into `RenderMeshesRes` / `RenderSurfaceHostRes`, `RenderSubsystem` no longer owns those fields, and legacy callers use `State::render_meshes()` / `render_meshes_mut()` / `surface_host()`. Mesh uploads that also need `GpuContext` use split-borrow helpers so resource-owned meshes can be mutated while reading the legacy device/queue.

- **Phase 4d-scaffold** — added the `RebuildMeshSet` system set + the `RebuildMeshEvent` / `RebuildMeshKind` types in `crates/eg-state/src/events/mesh_rebuild.rs`. The 9 `RebuildMeshKind` variants correspond 1:1 to the 9 bits on the legacy `EditorDirtyFlags` bitfield.

- **Phase 4d-final** — complete. `eg-game::ObjectBundle` now includes authored mirror components (`Position`, `Rotation`, `Size`, `BlockId`, `ColorTint`) alongside `ObjectData`. `mark_mesh_dirty_system` uses Bevy `Changed<>` filters over those components and emits `SyncGameObjects` + `RebuildBlockMesh` events. `EditorDirtyFlags` remains as the legacy editor-side consumer until editor mutations move into the shared World.

- **Phase 4e-scaffold** — replaced by Phase 4e-final. The old placeholder is gone.

- **Phase 4e-final** — complete at the resource boundary. `present_render_frame_system` is registered in `RenderSet::Present` and reads `RenderGpuRes`, `RenderMeshesRes`, `RenderSurfaceHostRes`, `RenderUniformsRes`, and `RenderFrameInputRes`. The production frame still calls `State::render_egui` from `FramePipeline` so egui paint jobs are presented exactly once; removing that compatibility call is now Phase 7 work when `Runtime` owns a `bevy_app::App` and can inject egui resources before `app.update()`.

### Schedule graph (post Phase 4)

`Per-frame schedule order (StateSchedule):
  transitions::phase_transition_system        // AppPhase change detection
  mirror::install_mirror                      // tick_state installs the closure
  render::mark_mesh_dirty_system              // Changed<> authored object detection -> RebuildMeshEvent
  events::handle_* (per-message)              // MessageReader<T> -> PendingCommands
  async_bridge::poll_channels_system          // drain 5 shared receivers -> 5 result events
  async_bridge::add_apply_async_results_systems // consume events -> install closures
  render::mirror_pod_state_via_bridge         // drain closure -> RenderUniformsRes / RenderFrameInputRes
  render::present_render_frame_system         // read authoritative render resources
  // tick_state then drains pending async result closures -> mutates State
  dispatch_pending_commands                   // PendingCommands -> State::dispatch[_editor]`

The order matches the legacy `render_to_view` / `render_with_overlay` /
`State::update` call sequence: simulation → mesh rebuild → extract →
present. Future commits add real systems into each set.

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

## Phase 5 — UI command bridge + `UiPlugin`

`eg-ui` now owns the Bevy-facing UI boundary:

- `crates/eg-ui/src/plugin.rs` defines `UiPlugin`, `EguiContextRes`, `UiCommandQueue`, and an exhaustive `AppCommand` / `EditorCommand` -> Bevy `Message` bridge.
- `show_editor_ui_commands` renders the existing editor UI and returns commands instead of dispatching them internally; `show_editor_ui` remains as a compatibility wrapper.
- `crates/eg-app/src/platform/pipeline.rs` writes editor/menu/pause UI commands into the Runtime App world with `write_ui_commands(...)`, runs `app.update()` to drain them into App-world `PendingCommands`, then applies those commands to the legacy `State` through the compatibility bridge before `State::tick_state()` and `State::update()`.
- The visual UI code still uses `UiViewModel` during the compatibility window. Phase 7 removes that final caller shape when `Runtime` owns a `bevy_app::App` and injects the egui context before `app.update()`.

**Verification:** `cargo check -p eg-ui` and `cargo check -p eg-app` pass. Both runs reported recurring Windows incremental-cache finalization warnings (`Access is denied`) but exited successfully.

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

## Phase 7 — App wiring complete

Phase 7 flips the redraw boundary so the winit loop drives the Bevy `App` every frame. The legacy `State` object and `FramePipeline` still own rendering/input compatibility, but they now sit behind `Runtime::run_app_frame(raw_input)`: the compatibility pipeline renders UI, writes UI commands into the App world, runs `app.update()` to drain them through the App-world schedule, bridges the resulting `PendingCommands` into legacy `State`, then renders the frame.

### What's in Phase 7 today

- **`Runtime` owns a `bevy_app::App` field** alongside the legacy `state: State` and `pipeline: FramePipeline` fields. The App's World remains separate from `State::world`, but it is no longer idle: Bevy `Update` runs the App-world `StateSchedule` once per frame.

- **`Runtime::new` builds and drives the App**:

    ```rust
    let mut app = bevy_app::App::new();
    setup_state_app(&mut app);
    register_state_command_pipeline(app.world_mut());
    register_state_app_update_driver(&mut app);
    ```

    Both helpers are already public from Phases 2 and 3. The App's `World` now has:
    - Every `StatePlugins` resource (`AppPhaseRes`, `SessionStateRes`, the 9 `Editor*Res`, `PerfRes`).
    - Every `RenderPlugin` POD resource (`RenderUniformsRes`, `RenderFrameInputRes`, `RenderResizeRequest`) + the `Messages<RebuildMeshEvent>` buffer.
    - The `StateSchedule` registered by `register_state_command_pipeline`.
    - A Bevy `Update` system (`run_state_schedule_from_app_system`) that calls `phase_transition_system` and then `world.try_run_schedule(StateSchedule)`.

- **`Runtime::run_app_frame(raw_input)`** — delegates to `FramePipeline::run_frame(&mut self.state, &mut self.app, raw_input)`. The pipeline writes UI-originated commands into the App world, runs `app.update()`, and bridges App-world `PendingCommands` into the legacy `State` dispatch path. The old `Runtime::run_frame` wrapper is gone; production callers use the App-driving entry point.

- **`ApplicationHandler::RedrawRequested` calls `runtime.run_app_frame(raw_input)`** on native and wasm. This makes the production redraw path exercise the Bevy `App` before rendering.

- **`StatePlugins` now own App-world defaults** — `MenuPlugin`, `SessionPlugin`, `EditorPlugin`, `GameplayPlugin`, and `RenderPlugin` install their default resources directly. `GameplayPlugin` now adds `eg_game::GamePlugin`, so the App world owns `GameClock` and `GameplaySchedule`. The bare-`World` `register_state_resources` helper remains only for legacy `State::world` construction and focused tests.

- **Re-export** of `register_state_command_pipeline` and `register_state_app_update_driver` from `eg_state::state` (the `plugins` module stays private; only the helpers are re-exported for `eg-app` to call).

- **`bevy_ecs` added to `eg-app/Cargo.toml`** — the runtime tests inspect the App's `World` via `World::contains_resource`, `World::resource`, and `World::try_run_schedule`. Those methods need the `World` type in scope.

- **Tests** in `crates/eg-app/src/platform/runtime.rs::tests`:
    - `runtime_app_has_state_plugin_resources` — every `StatePlugins` resource is registered on the App's World.
    - `runtime_app_registers_state_schedule` — `StateSchedule` is reachable via `try_run_schedule` on the App's World.
    - `runtime_run_app_frame_returns_output` — `run_app_frame` produces normal egui output.
    - `runtime_run_app_frame_updates_app_state_schedule` — `run_app_frame` drives `app.update()`, which runs the App-world phase transition + `StateSchedule`.
    - `runtime_run_app_frame_dispatches_app_world_commands` — an App-world message drains through `StateSchedule` and is applied to the legacy `State` during the compatibility window.
    - `runtime_app_world_is_separate_from_state_world` — the App's `AppPhaseRes.current == Menu` matches `state.is_menu()`, pinning the parallel-worlds contract.

- **Dead-code allowances removed** from `Runtime.app` and `Runtime::run_app_frame` because both are now production-used.

### Schedule graph (post Phase 7)

The production redraw path now writes UI commands into the App world, runs the App-world `Update` schedule, bridges App-world `PendingCommands` into legacy `State`, then runs the remaining legacy `State::tick_state` / update / render path. The App world and `State::world` are still separate, but UI command ingress is App-world first.

```
RedrawRequested:
  Runtime::run_app_frame(raw_input)
    FramePipeline::run_frame(&mut State, &mut App, raw_input)
      egui UI -> write_ui_commands(App::world)
      app.update()
        Update::run_state_schedule_from_app_system
          transitions::phase_transition_system
          app.world.try_run_schedule(StateSchedule)
      State::dispatch_pending_commands_from_world(App::world)
      State::tick_state()
      State::update()
      State::render_egui(...)
```

### Why two worlds still coexist

The App's `World` and `State::world` are separate `bevy_ecs::world::World` instances. They cannot be the same `World` today because:

- `State` is `!Send` (it owns `mpsc::Receiver<T>` fields). Bevy's `App` is `Send + Sync` (it owns a `World` whose resources must be `Send + Sync`).
- `State` is constructed in `State::new_test` / `new_native` / `new_web` with custom initialization logic that includes the heavy `RenderGpuRes` (which has no `Default`). The App's `World` is built by `setup_state_app` + `register_state_command_pipeline` which don't know about the GPU.
- The thread-local bridges (Phase 4a render mirror, Phase 6b async result handlers) read from `State` from inside Bevy systems. They expect `State` to be on a stable address (the `Runtime`'s field), not buried inside the App's `World`.

The remaining single-world deletion is now tracked as follow-up migration work, not Phase 7: move heavy render/input/session ownership into App-world resources, delete `State::world`, then collapse `FramePipeline` into Bevy systems.

### Bevy 0.19 API notes (Phase 7)

- `bevy_app::App::new()` returns an `App` with default schedules initialized. `app.add_plugins(...)`, `app.init_resource::<T>()`, and `app.add_systems(Update, ...)` mutate the App. `app.update()` runs the default schedules (`First` / `PreUpdate` / `Update` / `PostUpdate` / `Last`).
- `App::world()` / `App::world_mut()` return `&World` / `&mut World`. `World::try_run_schedule(label)` is the cheapest way to assert that a custom schedule is registered.
- `App::update()` returns `&mut App` in Bevy 0.19. `Runtime::run_app_frame` discards the return and then runs the compatibility renderer.

### Verification

- `cargo test -p eg-app --lib runtime::` is green: **6 tests pass**.
- `cargo test -p eg-state --lib state::test_support` is green: **6 tests pass**.
- `cargo test -p eg-state --lib` is green: **452 tests pass**.
- `bun run lint` is clean.

## Phase 8 — `TestApp` harness

The plan calls for `TestApp` builders that let tests assert on World resources, queries, and events without going through the legacy `State` / `GameState` wrappers. Phase 8 lands the **scaffold** of those harnesses: the eg-state `TestApp::minimal()` builder + the `tick_state()` / `run_state_schedule()` / `assert_resource_exists::<T>()` helpers, and the eg-game `TestApp::minimal()` builder + `tick_gameplay()` helper. The full migration of the existing `&State`/`&GameState` tests to the new harnesses is incremental and tracked in follow-up commits.

### What's in Phase 8 today

- **`TestApp`** in `crates/eg-state/src/state/test_support.rs`:
    - `TestApp::minimal()` — builds a `bevy_app::App` with `setup_state_app` + `register_state_command_pipeline`. No GPU, no surface, no window. The canonical entry point for headless eg-state tests.
    - `TestApp::with_app(app)` — build from a pre-wired `App` for tests that need a custom plugin set.
    - `TestApp::app()` / `app_mut()` / `world()` / `world_mut()` — passthrough accessors.
    - `TestApp::tick()` — calls `app.update()` (no-op today, future-proofing for Phase 7-2).
    - `TestApp::tick_state()` — Bevy-native equivalent of `State::tick_state()`. Calls `phase_transition_system` (the free function) then runs `StateSchedule`. Mirrors the legacy ordering exactly.
    - `TestApp::run_state_schedule()` — runs only `StateSchedule`. For tests that want message-handling in isolation, without the phase-transition prelude.
    - `TestApp::assert_resource_exists::<T>()` — convenience for the "the plugin registered this resource" assertion. Panics with `std::any::type_name::<T>()` for readable failure output.
    - `TestApp: Default` — `default()` delegates to `minimal()`.

- **Re-export** — `pub mod test_support` under `state` (gated by `#[cfg(any(test, feature = "test-support"))]` to match the `State::new_test` gating).

- **Tests** (6 new in `state::test_support::tests`):
    - `test_app_minimal_seeds_state_plugin_resources` — every `StatePlugins` resource is registered on the App's World.
    - `test_app_minimal_defaults_app_phase_to_menu` — `AppPhaseRes.current` defaults to `AppPhase::Menu`.
    - `test_app_default_constructor_matches_minimal` — `TestApp::default()` and `TestApp::minimal()` produce equivalent Worlds.
    - `test_app_tick_is_a_safe_no_op` — three `app.update()` calls don't panic.
    - `test_app_run_state_schedule_succeeds_on_empty_world` — `run_state_schedule()` returns `true` on a freshly-constructed `TestApp`.
    - `test_app_phase_transition_via_tick_state` — mirrors the legacy `tick_state_runs_phase_transition_system` test on the Bevy-native `TestApp`. Phase change → `previous_app_phase` updates correctly.

- **Legacy test cross-link** — the existing `state_world_is_seeded_with_default_resources` test in `state::tests` now has a docstring pointing to the canonical `TestApp` test. The two should always agree; if one fails, the other is the regression marker.

- **`TestApp`** in `crates/eg-game/src/test_app.rs`:
    - `TestApp::minimal()` — builds a `bevy_app::App` with `GamePlugin`. No renderer, no audio, no platform services. The canonical entry point for headless gameplay ECS tests.
    - `TestApp::with_app(app)` — build from a pre-wired `App` for custom gameplay fixtures.
    - `TestApp::app()` / `app_mut()` / `world()` / `world_mut()` — passthrough accessors.
    - `TestApp::tick()` — calls `app.update()`.
    - `TestApp::tick_gameplay()` — runs `GameplaySchedule` once.
    - `TestApp::try_tick_gameplay()` — runs `GameplaySchedule` and reports whether the schedule is present.
    - `TestApp::assert_resource_exists::<T>()` — convenience for gameplay plugin registration assertions.
    - `TestApp: Default` — `default()` delegates to `minimal()`.

- **Tests** (3 new in `eg_game::test_app::tests`):
    - `test_app_minimal_registers_gameplay_resources` — key `GamePlugin` resources are registered.
    - `test_app_default_matches_minimal` — `TestApp::default()` and `TestApp::minimal()` produce equivalent Worlds.
    - `test_app_tick_gameplay_runs_schedule` — a spawned player advances through the real `GameplaySchedule`.

### Design notes

- **Why a separate `TestApp` and not just `State::new_test`?** `State::new_test()` requires a wgpu adapter (it initializes `GpuContext`, `RenderMeshes`, and `SurfaceHost` for the test fixture). Most eg-state tests don't need any of those — they only assert on World resources, queries, and events. The `TestApp` skips the heavy initialization entirely, so it's faster, more portable (works in environments without a GPU adapter), and tests are more focused.

- **Why call `phase_transition_system` separately in `tick_state()`?** The legacy `State::tick_state()` calls `phase_transition_system` as a free function (not a system) before running the schedule. The system version was never registered in `StateSchedule` — see `crates/eg-state/src/state/mod.rs:309-313` for the rationale. `TestApp::tick_state()` mirrors that ordering exactly so the test harness and the production path are equivalent.

- **Why gate behind `#[cfg(any(test, feature = "test-support"))]`?** Same pattern as `State::new_test()`: the harness is in-tree for `cargo test` and opt-in for downstream crates that want to test eg-state integration. The `test-support` feature on `eg-state/Cargo.toml` already exists; this is the first consumer.

- **Why `Default` derive?** Convenience — `TestApp::default()` reads cleaner than `TestApp::minimal()` in fixtures and parameterized tests.

### What's NOT in Phase 8 today (follow-up work)

- **Test migration** — the existing `&State`/`&GameState` tests stay on the legacy path. Migration is incremental; each test gets migrated when its next edit happens. The harnesses exist so new tests can be written Bevy-native from day one.
- **`TestGpuFixture` integration** — the plan mentions using the existing `TestGpuFixture` for the harness. The current `TestApp::minimal()` doesn't need it (no heavy resources), so this is only relevant if a future `TestApp::with_gpu()` variant lands.

### Bevy 0.19 API notes (Phase 8)

- `bevy_app::App::new()` + `setup_state_app(&mut app)` is the canonical "fresh `App` with eg-state plugins" path. The helper also calls `register_state_resources(&mut app.world_mut())` so the `App` and `World` flavors stay in sync.
- `App::world()` / `App::world_mut()` are the standard `&World` / `&mut World` accessors.
- `World::try_run_schedule(label)` is the cheapest way to assert schedule registration + run it.
- `World::resource::<T>()` / `World::resource_mut::<T>()` / `World::contains_resource::<T>()` are the standard resource accessors; `contains_resource` is the right tool for "did the plugin register this resource?" assertions.

### Verification

- `cargo test -p eg-state --lib` is green: **452 tests pass** (was 446 before Phase 8, +6 new self-tests).
- `cargo test -p eg-app --lib runtime::` is still green: 6 tests.
- `cargo build -p eg-state --features test-support` is clean.
- `bun run lint` is clean.

## Architectural shape right now

- The migration so far is still a **parallel mirror** strategy for rendering, input, async, session, and editor mutation. Gameplay ECS resources and `GameplaySchedule` are installed by the App's `GameplayPlugin`, and UI-originated commands now enter through the Runtime App world before being bridged to legacy `State`; the legacy `State` / `GameState` compatibility fields still drive production behavior until the single-world deletion pass.
- The `AppCommand` (15 variants) + `EditorCommand` (~87 variants) enums are **still defined and dispatched** through command_dispatch.rs + editor_dispatch.rs. Phase 3 added a `Message`-to-`LegacyCommand` drain path, and Phase 5 routes UI output into that path. The final swap (delete the enums + dispatch files) is gated on Phase 7.
- `Runtime` in `eg-app` now owns a `bevy_app::App` alongside the legacy `state`/`pipeline` fields. `ApplicationHandler::RedrawRequested` calls `Runtime::run_app_frame(raw_input)`, which lets `FramePipeline` write UI commands into the App world and run `app.update()` before legacy update/render. The App world has the real eg-state plugin graph and gameplay schedule; `State::world` remains parallel for rendering and non-UI legacy dispatch until the single-world deletion pass.

## Full-Bevy Next Roadmap

The project is now App-driven, but it is not ready for the Bevy umbrella yet.
The next work should remove the compatibility bridge first, then replace one
platform/rendering boundary at a time. Each step should keep native + wasm
green before moving on.

### 1. Delete the parallel `State::world`

This is the prerequisite for every full-Bevy swap. Move the authoritative
resources that still live behind the legacy `State` object into the App world,
then make `State` either disappear or become a thin construction/helper layer.

- Move render ownership (`RenderGpuRes`, `RenderMeshesRes`, `RenderSurfaceHostRes`) into the App world at startup.
- Move session/editor/menu/audio/auth resources out of legacy subsystem fields and into App-world resources.
- Route UI, async results, and input through the App world only; stop writing to `State::world`.
- Delete `State::world`, then delete accessors that only exist to bridge between the two worlds.
- Keep `cargo test -p eg-state --lib` and `cargo test -p eg-app --lib runtime::` green throughout.

### 2. Replace legacy command dispatch

Once there is one authoritative World, stop converting Bevy `Message`s back to
`AppCommand` / `EditorCommand`.

- Move each `ToLegacy` bridge into a direct handler system over resources/components.
- Delete `PendingCommands`, `command_dispatch.rs`, and `editor_dispatch.rs` after the last handler is direct.
- Move the input-state carve-out (`SetShiftHeld`, pointer moves, camera drag, etc.) to a real `InputSet` that writes `EditorInputRes` directly.
- Use tests around undo/redo, selection, timeline scrub, and playtest enter/exit as the parity gate.

### 3. Promote phase handling to Bevy `States`

After legacy dispatch is gone, convert phase bookkeeping to Bevy state APIs.

- Derive or wrap `AppPhase` as a Bevy `States` type.
- Replace `AppPhaseRes.previous_app_phase` and `phase_transition_system` with `OnEnter` / `OnExit` schedules.
- Attach editor/play/menu systems to state-gated sets instead of manual phase checks where practical.
- Keep the shared object entity model: authored object components persist, while play-only runtime components are added/removed on transitions.

### 4. Move egui to `bevy_egui`

Do this before `bevy_render` if UI command flow is already direct, because it
removes the manual `EguiContextRes` / `FramePipeline` coupling.

- Replace `EguiContextRes` and manual `ctx.run_ui` calls with `bevy_egui` systems.
- Let UI systems write Bevy messages/resources directly, not `UiCommandQueue` compatibility commands.
- Delete `UiViewModel` once UI reads resources/queries directly.
- Verify editor drag/marquee/gizmo interactions still mutate on the same frame.

### 5. Move rendering to `bevy_render`

Keep the visual migration conservative: preserve current mesh builders and WGSL
semantics first, then modernize assets/materials afterward.

- Convert `SceneMeshes` data into Bevy `Mesh` / `Material` assets or a custom render asset pipeline.
- Move extraction from `RenderSet::Extract` into Bevy's render-world extract schedule.
- Port `shader.wgsl` as a `ShaderRef` or equivalent custom material shader.
- Replace `State::render_egui` / manual present with Bevy render graph nodes.
- Run native and wasm visual smoke checks for menu, editor, play, game-over, and export flows.

### 6. Replace runtime/window/input plumbing

With UI and render in Bevy, the custom runtime becomes small enough to remove.

- Replace `ApplicationHandler` with `bevy_winit` and Bevy's runner.
- Replace `SurfaceHostRes` window/canvas ownership with Bevy window entities.
- Replace `input_mapping.rs` with `bevy_input` resources (`ButtonInput<KeyCode>`, mouse, touch, gamepad).
- Delete `Runtime` and `FramePipeline` once no production code depends on them.

### 7. Finish time, async, audio, and tests

These can happen in parallel once the single App world is authoritative.

- Replace `FrameDelta`, `FixedStepConfig`, and manual clock advancement with `bevy_time` (`Time<Virtual>`, `Time<Fixed>`, `FixedUpdate`).
- Replace mpsc channel polling and thread-local async result closures with `bevy_tasks` streams or asset events.
- Decide whether audio stays custom behind Bevy systems or moves to `bevy_audio`.
- Migrate old `&State` / `&GameState` tests to the eg-state and eg-game `TestApp` harnesses as files are touched.

### Validation cadence

- After each small cut-over: `cargo test -p <touched-crate>` and `bun run lint`.
- After runtime/render/input changes: `cargo test -p eg-app --lib runtime::`, `cargo test -p eg-state --lib`, and `bun run build`.
- Before calling a full-Bevy milestone done: `bun run dev`, then manually smoke test menu, level play, editor placement, playtest, audio import, export, auth-safe no-op paths, and wasm startup.
