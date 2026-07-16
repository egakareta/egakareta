# Bevy ECS Migration — Refactor Point Registry

> Living document. Every site that will eventually be replaced by a full-Bevy
> feature has a `// REFACTOR POINT(full-bevy):` comment in the code AND a row in
> the table below. Remove both together when the swap lands.

## Strategy

**Now (Phase 0–9 in progress):** `eg-game` has a headless ECS core,
`eg-state` exposes a Bevy `Message`/schedule/resource boundary, `eg-ui` writes
commands into that boundary, and `eg-app` drives a `bevy_app::App` from the
winit redraw path. The duplicate `State::world` and `FramePipeline` wrapper have
been deleted; App owns the egui context, renderer, menu textures, block-icon
cache, GPU, scene meshes, surface host, and scheduled present path. The legacy `State` is no longer constructed by production startup and remains only as a
GPU-backed compatibility fixture while its remaining tests migrate to `TestApp` and direct
App-owned render fixtures.

**Later (full Bevy):** drop the custom wgpu / egui-wgpu / winit plumbing in
favor of the Bevy umbrella. The migration is designed to be mechanical.

## Compatibility Boundary Ledger

Only the following compatibility boundaries may remain or shrink during the
single-world deletion pass. New State-to-World mirrors, request consumers, or
snapshot paths are out of scope; a slice must instead move the owning data and
side effect into the App world.

| Boundary                                     | Current role                                                                                                                                                                                                     | Deletion gate                                                                                                            |
| -------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| `Runtime::run_app_frame`                     | Ingests raw input, runs egui, stores paint jobs/texture deltas in App resources, and drives the App schedules. App-world render extraction and `RenderSet::Present` own mesh upload and scene/egui presentation. | Runtime only prepares platform/UI input and calls the App schedules; no legacy update/render callback remains.           |
| `FramePipeline::run_frame`                   | Deleted. Its egui context, renderer, menu textures, cache, tessellation, texture upload, and present ordering now live in App Update and `RenderExtractSchedule`.                                                | Keep raw input ingress in Runtime, then delete the remaining State compatibility shell.                                  |
| `State::update`                              | Deleted. App schedules own editor interaction, gameplay, timeline, render extraction, and presentation preparation.                                                                                              | Must remain deleted.                                                                                                     |
| `State::render_egui`                         | Deleted. `RenderSet::Present` calls the resource presenter with App-owned GPU, mesh, egui, uniform, and frame-input resources.                                                                                   | Must remain deleted; remaining pixel tests use direct render fixtures only.                                              |
| `UiStateView`                                | App-owned `UiStateRes::view()` is the only borrowed UI adapter, and all eg-ui helpers now accept `UiStateView` directly; the old `UiStateSnapshot` alias is deleted.                                             | Replace the remaining borrowed adapter with direct resource/query reads, then retire the borrowed `UiStateView` adapter. |
| `State::dispatch` / `State::dispatch_editor` | Deleted command bridge.                                                                                                                                                                                          | Must remain deleted; new handlers may not revive them or a replacement bridge.                                           |

## App-World Ownership Map

This map is the deletion contract for `State`'s remaining subsystem fields.
"Partial" means the named App resource exists but `State` still owns a field or
side effect that must migrate before the compatibility shell can disappear.

| Legacy `State` field | App-world destination                                                                                                                                                                                       | Current status and remaining transfer                                                                                                                                                                                                                                                                                                  |
| -------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `auth`               | `AuthStateRes`, async channel resources                                                                                                                                                                     | Partial: auth request/result state is scheduled, but State still retains the compatibility channel and mirrored fields.                                                                                                                                                                                                                |
| `gameplay`           | `eg_game` components, `GameClock`, `PendingTurnInputs`, progress/resources                                                                                                                                  | Partial: Runtime seeds player/object entities and runs `GameplaySchedule` as the active-play authority; `GameState` remains a render/lifecycle compatibility view, and turn/restart/playtest transitions still need migration.                                                                                                         |
| `editor`             | `EditorInputRes`, `EditorConfigRes`, `EditorObjectsRes`, `EditorSpawnRes`, `EditorCameraRes`, `EditorTriggerRes`, `EditorTimelineRes`, `EditorTimingRes`, `EditorRuntimeRes`, `EditorHistoryRes`, `PerfRes` | Partial: direct handlers use these resources, but ordered object/entity ownership, pick caches, derived meshes, and gizmo interaction presentation remain on State; timeline playback, snapshot/trail extraction, tap-indicator extraction, selection/hover overlay extraction, and transform-trigger marker extraction are App-owned. |
| `audio`              | New App-local audio resource/service plus existing async resources                                                                                                                                          | Not started: State owns playback, SFX, backend switching, and audio import side effects.                                                                                                                                                                                                                                               |
| `session`            | `SessionStateRes` plus App async/import/checkpoint resources                                                                                                                                                | Partial: metadata/settings/pause/practice flags, trigger-base metadata, and checkpoint create/remove are App-owned; level-import channel and lifecycle side effects remain on State.                                                                                                                                                   |
| `menu`               | `MenuStateRes`                                                                                                                                                                                              | Partial: selection and preview metadata plus scheduled preview refresh are App-readable, while level start/editor start still call State lifecycle methods.                                                                                                                                                                            |
| `phase`              | `AppPhaseRes`, then Bevy `States<AppPhase>`                                                                                                                                                                 | Partial: direct carve-outs can change `AppPhaseRes`; lifecycle transitions remain State-owned.                                                                                                                                                                                                                                         |
| `frame_runtime`      | `FrameTimeRes`, `RenderUniformsRes`, and a future player-render/effect resource                                                                                                                             | Partial: frame timing, camera/line extraction, and active block-effect lifecycle are scheduled; compatibility mesh upload and editor runtime bookkeeping remain State-owned.                                                                                                                                                           |
| `render_resources`   | `RenderConfigRes`, `RenderUniformUploadRes`, `RenderGpuRes`, `RenderMeshesRes`, `RenderSurfaceHostRes`, `RenderUniformsRes`, `RenderFrameInputRes`, `RenderEguiRes`                                         | App-owned in production: Runtime moves GPU, scene meshes, surface host, and egui renderer into non-send resources; `RenderExtractSchedule` uploads extracted meshes and `RenderSet::Present` draws scene plus egui. State retains only an empty compatibility slot for test-only legacy callers.                                       |

## Constraints

- `bevy_ecs 0.19`, `bevy_app 0.19`, `bevy_time 0.19`, `bevy_tasks 0.19` (matches
  the existing `bevy_clipboard 0.19` workspace dep).
- `nightly-2026-03-12` toolchain, `edition = "2024"`, `wasm32-unknown-unknown`
  must remain green.
- Clippy `-D warnings` is a hard gate (`bun run lint`).
- Puffin profiling must keep working; `puffin::profile_scope!("name")` wraps
  each `SystemSet`.

## Refactor points (full-Bevy future)

| #   | Site (code)                                                                                 | Current                                                                                                          | Full-Bevy replacement                                                   |
| --- | ------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------- |
| 1   | `crates/eg-app/src/platform/application.rs`                                                 | Custom winit `ApplicationHandler` owns window/canvas events                                                      | `bevy_winit` plugin + `App::run()`                                      |
| 2   | `crates/eg-app/src/platform/runtime.rs`                                                     | `Runtime` keeps the remaining `State` shell beside the App; egui/render helpers are App-owned non-send resources | App resources own egui/render/window state directly                     |
| 3   | `crates/eg-app/src/platform/pipeline.rs`                                                    | Deleted; manual egui/wgpu ordering is currently inline in `Runtime::run_app_frame`                               | `bevy_egui`, `bevy_render`, and Bevy schedule-driven presentation       |
| 4   | `crates/eg-app/src/platform/input_mapping.rs`                                               | winit input translated into project `InputEvent` strings                                                         | `bevy_input` `ButtonInput<KeyCode>` / mouse / touch / gamepad resources |
| 5   | `crates/eg-ui/src/plugin.rs`                                                                | `EguiContextRes` + UI command bridge                                                                             | `bevy_egui` systems writing Bevy events/input directly                  |
| 6   | `crates/eg-game/src/state.rs`                                                               | `GameState` wrapper mirrors ECS world fields                                                                     | `eg-state` consumes gameplay components/resources directly              |
| 7   | `crates/eg-game/src/simulation.rs`                                                          | Timeline scrub uses a private `GameState` wrapper                                                                | Shared/sub `bevy_ecs::World` + `Schedule` for editor and play views     |
| 8   | `crates/eg-game/src/resources.rs`                                                           | `GameClock`, `FixedStepConfig`, `FrameDelta` resources                                                           | `bevy_time` `Time<Fixed>` / `Time<Virtual>` schedules                   |
| 9   | `crates/eg-game/src/plugin.rs`                                                              | Custom `Schedule` + `GameplaySet`                                                                                | Bevy `FixedUpdate` / standard gameplay schedule sets                    |
| 10  | `crates/eg-game/src/events.rs`                                                              | `ConsumedEventsBuffer` compatibility resource                                                                    | Normal Bevy message/event stream                                        |
| 11  | `crates/eg-state/src/events/mod.rs`                                                         | Bevy `Message` taxonomy bridged back to legacy commands                                                          | Direct Bevy message handlers over resources/components                  |
| 12  | `crates/eg-state/src/events/async_results.rs` + `crates/eg-state/src/state/async_bridge.rs` | mpsc channel polling into direct App-world handlers                                                              | `bevy_tasks` streams remove the polling bridge                          |
| 13  | `crates/eg-state/src/events/mesh_rebuild.rs`                                                | Rebuild messages bridge `Changed<>` to legacy mesh dirties                                                       | `bevy_render` extraction / asset events drive render rebuilds           |
| 14  | `crates/eg-state/src/state/state_resources.rs`                                              | `AppPhaseRes` tracks current/previous phase                                                                      | `AppPhase` derives Bevy `States`; wrapper bookkeeping disappears        |
| 15  | `crates/eg-state/src/state/transitions.rs`                                                  | Manual phase-transition detection                                                                                | Bevy `States`, `OnEnter`, and `OnExit` schedules                        |
| 16  | `crates/eg-state/src/state/state_resources.rs`                                              | `FrameTimeRes` advances frame delta/FPS/global time                                                              | `bevy_time` `Time<Virtual>` / `Time<Fixed>` resources                   |
| 17  | `crates/eg-state/src/state/render/plugin.rs` + `crates/eg-state/src/state/update.rs`        | Scheduled color-space POD values are uploaded by `State`                                                         | Render prep/upload lives in `bevy_render` extract/prepare systems       |
| 18  | `crates/eg-state/src/state/plugins.rs`                                                      | Project plugin shells and custom schedule labels                                                                 | Standard Bevy plugin graph and schedule labels                          |
| 19  | `crates/eg-state/src/state/mod.rs`                                                          | Legacy `State` subsystem fields remain authoritative                                                             | State lives in App-world resources/components                           |
| 20  | `crates/eg-state/src/state/input_bridge.rs` + `events::*`                                   | Input/UI command enums translate into typed App-world messages                                                   | Full-Bevy input/action systems without compatibility enums              |
| 21  | `crates/eg-state/src/state/audio_state.rs`                                                  | Custom audio subsystem polled through `State`                                                                    | Bevy audio resources/systems or retained custom backend behind systems  |
| 22  | `crates/eg-state/src/state/render/`                                                         | Custom wgpu resources, extraction, and present bridge                                                            | `bevy_render` render graph, assets, extract schedule, `ShaderRef`       |
| 23  | `crates/eg-state/src/state/test_support.rs`                                                 | `TestApp` wraps the compatibility plugin graph                                                                   | Thin `bevy_app::App` test harness only                                  |

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
  `CameraDrag`) are **not** commands — their message handlers write App-world
  editor input/camera resources directly during the compatibility window. The
  remaining command messages now mutate App-world resources directly or write
  typed service requests consumed inside `StateSchedule`; Runtime no longer
  consumes gameplay/navigation request bridges.
  See the system-set graph below for ordering.

    After Phases 3.5–3.14 the count shifts: 4 snap messages + 1
    block-id message + 2 bpm messages + 5 session messages + 6
    timing messages + 3 spawn messages + 5 camera messages + 3 misc messages (hitbox, perf,
    place-window) + 2 editor-mode messages (SetEditorMode,
    SelectRecentBlock) + 6 timeline-setting messages
    (`ShiftTimeline`, `SetTimelineTime`, `SetTimelineDuration`,
    `SetPlaybackSpeed`, `SetWaveformZoom`, `SetWaveformScroll`) +
    1 timeline-selection message (`SetSelectedTap`) +
    6 timeline playback/tap-array/time messages (`ToggleTimelinePlayback`,
    `AddTap`, `ClearTaps`,
    `SetSelectedTapTime`, `RemoveTap`, `RemoveTapAt`) +
    3 navigation messages (`GameResume`, `NextLevel`,
    `PrevLevel`) +
    6 session metadata/settings messages (`RenameLevel`,
    `SetGraphicsBackend`, `SetUiScaleMultiplier`, `UpdateMusic`,
    `UpdateCreatorMetadata`, `UpdateSkyColor`) +
    4 session keybind messages (`SetKeybind`, `ClearKeybindSlot`,
    `ResetKeybind`, `ResetKeybinds`) +
    2 session menu-preview camera messages
    (`CaptureMenuPreviewCamera`, `UseAutoMenuPreviewCamera`) +
    2 auth/surface messages (`ResizeSurface`, `CopyLatestPerfFrame`) +
    3 auth messages (`AuthSubmitSignIn`, `AuthSignOut`, `AuthOpenSignup`) +
    1 misc message (`Escape`) +
    4 navigation messages (`GameRestartLevel`, `GameSetPracticeCheckpoint`, `GameRemovePracticeCheckpoint`, `GameQuitToMenu`) +
    2 history messages (`Undo`, `Redo`) +
    5 block messages (`RemoveBlock`, `DuplicateBlock`, `CopyBlock`,
    `PasteBlock`, `UpdateSelectedBlock`) +
    `PickBlockAt` + `Playtest` + 5 session side-effect messages +
    `TurnRight` (all menu, gameplay, editor, and game-over branches direct) +
    `ToggleEditor` =
    **all command messages** moved out of the legacy-routed bucket.
    Side-effect-heavy messages now mutate App-world resources or enqueue typed
    service requests handled inside the App schedule.

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
- `StateSchedule` runs before the remaining test-only session compatibility
  sync. Production frames stay entirely App-world owned.

## Migration Checklist (single-world ECS migration plan)

> High-level checklist that mirrors the `Finish Single-World ECS
Migration` plan in `AGENTS.md` / the in-session `plan.md`. The 11
> steps are listed with their current status; each step also points
> to the specific commits / doc sections that prove progress. The
> per-step status is updated as commits land; "in progress" means
> the step is partially done and the next commits will close the
> gap, "blocked" means a prerequisite step is unfinished, "done"
> means the code is in place and the tests are green.

| #   | Step (from plan)                                              | Status          | Evidence                                                                                                                                                                                                                                                                                                                                                                       |
| --- | ------------------------------------------------------------- | --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 1   | Freeze the compatibility boundary and baseline                | **done**        | This section + the per-step `REFACTOR POINT` comments in code.                                                                                                                                                                                                                                                                                                                 |
| 2   | Create the authoritative App-world ownership map              | **in progress** | App resources own all live production values. Legacy `State` seeding remains only in compatibility test construction.                                                                                                                                                                                                                                                          |
| 3   | Move legacy subsystem ownership into App resources            | **in progress** | Production startup installs subsystem and presentation resources directly through `StateAppBootstrap`; no production `State` is constructed. Remaining subsystem fields exist only in compatibility tests.                                                                                                                                                                     |
| 4   | Convert messages from `ToLegacy` to direct handlers by domain | **done**        | All 97 command/input/pointer messages now use direct App-world resource mutation or typed request resources. `LegacyCommand`/`ToLegacy` are deleted; `PickBlockAt`, `Playtest`, session side-effect messages, `TurnRight`, and `ToggleEditor` were the final request/direct carve-outs.                                                                                        |
| 5   | Delete the command compatibility bridge                       | **done**        | `LegacyCommand`/`ToLegacy`, the runtime pending-command drain, `PendingCommands`, `State::dispatch`, `State::dispatch_editor`, `State::process_keyboard_input`, `State::process_input_event`, `command_dispatch.rs`, and `editor_dispatch.rs` are deleted. UI and platform input ingress now translate into typed Bevy messages through `events::*` and `state::input_bridge`. |
| 6   | Move `State::update` into scheduled systems                   | **done**        | `StateSchedule`, `GameplaySchedule`, and `RenderExtractSchedule` own frame mutation and extraction. `State::update` and `state/update.rs` are deleted.                                                                                                                                                                                                                         |
| 7   | Replace manual phase/time compatibility                       | **not started** | `AppPhaseRes` still tracks `previous_app_phase` manually; no `bevy_time::Time<Fixed>` for gameplay; `GameClock` mirrors Bevy time fields.                                                                                                                                                                                                                                      |
| 8   | Move UI off snapshots and into ECS systems                    | **in progress** | `UiStateRes::from_world` derives UI data from App resources, App Update runs egui/tessellation/texture upload, and all helpers use `UiStateView`; direct resource/query reads and eventual `UiStateView` retirement remain.                                                                                                                                                    |
| 9   | Absorb `FramePipeline` into App schedule                      | **in progress** | `FramePipeline` has been deleted. Runtime prepares raw input, egui UI, tessellation, and texture deltas; App owns `RenderEguiRes`, `RenderGpuRes`, `RenderMeshesRes`, `RenderSurfaceHostRes`, and the `RenderSet::Present` scene/egui system. Remaining work is moving the inline UI preparation from Runtime and the last platform/session side effects into App systems.     |
| 10  | Delete legacy `State` and bridge code                         | **in progress** | Production bridge deletion is complete. `State` remains only for compatibility tests and direct GPU pixel fixtures; deletion now depends on the remaining test migrations.                                                                                                                                                                                                     |
| 11  | Migrate tests to `TestApp` as deletion gates                  | **in progress** | `eg-state::state::test_support::TestApp::minimal()` is in place; the 489 eg-state tests + 111 eg-app tests are split between the old `State::new_test` harness and the new `TestApp` harness. Migration is incremental and tracked per step.                                                                                                                                   |

**Carve-out count by domain (post Phase 3.53):**

| Domain              |   Done |  Total | Examples carved out                                                                                                                                                                                                                                                                                                                                                                            |
| ------------------- | -----: | -----: | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| snap                |      4 |      4 | SetSnapToGrid, SetSnapStep, SetSnapRotation, SetSnapRotationStep                                                                                                                                                                                                                                                                                                                               |
| block               |      5 |      5 | RemoveBlock, DuplicateBlock, CopyBlock, PasteBlock, UpdateSelectedBlock                                                                                                                                                                                                                                                                                                                        |
| block-id            |      1 |      1 | SetBlockId (in editor_mode module)                                                                                                                                                                                                                                                                                                                                                             |
| bpm                 |      2 |      2 | BpmTap, BpmTapReset                                                                                                                                                                                                                                                                                                                                                                            |
| session             |     22 |     22 | RenameLevel, SetShowMetadata, SetShowSettings, ToggleSettings, SetSettingsSection, SetGraphicsBackend, SetKeybindCapture, SetKeybind, ClearKeybindSlot, ResetKeybind, ResetKeybinds, SetUiScaleMultiplier, UpdateMusic, UpdateCreatorMetadata, UpdateSkyColor, CaptureMenuPreviewCamera, UseAutoMenuPreviewCamera, LoadLevel, ExportLevel, SetAudioBackend, CompleteImport, TriggerAudioImport |
| timing              |      6 |      6 | AddTimingPoint, RemoveTimingPoint, SetTimingPointTime, SetTimingPointBpm, SetTimingPointTimeSignature, SetTimingSelected                                                                                                                                                                                                                                                                       |
| spawn               |      3 |      3 | SetSpawnHere, RotateSpawnDirection, RotatePlacementPreview (config)                                                                                                                                                                                                                                                                                                                            |
| camera              |      5 |      5 | AdjustZoom, SetCameraOrientation, AddCameraTrigger, AddCameraFollowTrigger, SetTriggerSelected                                                                                                                                                                                                                                                                                                 |
| misc                |      4 |      4 | ToggleHitboxVisualization, TogglePerfOverlay, TogglePlaceWindow (session), Escape (real-gameplay pause + editor selection/playback stop/timeline reset direct; back-to-menu branches request)                                                                                                                                                                                                  |
| auth/surface        |      5 |      5 | ResizeSurface, CopyLatestPerfFrame, AuthSubmitSignIn, AuthSignOut, AuthOpenSignup                                                                                                                                                                                                                                                                                                              |
| editor_mode         |      5 |      5 | SetEditorMode, SetBlockId, SelectRecentBlock, PickSelectedBlock, PickBlockAt                                                                                                                                                                                                                                                                                                                   |
| selection           |      3 |      3 | AddTransformTrigger, SnapSelectionToGrid, FocusCameraTarget                                                                                                                                                                                                                                                                                                                                    |
| navigation          |     10 |     10 | TurnRight (all branches direct), ToggleEditor, GameResume (session game_paused), GameSetPracticeMode (session), NextLevel/PrevLevel (menu selection or editor cursor), GameRestartLevel, GameSetPracticeCheckpoint, GameRemovePracticeCheckpoint, GameQuitToMenu                                                                                                                               |
| timeline            |     14 |     14 | ToggleTimelinePlayback, ShiftTimeline, SetTimelineTime, SetTimelineDuration, AddTap, SetSelectedTap, SetSelectedTapTime, RemoveTap, RemoveTapAt, ClearTaps, SetPlaybackSpeed, SetWaveformZoom, SetWaveformScroll, Playtest                                                                                                                                                                     |
| history             |      2 |      3 | Undo, Redo                                                                                                                                                                                                                                                                                                                                                                                     |
| input_state         |      7 |      7 | (direct resource writes; no command bridge)                                                                                                                                                                                                                                                                                                                                                    |
| pointer             |      5 |      5 | (direct resource writes; no command bridge)                                                                                                                                                                                                                                                                                                                                                    |
| **Carve-out total** | **97** | **97** | Direct writes or request-resource carve-outs; no message handler enters the legacy command bridge.                                                                                                                                                                                                                                                                                             |

No message remains legacy-routed through the command bridge. Session messages
(`LoadLevel`, `ExportLevel`, `SetAudioBackend`, `CompleteImport`, and
`TriggerAudioImport`) and auth now apply through App-world resources/services.
Resize, perf, and auth request resources are schedule-local service intents;
gameplay/navigation messages no longer depend on Runtime request consumers.

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
- [x] **Phase 4e-scaffold** — `present_render_frame_placeholder_system` reads the POD resources; the actual wgpu present call from the inline `eg-app/src/platform/runtime.rs` presentation path stays in place.
- [x] **Phase 4b-final** — move `State::render.gpu` ownership into `RenderGpuRes`; direct call sites now route through `State::render_gpu()` / `render_gpu_mut()` backed by the world.
- [x] **Phase 4c-final** — `RenderMeshesRes` / `RenderSurfaceHostRes` now own the actual `SceneMeshes` / `SurfaceHost`; legacy mesh/surface call sites route through `State` accessors backed by the world.
- [x] **Phase 4d-final** — `ObjectBundle` mirror components + `Changed<>` detection now drive `mark_mesh_dirty_system`; `EditorDirtyFlags` remains as the legacy consumer until editor mutation moves into the shared World.
- [x] **Phase 4e-final** — `present_render_frame_system` is registered in `RenderSet::Present` and reads authoritative render resources; the production `render_egui` compatibility call remains until Phase 7 gives the Bevy app ownership of the frame.
- [x] **Phase 5** — UI as Bevy systems.
- [x] **Phase 6a** — async bridging scaffold (5 channel wrapper resources + `PollChannelsSet` + 5 result events).
- [x] **Phase 6b** — replace legacy `update_*` methods with handler systems that consume result events.
- [x] **Phase 6c** — frame timing carve-out (`FrameTimeRes` + `advance_frame_time_system`) moves frame delta, FPS smoothing, and global time into `StateSchedule`; legacy `State::update` consumes the scheduled delta during the compatibility window.
- [x] **Phase 6d** — color-space uniform carve-out (`update_color_space_uniform_system`) makes `RenderUniformsRes.color_space.time_seconds` follow App-world `FrameTimeRes`; legacy `State::update` uploads the scheduled POD uniform during the compatibility window and keeps a direct-call fallback.
- [x] **Phase 6e** — schedule boundary cleanup: every per-message handler group is tagged with `CommandHandlerSet`, and `StateSchedule` orders `CommandHandlerSet -> PollChannelsSet -> ApplyAsyncResultsSet` so command mutation completes before async polling/result application.
- [x] **Phase 6f** — phase transition scheduling: `phase_transition_schedule_system` now runs inside `StateSchedule`, so Runtime and `TestApp` no longer call phase transition detection as a separate prelude.
- [x] **Phase 6g** — scheduled render extraction: `RenderFrameInputRes` and `RenderUniformsRes` are rebuilt from App-world phase, editor, gameplay, camera, menu, and frame-time resources in the post-gameplay `RenderExtractSchedule`; camera and line buffers are uploaded there, and the compatibility update skips duplicate uniform uploads when the scheduled phase matches.
- [x] **Phase 7** — `Runtime` owns only a `bevy_app::App`; `Runtime::run_app_frame(raw_input)` drives App Update, `StateSchedule`, `GameplaySchedule`, `RenderExtractSchedule`, and `RenderSet::Present`. Native and WASM startup install resources through `StateAppBootstrap`; no production `State`, `FramePipeline`, legacy update callback, or State render callback remains.
- [x] **Phase 8** — `TestApp` harness scaffold: `TestApp::minimal()` builder in `eg-state::state::test_support` builds a `bevy_app::App` with the eg-state plugin graph + `StateSchedule`; `tick_state()` mirrors `State::tick_state()`; `run_state_schedule()` runs the schedule only; `assert_resource_exists::<T>()` helper. `eg-game::TestApp` builds a headless gameplay app with `GamePlugin` + `tick_gameplay()`. Migration of the existing `&State`/`&GameState` tests to the new harness is incremental and tracked in follow-up commits.
- [x] **Phase 9** — documentation pass: refactor-point registry, event taxonomy, system-set graph, plugin replacement table, and AGENTS architecture notes are current.
- [x] **Phase 3.5 / Snap carve-out** — `events::snap` handlers (4 messages) migrated to write `EditorConfigRes.0.snap_*` directly; the original `Runtime::sync_app_editor_config_state` mirror was deleted in Phase 3.153 after normalization, persistence, history, and selected-object resnapping moved into App handlers. 4 schedule-level tests in `events::tests` + 5 end-to-end tests in `runtime::tests` exercise the full pipeline. The input_state + pointer carve-out pattern is now extended to a stateful-resource domain (the snap config), proving the migration works for resources with setter side effects.
- [x] **Phase 3.6 / Block-id carve-out + `EditorConfigRes` seeding** — `events::editor_mode::handle_set_block_id` migrated to write `EditorConfigRes.0.selected_block_id` directly; The original config mirror used `set_editor_block_id` for recent-block side effects; Phase 3.153 moved MRU ownership into App handlers and deleted the mirror. `Runtime::new` now seeds `EditorConfigRes` from the legacy `State` via `State::editor_config_resource()`, fixing a latent first-frame clobber where the Default-initialized resource would overwrite user-loaded `app_settings` snap/block config on the first frame. 1 schedule-level test in `events::tests` + 3 end-to-end tests in `runtime::tests` (block-id propagation, no-op resync guard, first-frame no-clobber regression).
- [x] **Phase 3.7 / BPM carve-out + `EditorTimingRes` seeding** — `events::bpm::handle_bpm_tap` and `handle_bpm_tap_reset` migrated to write `EditorTimingRes.0.bpm_tap_times` / `bpm_tap_result` directly. The bpm-tap calculation (wall-clock-time read, append to history, recompute running BPM as 60 / average diff, truncate to 16 entries) is now performed in the App world, mirroring the legacy `State::editor_bpm_tap` logic. `Runtime::sync_app_editor_timing_state` change-guarded sync propagates the resource to the legacy `State.editor.timing` fields. Phase 3.150 later deleted this test-only mirror. `Runtime::new` seeds `EditorTimingRes` from the legacy state via `State::editor_timing_resource()` (which required `EditorTimingState: Clone`), so the first frame is a no-op for an in-flight tap history. 2 schedule-level tests in `events::tests` (`carve_out_bpm_tap_mutates_editor_timing_resource`, `carve_out_bpm_tap_reset_clears_editor_timing_resource`) + 4 end-to-end tests in `runtime::tests` (BpmTap propagation, BpmTapReset propagation, no-op resync guard, first-frame no-clobber regression).
- [x] **Phase 3.8 / Session carve-out + `SessionStateRes` seeding** — `events::session::handle_set_show_metadata`, `handle_set_show_settings`, `handle_toggle_settings`, `handle_set_settings_section`, and `handle_set_keybind_capture` migrated to write `SessionStateRes` fields directly. Later Phase 3.19 added `SetUiScaleMultiplier`, Phase 3.20 added `UpdateSkyColor`, Phase 3.21 added `UpdateCreatorMetadata`, Phase 3.22 added `RenameLevel` + `UpdateMusic`, Phase 3.23 added `SetGraphicsBackend`, Phase 3.24 added the 4 keybind mapping messages, and Phase 3.25 added the 2 menu-preview camera messages to this same direct-resource bucket. At this intermediate phase, side-effect-heavy session events still used typed Runtime requests; Phases 3.89-3.95 subsequently moved import pickers, built-in load, export, and audio-backend switching to App-owned resources/services. `Runtime::new` seeds `SessionStateRes` from the legacy `State.session`. Schedule-level and end-to-end runtime tests cover per-message propagation, first-frame seeding, stale-resource authority, and locked guards.
- [x] **Phase 3.9 / Timing carve-out (extends `EditorTimingRes`)** — `events::timing::handle_add_timing_point`, `handle_remove_timing_point`, `handle_set_timing_point_time`, `handle_set_timing_point_bpm`, `handle_set_timing_point_time_signature`, and `handle_set_timing_selected` migrated to write `EditorTimingRes.0.timing_points` and `timing_selected_index` directly. The bpm-tap fields (Phase 3.7) + the timing-points fields now share the same `EditorTimingRes` resource and the same `Runtime::sync_app_editor_timing_state` change-guarded sync. Phase 3.150 later deleted this test-only mirror. The handlers mirror the legacy `EditorSubsystem::add_timing_point` / `update_timing_point_time` / ... behavior (sort ascending by `time_seconds`, clamp BPM to `>= 1.0`, clamp time-signature numerator/denominator to `>= 1`, bump `revision` on every change). Direct `EditorCommand` tests continue to cover the legacy dispatch methods separately; Phase 3.38 removed the message `ToLegacy` impls after this direct-handler path was gated. 7 schedule-level tests in `events::tests` (one per carve-out message, plus an out-of-range RemoveTimingPoint no-op test) + 8 end-to-end tests in `runtime::tests` (per-message propagation, no-op resync guard, first-frame no-clobber regression that exercises both `timing_points` and `timing_selected_index`).
- [x] **Phase 3.10 / Spawn carve-out + new `EditorSpawnRes` resource** — `events::spawn::handle_set_spawn_here` and `handle_rotate_spawn_direction` migrated to write `EditorSpawnRes.0.position` and `.direction` directly. `events::spawn::handle_rotate_placement_preview` writes `EditorConfigRes.0.selected_block_rotation_degrees[1]` directly (extending the existing config carve-out with the placement-preview yaw field). The new `EditorSpawnRes` resource wraps `SpawnMetadata`; `State::editor_spawn_resource()` seeds it from the legacy `State.editor.spawn` at `Runtime::new`; `Runtime::sync_app_editor_spawn_state` originally propagated the carved-out fields and was deleted in Phase 3.147. The temporary placement-rotation config mirror was deleted in Phase 3.153 once the App resource became the sole presentation source. 3 schedule-level tests in `events::tests` (one per carve-out message) + 5 end-to-end tests in `runtime::tests` (per-message propagation + a no-op resync guard + a first-frame no-clobber regression that exercises the spawn position + direction).
- [x] **Phase 3.11 / Camera + trigger-selected carve-out (partial) + new `EditorCameraRes` Clone** — The first 3 of the 5 `events::camera::*` messages migrated to carve-outs: `handle_adjust_zoom` writes `EditorCameraRes.0.editor_pan` / `editor_target_z`; `handle_set_camera_orientation` writes `EditorCameraRes.0.editor_yaw` / `editor_pitch` / `transition`; `handle_set_trigger_selected` writes `EditorTriggerRes.0.selected_index`. `EditorCameraState` and `CameraTransition` gained manual `Clone` impls (the previous `Default` impl was hand-rolled and `Clone` was missing). `State::editor_camera_resource()` and `State::editor_trigger_resource()` seed the resources at `Runtime::new`; `Runtime::sync_app_editor_camera_state` and `Runtime::sync_app_editor_trigger_state` originally propagated the carved-out fields; Phases 3.146 and 3.148 deleted both mirrors. Phase 3.43 later completed this domain by moving `AddCameraTrigger` and `AddCameraFollowTrigger`.
- [x] **Phase 3.12 / Misc carve-out (hitbox, perf, place-window) + new `PerfRes` seeding** — 3 of the 4 `events::misc::*` messages migrated to carve-outs: `handle_toggle_hitbox_visualization` writes `EditorInputRes.0.show_hitbox_visualization` directly (extending the input carve-out with the hitbox-toggle field); `handle_toggle_perf_overlay` writes `PerfRes.0.overlay_enabled` directly; `handle_toggle_place_window` writes `SessionStateRes.editor_show_place_window` directly (extending the session carve-out with the place-window field). `Escape` later moved fully into App-owned phase/session/editor resources in Phase 3.101. `At this intermediate phase `State::editor_hitbox_visualization()`and`State::set_editor_hitbox_visualization(...)`were exposed so Runtime could invoke the input mirror; Phase 3.152 deleted both bridge-only APIs;`State::set_perf_overlay_enabled(...)`is added so the runtime sync can invoke the`puffin::set_scopes_on`side effect.`Runtime::sync_app_editor_input_state`extended with a change-guarded`show_hitbox_visualization`sync; Phase 3.152 later deleted that test-only mirror once render extraction consumed`EditorInputRes`directly;`Runtime::sync_app_session_state`extended with a change-guarded`editor_show_place_window`sync; new`Runtime::sync_app_perf_state`change-guarded sync propagates`overlay_enabled`(and invokes the legacy`puffin::set_scopes_on`side effect when the value differs).`State::perf_resource()`seeds`PerfRes`from the legacy`State.editor.perf`at`Runtime::new`(only`overlay_enabled`is part of the carve-out;`fps_smoothed`and`global_frame_view`are runtime-derived and stay legacy-authoritative). 8 end-to-end tests in`runtime::tests`cover per-message propagation × 3, no-op resync guard × 3, and first-frame no-clobber regression for`PerfRes`.
- [x] **Phase 3.13 / Editor-mode carve-out (SetEditorMode)** — `events::editor_mode::handle_set_editor_mode` migrated to write `EditorInputRes.0.mode` directly. At this intermediate phase the legacy `State.editor.input.mode` was synced from the resource by `Runtime::sync_app_editor_input_state`; Phase 3.152 later deleted that test-only mirror. The legacy `set_editor_mode` has heavy side effects (rebuilds gizmo/hover/selection outline vertices, marks `EditorDirtyFlags::tap_timeline_changed()`); the change guard in the runtime invokes the legacy setter only on real user-driven changes, keeping the vertex-rebuild cost off the per-frame path. The Tapping-auxiliary side effects (tap selection / preview refresh on Tapping↔other transitions) live in `dispatch_editor` rather than in `set_editor_mode` itself; the carve-out sync goes through `set_editor_mode` (matching the carve-out pattern: the App world writes the value, the legacy setter is invoked once per real change, and the dispatch path is the existing one). 1 schedule-level test in `events::tests` (`carve_out_set_editor_mode_mutates_editor_input_resource`) + 2 end-to-end tests in `runtime::tests` (`runtime_carve_out_set_editor_mode_propagates_to_legacy_state`, `runtime_carve_out_no_op_frame_does_not_resync_editor_mode`).
- [x] **Phase 3.14 / SelectRecentBlock carve-out** — `events::editor_mode::handle_select_recent_block` migrated to write `EditorConfigRes.0.selected_block_id` and `EditorInputRes.0.mode` directly. The handler reads `EditorConfigRes.0.recent_block_ids`, filters to placeable blocks via `eg_blocks::BlockDefinition::get` (mirroring the legacy `select_recent_block` filter), picks the indexed entry, and writes the result. At this intermediate phase change-guarded Runtime syncs propagated both values into legacy State; Phase 3.152 deleted the input half and Phase 3.153 moved recent-block behavior into App handlers before deleting the config half; the legacy setters handle the recent-blocks list, vertex rebuilds, and dirty-flag marks once per real user-driven change. `PickBlockAt` and `PickSelectedBlock` have since moved to App-world editor-object/resource handlers; no editor-mode message stages `PendingCommands`. 1 schedule-level test in `events::tests` (`carve_out_select_recent_block_mutates_resources`) verifies the placeable filter, the out-of-range no-op, the index-0/index-1 picks, and the `EditorMode::Place` switch — all without staging a legacy editor command.
- [x] **Phase 3.15 / Navigation carve-out (GameSetPracticeMode) + SessionStateRes.practice_mode_enabled seeding** — `events::navigation::handle_game_set_practice_mode` migrated to write `SessionStateRes.practice_mode_enabled` directly (extending the Phase 3.8 session carve-out with the practice-mode field). The legacy `set_practice_mode_enabled` setter has heavy side effects (phase-guard, clears `practice_checkpoints`, calls `rebuild_practice_checkpoint_vertices`, `restart_level`, and `resume_game`); the change guard in `Runtime::sync_app_session_state` invokes the legacy setter only on real user-driven changes, keeping the level-restart cost off the per-frame path. The internal `set_practice_mode_enabled_internal` is exposed via a new public `State::set_practice_mode_enabled` setter (in `editor_state.rs`); the public `State::practice_mode_enabled_value()` getter exposes the raw `session.practice_mode_enabled` for the runtime sync to compare against. `State::session_state_resource()` now seeds `practice_mode_enabled` from the legacy state at `Runtime::new` (the `..Default::default()` fallback was a latent first-frame clobber: a user who toggled practice mode on a previous frame would see the resource reset to `false` and the change-guarded sync would fire `restart_level` + `resume_game` again on the first frame). Test-only setters (`set_phase_for_test`, `set_playtesting_editor_for_test`, `set_session_practice_mode_enabled_for_test`) are exposed in `state/state_helpers.rs` for the eg-app runtime tests to drive the legacy state into `AppPhase::Playing` without going through `enter_playing_phase`'s full init (which would clobber the seeded practice-mode value). 1 schedule-level test in `events::tests` (`carve_out_game_set_practice_mode_mutates_session_resource`) verifies the on/off write; the `state::tests` gate is now `test_app_tick_state_practice_mode_mutates_session_resource`, which runs through `TestApp` without constructing legacy `State`. 3 end-to-end tests in `runtime::tests` (`runtime_carve_out_game_set_practice_mode_propagates_to_legacy_state`, `runtime_carve_out_no_op_frame_does_not_resync_practice_mode`, `runtime_seeds_practice_mode_from_legacy_state_no_first_frame_clobber`) exercise the full message → resource → sync → legacy state pipeline including the first-frame no-clobber regression.

- [x] **Phase 3.16 / Timeline setting carve-out (`EditorTimelineRes` seeding + `EditorTimingRes` extension)** — 6 of the 14 `events::timeline::*` messages are now carve-outs: `handle_shift_timeline` and `handle_set_timeline_time` write `EditorTimelineRes.0.clock.time_seconds`; `handle_set_timeline_duration` writes `EditorTimelineRes.0.clock.{duration_seconds,time_seconds}`; `handle_set_playback_speed`, `handle_set_waveform_zoom`, and `handle_set_waveform_scroll` write `EditorTimingRes.0.{playback_speed,waveform_zoom,waveform_scroll}` directly. `Runtime::new` now seeds `EditorTimelineRes` from the legacy `State` via `State::editor_timeline_resource()` so the first frame does not clobber user timeline position/duration; `Runtime::sync_app_editor_timeline_state` change-guards the legacy `set_editor_timeline_duration_seconds` / `set_editor_timeline_time_seconds` setters, and `Runtime::sync_app_editor_timing_state` now also syncs playback speed + waveform zoom/scroll through their legacy setters. The remaining timeline messages at this point (`ToggleTimelinePlayback`, `AddTap`, `RemoveTap`, `RemoveTapAt`, `SetSelectedTap`, `SetSelectedTapTime`, `ClearTaps`, `Playtest`) stayed on the legacy `PendingCommands` path because they still drove tap/playback state machines, history, preview caches, or audio/startup side effects that needed a dedicated App-world ownership pass. 6 schedule-level tests in `events::tests` pin the resource writes and empty `PendingCommands` buffer, and 7 end-to-end tests in `eg-app` `runtime::tests` pin propagation, no-op sync guards, and first-frame seeding.

- [x] **Phase 3.17 / Timeline selection carve-out (`SetSelectedTap`)** — `events::timeline::handle_set_selected_tap` now writes `EditorTimelineRes.0.taps.selected_index` directly instead of staging `EditorCommand::SetSelectedTap`. The tap arrays themselves (`tap_times` + `tap_indicator_positions`) remain legacy-authored for now, so `State::prepare_app_world_for_state_schedule` mirrors those arrays into `EditorTimelineRes` before the schedule runs; this gives the App-world handler the real validation data it needs without pretending tap insertion/removal/time-edit history already moved. `Runtime::sync_app_editor_timeline_state` now change-guards the legacy `set_editor_selected_tap_index` side effects (cursor move + tap-indicator rebuild) after syncing the clock fields. `State::editor_timeline_resource()` now seeds tap arrays + `selected_index` at `Runtime::new`, closing the same first-frame clobber window for timeline selection that earlier carve-outs hit in config/session/timing. At this point, `SetSelectedTapTime`, `AddTap`, `RemoveTap`, `RemoveTapAt`, `ClearTaps`, `ToggleTimelinePlayback`, and `Playtest` stayed on the legacy `PendingCommands` path because they still mutated tap arrays, history, preview caches, playback runtime, or audio/playtest startup state machines; Phase 3.28 later moves `ClearTaps`, and Phase 3.29 later moves `SetSelectedTapTime`. 1 schedule-level test in `events::tests` pins the resource write and empty `PendingCommands` buffer, and the `eg-app` runtime tests extend the timeline carve-out coverage with selection propagation plus no-op/seed assertions.

- [x] **Phase 3.18 / Navigation carve-out (`GameResume`)** — `events::navigation::handle_game_resume` now writes `SessionStateRes.game_paused = false` directly instead of staging `AppCommand::GameResume`. `Runtime::sync_app_session_state` applies the existing legacy `resume_game` side-effect path only when the legacy state is paused and the App resource requests resume, preserving audio/gameplay startup behavior. Because pausing still happens through legacy paths such as Escape, `State::prepare_app_world_for_state_schedule` mirrors `State.session.game_paused` into `SessionStateRes.game_paused` before the schedule runs; a stale resource can no longer immediately undo a legacy pause on the next frame. `GameRestartLevel`, `GameQuitToMenu`, practice checkpoints, and phase/menu navigation stay legacy-routed because they still own gameplay restart, checkpoint, phase, audio, or menu side effects. 1 schedule-level test pins the direct resource write and empty `PendingCommands` buffer, and 2 `eg-app` runtime tests cover message propagation plus the stale-resource/legacy-pause guard.

- [x] **Phase 3.19 / Session settings carve-out (`SetUiScaleMultiplier`)** — `events::session::handle_set_ui_scale_multiplier` now writes `SessionStateRes.app_settings.ui_scale_multiplier` directly instead of staging `EditorCommand::SetUiScaleMultiplier`. The handler uses `AppSettings::clamp_ui_scale_multiplier`, mirroring the legacy clamp. `Runtime::sync_app_session_state` compares only this one `AppSettings` field and calls the existing legacy `set_ui_scale_multiplier` setter when it differs, preserving the current persistence side effect without pretending keybinds/backend/level-progress settings have moved. Because direct legacy callers can still mutate `State.session.app_settings`, `State::prepare_app_world_for_state_schedule` mirrors the legacy UI scale into `SessionStateRes` before the schedule runs; a stale resource can no longer clobber a legacy settings change on a no-message frame. Backend selection, keybind writes, level progress, and metadata/audio import session messages stay legacy-routed until their side effects have App-world owners. 1 schedule-level test pins the direct resource write and empty `PendingCommands` buffer, and 3 `eg-app` runtime tests cover message propagation, stale-resource protection, and first-frame seeding.

- [x] **Phase 3.20 / Session sky-color carve-out (`UpdateSkyColor`)** — `events::session::handle_update_sky_color` now writes `SessionStateRes.editor_sky_color` directly instead of staging `EditorCommand::UpdateSkyColor`. The handler clamps color components to `[0.0, 1.0]`, matching the legacy `set_editor_sky_color` setter. `Runtime::sync_app_session_state` calls that setter only when the resource differs, keeping legacy render/UI paths in sync without re-routing through `PendingCommands`. `State::prepare_app_world_for_state_schedule` no longer mirrors legacy sky color into `SessionStateRes`; the App resource is the live sky-color value after Runtime seeding, and a runtime regression pins that stale legacy `State.session.editor_sky_color` cannot clobber it on a no-message frame. Backend/keybind settings, import/export, and audio backend session messages keep typed-request or compatibility sync paths until their side effects have App-world owners. 1 schedule-level test pins the direct resource write and empty `PendingCommands` buffer, and 2 `eg-app` runtime tests cover message propagation plus App-resource authority; the existing session first-frame seeding regression now includes sky color.

- [x] **Phase 3.21 / Creator metadata carve-out (`UpdateCreatorMetadata`)** — `events::session::handle_update_creator_metadata` now writes `SessionStateRes.editor_creator_metadata` directly instead of staging `EditorCommand::UpdateCreatorMetadata`. The legacy setter is a pure assignment, so `Runtime::sync_app_session_state` uses it as a change-guarded compatibility copy while export/UI metadata reads still come from legacy `State.session`. `State::prepare_app_world_for_state_schedule` no longer mirrors legacy creator metadata into `SessionStateRes`; the App resource is the live value after Runtime seeding, and a runtime regression pins that stale legacy creator metadata cannot clobber it on a no-message frame. Level load refreshes the App resource after its legacy side effect. 1 schedule-level test pins the direct resource write and empty `PendingCommands` buffer, and `eg-app` runtime tests cover message propagation, App-resource authority, and first-frame seeding.

- [x] **Phase 3.22 / Basic level metadata carve-out (`RenameLevel`, `UpdateMusic`)** — `events::session::handle_rename_level` and `handle_update_music` now write `SessionStateRes.editor_level_name` and `SessionStateRes.editor_music_metadata` directly instead of staging `EditorCommand::RenameLevel` / `EditorCommand::UpdateMusic`. Both legacy setters are pure session-field writes, so `Runtime::sync_app_session_state` uses them as change-guarded compatibility copies while export/UI metadata reads still come from legacy `State.session`. `State::prepare_app_world_for_state_schedule` no longer mirrors legacy level name or music metadata into `SessionStateRes`; the App resource is the live value after Runtime seeding, and a runtime regression pins that stale legacy name/music cannot clobber it on a no-message frame. Level load refreshes the App resource after its legacy side effect. 2 schedule-level tests pin the direct resource writes and empty `PendingCommands` buffer, and `eg-app` runtime tests cover message propagation, App-resource authority, and first-frame seeding.

- [x] **Phase 3.23 / Graphics backend preference carve-out (`SetGraphicsBackend`)** — `events::session::handle_set_graphics_backend` now writes `SessionStateRes.app_settings.graphics_backend` directly when graphics backend selection is not locked, and marks `SessionStateRes.settings_restart_required` when the preference changes. The handler mirrors the legacy lock/no-op guards, while `Runtime::sync_app_session_state` still calls `State::set_preferred_graphics_backend` as the compatibility point so restart-required bookkeeping and settings persistence keep using the established legacy path. `State::prepare_app_world_for_state_schedule` mirrors the legacy graphics backend preference, restart flag, and lock into `SessionStateRes` before the schedule runs, preventing stale App settings from clobbering direct legacy updates and ensuring locked startup overrides are honored. `SetAudioBackend`, level load/import/export, and menu-preview camera session messages stay legacy-routed until their live side effects have App-world owners. 2 schedule-level tests pin the direct write plus locked guard, and 3 `eg-app` runtime tests cover message propagation, stale-resource protection, and locked-backend rejection; the existing session first-frame seeding regression now includes graphics backend preference and restart-required state.

- [x] **Phase 3.24 / Keybind settings carve-out (`SetKeybind`, `ClearKeybindSlot`, `ResetKeybind`, `ResetKeybinds`)** — `events::session` keybind handlers now mutate `SessionStateRes.app_settings.keybinds` directly through the shared `AppSettings` helpers, preserving key normalization, duplicate handling, and default reset semantics without staging `EditorCommand` variants. `Runtime::sync_app_session_state` mirrors the final keybind vector back through `State::set_app_settings_keybinds` as the temporary compatibility point so legacy settings persistence and UI reads remain intact. `State::prepare_app_world_for_state_schedule` mirrors legacy keybinds into `SessionStateRes` before the schedule runs, preventing stale App settings from clobbering direct legacy updates on no-message frames. 4 schedule-level tests pin set/clear/reset/reset-all resource writes and empty `PendingCommands`; 5 `eg-app` runtime tests cover per-message propagation plus stale-resource protection, and the session first-frame seeding regression now includes keybinds.

- [x] **Phase 3.25 / Menu-preview camera carve-out (`CaptureMenuPreviewCamera`, `UseAutoMenuPreviewCamera`)** — the session handlers now mutate `SessionStateRes.editor_menu_preview_camera` directly. `CaptureMenuPreviewCamera` reads `AppPhaseRes` and `EditorCameraRes`, preserves the legacy Editor-only phase guard, and stores the current editor camera pose as `LevelPreviewCameraMetadata`; `UseAutoMenuPreviewCamera` clears the manual camera only in Editor. `Runtime::sync_app_session_state` mirrors the resource back through `State::set_editor_menu_preview_camera` for legacy UI/export compatibility, and `State::prepare_app_world_for_state_schedule` no longer mirrors legacy menu-preview metadata before the schedule. A runtime regression pins that stale legacy menu-preview metadata cannot clobber the App resource on a no-message frame. 3 schedule-level tests cover capture, clear, and phase-guard behavior; `eg-app` runtime tests cover propagation, App-resource authority, and first-frame seeding.

- [x] **Phase 3.26 / Menu navigation partial carve-out (`NextLevel`, `PrevLevel` menu branch) + `MenuStateRes` seeding** — `Runtime::new` now seeds `MenuStateRes` from legacy `State.menu`, and `Runtime::sync_app_menu_state` change-guards `MenuStateRes.selected_level` back into legacy state for current UI/render compatibility. `events::navigation::handle_next_level` and `handle_prev_level` read `AppPhaseRes`: in `Menu`, they mutate `MenuStateRes.selected_level` directly with wraparound and do not stage `PendingCommands`. At this historical point, the non-menu branch still used the legacy command path; Phase 3.27 completed the editor cursor branch and promoted both messages to full carve-outs. `State::prepare_app_world_for_state_schedule` no longer mirrors legacy menu selection into `MenuStateRes`; the App resource is the live menu selection after Runtime seeding, and a runtime regression pins that stale legacy `State.menu` cannot clobber it on a no-message frame. 2 schedule-level tests pin menu direct writes plus editor fallback; 2 `eg-app` runtime tests cover propagation and App-resource authority.

- [x] **Phase 3.27 / Navigation carve-out completion (`NextLevel`, `PrevLevel` editor cursor branch) + `EditorInputRes` seeding** — `events::navigation::handle_next_level` and `handle_prev_level` now mutate `EditorInputRes.0.cursor` directly in `AppPhase::Editor`, using the same snap-step calculation as legacy `EditorSubsystem::move_cursor` (`snap_to_grid && !ctrl_held` → `snap_step.max(0.05)`, otherwise `1.0`). In `Menu`, they keep mutating `MenuStateRes.selected_level`; outside Menu/Editor they no-op, matching legacy `State::next_level` / `prev_level`. `Runtime::new` seeds `EditorInputRes` once from legacy `State.editor.input`; Phase 3.152 deleted the later cursor copy-back after App render extraction became authoritative. The old `Runtime::dispatch_app_pending_commands` refresh described in this phase has since been removed after all message handlers stopped writing `PendingCommands`. 3 schedule-level tests cover menu selection, editor cursor movement with snap, and no-op behavior outside Menu/Editor; 1 `eg-app` runtime test covers UI-command → authoritative App cursor mutation.

- [x] **Phase 3.28 / Timeline tap-array carve-out (`ClearTaps`) + tap-state sync** — `events::timeline::handle_clear_taps` now clears `EditorTimelineRes.0.taps.{tap_times,tap_indicator_positions,selected_index}` directly instead of staging `EditorCommand::ClearTaps`. `Runtime::sync_app_editor_timeline_state` now compares the tap arrays before selected-index sync and uses `State::set_editor_tap_state` as the temporary compatibility point; that setter records editor history, copies the arrays into legacy `State.editor.timeline.taps`, invalidates samples, and refreshes tap preview/indicator state once per real resource change. `State::prepare_app_world_for_state_schedule` still mirrors legacy tap arrays before the schedule so direct legacy callers are protected on no-message frames. 1 schedule-level test pins the resource write + empty `PendingCommands`; 1 `eg-app` runtime test covers UI-message → App tap arrays → legacy tap arrays propagation.

- [x] **Phase 3.29 / Timeline selected-tap time carve-out (`SetSelectedTapTime`)** — `events::timeline::handle_set_selected_tap_time` now runs the selected-tap retime directly over App resources instead of staging `EditorCommand::SetSelectedTapTime`. The handler preserves the legacy guards (`AppPhase::Editor`, finite time, selected tap exists, no-op within `0.0001s`), clamps the target time to `EditorTimelineRes.0.clock.duration_seconds`, removes the selected tap from the sorted arrays, reinserts it by time with its existing indicator position, updates `selected_index`, moves `EditorTimelineRes.0.clock.time_seconds`, and moves `EditorInputRes.0.cursor` to the selected tap position. At this intermediate phase Runtime propagated cursor, clock, tap arrays, and selection into legacy `State`; Phase 3.152 later removed the input mirror while timeline compatibility now writes any derived selected-tap cursor directly to `EditorInputRes`; because legacy still owns object/spawn-derived tap indicator refresh, the timeline sync copies legacy's final derived tap indicators and selected-tap cursor back into App resources before the frame continues. 1 schedule-level test pins reorder/clamp/cursor behavior + empty `PendingCommands`; 1 `eg-app` runtime test covers UI-message → App tap arrays/time/cursor → legacy state propagation plus App/legacy convergence after derived indicator refresh.

- [x] **Phase 3.30 / Timeline tap removal carve-out (`RemoveTap`, `RemoveTapAt`)** — `events::timeline::handle_remove_tap` and `handle_remove_tap_at` now mutate `EditorTimelineRes.0.taps.{tap_times,tap_indicator_positions,selected_index}` directly instead of staging legacy editor commands. The handlers use the shared `eg_editor_domain::remove_tap_with_indicator` epsilon behavior, remove by current timeline time or explicit time, shift `selected_index` when an earlier tap is removed, and clear selection when the selected tap is removed. `Runtime::sync_app_editor_timeline_state` uses the same tap-state compatibility path from Phases 3.28-3.29, so legacy records history/refreshes derived tap indicators once per real tap-array change and the App resource receives the final derived indicators back before the frame continues. 1 schedule-level test pins remove-current/remove-at selection behavior + empty `PendingCommands`; 1 `eg-app` runtime test covers message → App tap arrays → legacy state propagation and App/legacy convergence.

- [x] **Phase 3.31 / Timeline tap insertion carve-out (`AddTap`)** — `events::timeline::handle_add_tap` now mutates `EditorTimelineRes.0.taps.{tap_times,tap_indicator_positions,selected_index}` directly instead of staging `EditorCommand::AddTap`. The handler derives the tap indicator from the App-world preview position, uses the shared `eg_editor_domain::add_tap_with_indicator` epsilon/ordered-insert behavior, and selects either the newly inserted tap or an existing epsilon-matched tap. `Runtime::sync_app_editor_timeline_state` continues to use `State::set_editor_tap_state` as the temporary compatibility point so legacy history, sample invalidation, and derived tap-indicator refresh still run once per real tap-array change, then the final legacy-derived indicators are copied back into App resources. 1 schedule-level test pins insert/duplicate-selection behavior + empty `PendingCommands`; 1 `eg-app` runtime test covers message → App tap arrays → legacy state propagation and App/legacy convergence.

- [x] **Phase 3.32 / Timeline playback toggle carve-out (`ToggleTimelinePlayback`)** — `events::timeline::handle_toggle_timeline_playback` now mutates `EditorTimelineRes.0.playback.playing`, clears App-world playback seek/runtime state, and swaps `EditorInputRes.0.mode` through `EditorRuntimeRes.0.interaction.last_mode` instead of staging `EditorCommand::ToggleTimelinePlayback`. `Runtime::sync_app_editor_timeline_state` calls `State::set_editor_timeline_playback_active` as the temporary compatibility point so legacy audio start/stop, playback runtime construction, dirty flags, and mode restoration stay intact while the message itself is App-world authoritative. `Runtime::new` and `State::prepare_app_world_for_state_schedule` now seed/mirror the playback bit and last mode so stale resources cannot clobber legacy playback changes on no-message frames. 1 schedule-level test pins start/stop mode swapping + empty `PendingCommands`; 1 `eg-app` runtime test covers message → App playback/mode resources → legacy playback/mode propagation.

- [x] **Phase 3.33 / Navigation fallback cleanup (`NextLevel`, `PrevLevel`)** — `events::navigation::handle_next_level` and `handle_prev_level` no longer take `PendingCommands` or fall back to `AppCommand::NextLevel` / `AppCommand::PrevLevel` when `MenuStateRes` is absent. The handlers remain direct-resource carve-outs in their Menu and Editor branches and now no-op if the menu resource is unavailable, matching the App-world ownership rule that carved-out messages must not re-enter the legacy command drain. Carve-out totals are unchanged because both messages were already counted in Phase 3.27. 1 schedule-level regression test removes `MenuStateRes`, sends both messages, and pins the empty `PendingCommands` buffer.

- [x] **Phase 3.34 / Navigation `ToLegacy` cleanup** — the fully carved-out navigation messages (`NextLevel`, `PrevLevel`, `GameResume`, `GameSetPracticeMode`) stopped implementing `ToLegacy`. At that historical point, `TurnRight` and `ToggleEditor` still kept legacy conversion impls; the active branch has since moved `ToggleEditor` and non-editor `TurnRight` to request resources, moved editor `TurnRight` to direct App-world editor mutation, and deleted all `ToLegacy` impls.

- [x] **Phase 3.35 / Surface resize carve-out (`ResizeSurface`)** — `events::auth::handle_resize_surface` now writes the latest physical size into App-world `RenderResizeRequest` instead of staging `AppCommand::ResizeSurface`. `state::render::plugin::apply_resize_surface_system` consumes that request inside `StateSchedule`, applies it to `RenderSurfaceHostRes` + `RenderGpuRes` when those resources are present, and records the accepted size for Runtime to mirror into legacy `State` render resources during the compatibility window. Runtime no longer consumes `RenderResizeRequest` directly. The `ResizeSurface` `ToLegacy` impl was removed. Schedule-level tests pin request write and bare-world request consumption, and a runtime test pins message -> scheduled request consumption -> legacy render size convergence through `run_app_frame`.

- [x] **Phase 3.36 / Perf frame copy carve-out (`CopyLatestPerfFrame`)** — `events::auth::handle_copy_latest_perf_frame` now writes an App-world `PerfFrameCopyRequest` instead of staging `AppCommand::CopyLatestPerfFrame`. `state::perf::process_perf_frame_copy_request_system` consumes that request inside `StateSchedule` and writes the formatted latest profiler frame to the system clipboard when frame data is available, so Runtime no longer has a dedicated perf-frame-copy request sync. The `CopyLatestPerfFrame` `ToLegacy` impl was removed. Schedule-level tests pin both the request write and request consumption, and a runtime test pins same-frame consumption through `run_app_frame`.

- [x] **Phase 3.37 / HistorySnapshot bridge cleanup** — `events::history::handle_history_snapshot` now drains marker messages without taking `PendingCommands`. Counts are unchanged because `HistorySnapshot` was already a no-op marker for future pre-mutation history work, but the handler no longer depends on the command bridge. 1 schedule-level regression test pins empty `PendingCommands`.

- [x] **Phase 3.38 / Broad `ToLegacy` cleanup for carved-out messages** — stale `ToLegacy` impls were removed from direct-resource messages across `input_state`, `pointer`, `snap`, `bpm`, `timing`, `spawn`, the carved-out `editor_mode`, `camera`, `misc`, `timeline`, and `session` variants. At that historical point, request-heavy messages still kept conversion impls; the active branch has since moved those messages to request resources and deleted `LegacyCommand`/`ToLegacy` entirely.
- [x] **Phase 3.39 / History Undo-Redo carve-out + object/history mirrors** — `events::history::handle_undo` and `handle_redo` now mutate App-world resources directly instead of staging `EditorCommand::Undo` / `Redo`. The new `EditorObjectsRes` mirrors the ordered authored-object list and carries a one-shot `history_restore_pending` flag; `EditorHistoryRes` is seeded from legacy history at `Runtime::new` and mirrored before `StateSchedule`. Runtime syncs restored object/history resources back into the compatibility `State` and runs the existing editor scene rebuild hooks while rendering remains legacy-owned. The `Undo`/`Redo` `ToLegacy` impls were removed. 1 schedule-level test pins undo/redo resource mutation + empty `PendingCommands`, and 1 state tick test pins the App-world path without legacy command staging.
- [x] **Phase 3.40 / Block command carve-out + clipboard/object mutation sync** — `events::block` handlers now mutate App-world resources directly instead of staging `EditorCommand::{RemoveBlock, DuplicateBlock, CopyBlock, PasteBlock, UpdateSelectedBlock}`. `EditorObjectsRes` carries a one-shot `object_mutation_pending` flag for ordinary object edits, and `EditorRuntimeRes` mirrors the editor clipboard so copy/paste stays App-world authoritative while the legacy renderer still rebuilds presentation. Runtime syncs App-world object selection, clipboard, history, and objects back into the compatibility `State`, then consumes the mutation flag. Phase 3.149 later deleted the clipboard/dirty-flag mirror and frame-end `EditorRuntimeRes` overwrite; ordered object presentation remains a separate compatibility bridge. 3 schedule-level tests pin remove, copy/duplicate/paste, and update-selected behavior with empty `PendingCommands`; 1 runtime test pins clipboard + object convergence through `Runtime::run_app_frame`.
- [x] **Phase 3.41 / Selection transform-trigger carve-out** — `events::selection::handle_add_transform_trigger` now mutates App-world resources directly instead of staging `EditorCommand::AddTransformTrigger`. The handler records editor history, clears active object drags, closes the App-world place window, removes same-time transform-trigger targets that are being replaced, appends generated transform trigger objects to `EditorObjectsRes`, selects the created trigger blocks, and marks `object_mutation_pending` so Runtime rebuilds the legacy presentation once. `FocusCameraTarget` remained legacy-routed after this slice because camera-track evaluation still needed an App-world owner; `SnapSelectionToGrid` moved in Phase 3.42 and `FocusCameraTarget` moved in Phase 3.44. 1 schedule-level test pins trigger creation, target replacement, history, selection, and empty `PendingCommands`; 1 runtime test pins message → App objects/session → legacy State convergence.
- [x] **Phase 3.42 / Selection snap carve-out** — `events::selection::handle_snap_selection_to_grid` now mutates App-world resources directly instead of staging `EditorCommand::SnapSelectionToGrid`. The block branch snaps selected `EditorObjectsRes` positions, updates the App cursor, records App-world editor history, and marks `object_mutation_pending` for Runtime presentation rebuild. The selected-tap branch retimes and reorders `EditorTimelineRes.0.taps` using the same timeline-near solver as legacy, moves the App cursor, and sets a one-frame tap cursor override so `Runtime::sync_app_editor_timeline_state` can call the legacy refresh path with the snapped indicator preserved. 2 schedule-level tests pin block/tap resource mutation with empty `PendingCommands`; 2 runtime tests pin block/tap convergence through `Runtime::run_app_frame`.
- [x] **Phase 3.43 / Camera trigger creation carve-out** — `events::camera::handle_add_camera_trigger` and `handle_add_camera_follow_trigger` now mutate App-world resources directly instead of staging `EditorCommand::{AddCameraTrigger, AddCameraFollowTrigger}`. Static camera triggers capture `EditorCameraRes` pose into generated `CAMERA_TRIGGER_BLOCK_ID` objects; follow triggers use the App-world timeline snapshot cache to derive the visible trigger block pose. Both handlers record App-world editor history, clamp trigger selection against `EditorObjectsRes`, mark `object_mutation_pending`, and leave `PendingCommands` empty. 3 schedule-level tests pin static/follow creation, history/object mutation, phase guards, and empty `PendingCommands`; 1 runtime test pins message -> App objects -> legacy State convergence and mutation-flag consumption.
- [x] **Phase 3.44 / Selection focus-camera carve-out** — `events::selection::handle_focus_camera_target` now mutates `EditorCameraRes` directly instead of staging `EditorCommand::FocusCameraTarget`. The camera-track evaluator was split into `evaluate_camera_track_for_objects(...)` so the App-world handler and legacy `State::editor_preview_camera_view` share the same static/follow trigger interpolation logic. The handler preserves legacy priority: selected tap in effective Tapping mode, selected block/transform-trigger bounds, selected tap outside Tapping, then camera-track preview fallback with yaw/pitch orientation. 3 schedule-level tests pin block focus, tap priority, camera-track fallback, and empty `PendingCommands`; 1 runtime test pins message -> App camera -> legacy State convergence.
- [x] **Phase 3.45 / Editor-mode pick-selected carve-out** — `events::editor_mode::handle_pick_selected_block` now mutates App-world resources directly instead of staging `EditorCommand::PickSelectedBlock`. The handler reads normalized selection from `EditorInputRes` + `EditorObjectsRes`, rejects non-placeable block ids, writes `EditorConfigRes.0.selected_block_id`, and switches the effective editor mode to Place; during editor timeline playback it preserves the legacy effective-mode contract by writing `EditorRuntimeRes.0.interaction.last_mode = Some(Place)` while `EditorInputRes.0.mode` stays `Null`. `PickBlockAt` remains legacy-routed because screen picking still depends on presentation-owned state. 2 schedule-level tests pin normal and playback-effective-mode behavior with empty `PendingCommands`; Runtime coverage now pins authoritative App config/input behavior after Phase 3.153.

- [x] **Phase 3.46 / Misc `Escape` request-pattern carve-out** — `events::misc::handle_escape` moved off `EditorCommand::Escape` and initially wrote `EscapeRequest.pending = true` for the full legacy `handle_editor_escape` state machine. The new `EscapeRequest` resource (a `pending: bool` flag) is registered alongside `PerfFrameCopyRequest` / `RenderResizeRequest` in `state_resources` and `EditorPlugin`; `Runtime::sync_app_escape_request` consumes the request during the typed request sync pass and calls the public `State::apply_editor_escape` wrapper. The `Escape` `ToLegacy` impl was removed and the old pending-command regression was replaced with request/no-op coverage. Phases 3.50 through 3.52 have since split out editor selection clearing, playback stop, timeline reset, and real-gameplay pause so only back-to-menu Escape branches remain request-backed. `PickBlockAt` has since moved to an App-world handler, so no editor-mode message remains legacy-routed.

- [x] **Phase 3.47 / Auth request-pattern carve-outs (`AuthSubmitSignIn`, `AuthSignOut`, `AuthOpenSignup`)** — the three `events::auth::*` handlers write `AuthSubmitSignInRequest.pending` / `AuthSignOutRequest.pending` / `AuthOpenSignupRequest.pending` instead of staging `AppCommand::{AuthSubmitSignIn, AuthSignOut, AuthOpenSignup}` in `PendingCommands`. Phase 6e moved consumption of those requests into `StateSchedule`: `process_auth_requests_system` mutates `AuthStateRes` and triggers the platform auth service through the App-world sender, while Runtime only mirrors `AuthStateRes` back to legacy `State` during the compatibility window. `State::prepare_app_world_for_state_schedule` no longer overwrites `AuthStateRes` from legacy `State`, so stale compatibility auth values cannot clobber the App-world resource before scheduled auth systems run. All three `ToLegacy` impls were removed. 3 schedule-level tests + 4 runtime tests cover request write, scheduled request consumption, no-op frame guards, and the App-resource authority regression.

- [x] **Phase 3.48 / Navigation request-pattern carve-outs (`GameQuitToMenu`, `GameRestartLevel`, `GameSetPracticeCheckpoint`, `GameRemovePracticeCheckpoint`)** — the four `events::navigation::*` handlers initially wrote typed request resources instead of staging legacy commands. Phase 3.55 deleted the removal request and moved checkpoint-list ownership into `SessionStateRes`; Phase 3.56 deleted the checkpoint-creation request as well. Quit and restart remain request-backed because their gameplay/audio lifecycle side effects are still legacy-owned. `ToggleEditor` and non-editor `TurnRight` have since moved to request handlers, while editor `TurnRight` is App-world direct, so navigation no longer stages legacy commands.

- [x] **Phase 3.49 / Navigation `TurnRight` editor-placement carve-out** — `events::navigation::handle_turn_right` now splits on `AppPhaseRes`: in `AppPhase::Editor`, it records `EditorHistoryRes`, creates the selected block at `EditorInputRes.cursor`, updates App-world selection/hover/mode exactly like the legacy placement branch, and marks `EditorObjectsRes.object_mutation_pending`; outside editor it still writes `TurnRightRequest.pending` because menu start, gameplay turn, and game-over return remained legacy-owned at this point. Later phases 3.53 and 3.103–3.109 moved the remaining branches direct and deleted the request/Runtime consumer. Schedule-level and runtime tests pin editor placement without a leftover request and menu fallback request behavior.

- [x] **Phase 3.50 / Misc `Escape` editor-selection carve-out** — `events::misc::handle_escape` now handles the first editor Escape branch directly: when `AppPhaseRes` is `Editor` and either block selection/hover or a valid selected tap exists, the handler clears `EditorInputRes` block selection, clears `EditorTimelineRes.0.taps.selected_index`, marks `EditorObjectsRes.object_mutation_pending` for the compatibility selection sync, and does not write `EscapeRequest`. Phase 3.51 has since moved editor playback stop and timeline reset direct, and Phase 3.52 has since moved real-gameplay pause direct too; only back-to-menu fallback branches still write `EscapeRequest.pending` because those branches own phase/menu/gameplay/audio side effects on legacy `State`. Schedule-level and runtime tests pin direct selection clearing, request fallback, and same-frame legacy selection convergence.
- [x] **Phase 3.51 / Misc `Escape` editor playback/reset carve-out** — `events::misc::handle_escape` now handles the remaining editor-local Escape branches directly after selection clearing: if editor timeline playback is active, it stops playback by mutating `EditorTimelineRes.0.playback`, clearing App-world playback seek/runtime state, and restoring `EditorInputRes.0.mode` through `EditorRuntimeRes.0.interaction.last_mode`; otherwise, if `EditorTimelineRes.0.clock.time_seconds > 0.001`, it resets the App-world timeline clock to `0.0`. `Runtime::sync_app_editor_timeline_state` mirrors those resource changes through the legacy `State::set_editor_timeline_playback_active` / `State::set_editor_timeline_time_seconds` compatibility setters while the audio/preview side effects still live there. Only real-gameplay pause, non-editor back-to-menu, and editor back-to-menu at time zero remained request-backed through `EscapeRequest` until Phase 3.52 split pause out too. Schedule-level and runtime tests pin playback stop, timeline reset, request-free handling, and same-frame legacy convergence.
- [x] **Phase 3.52 / Misc `Escape` real-gameplay pause carve-out** — `events::misc::handle_escape` now toggles `SessionStateRes.game_paused` directly when `AppPhaseRes` is `Playing` and `SessionStateRes.playtesting_editor` is false. `Runtime::sync_app_session_state` now handles both pause and resume directions through `State::pause_game_from_playing` / `State::resume_game_from_pause`, preserving the legacy audio and pending-input side effects as compatibility setters while making the App resource authoritative. `State::session_state_resource` and `State::prepare_app_world_for_state_schedule` now seed/mirror `SessionStateRes.playtesting_editor` alongside `game_paused` so playtest Escape is not misclassified as real gameplay. Only playtest/editor back-to-menu and phase fallback branches remain `EscapeRequest`-backed. Schedule-level and runtime tests pin real-gameplay pause/resume without a request and playtest Escape request fallback without toggling the real-game pause flag.
- [x] **Phase 3.53 / Navigation `TurnRight` game-over return carve-out** — `events::navigation::handle_turn_right` now handles `AppPhase::GameOver` directly by setting `AppPhaseRes` to `Menu` instead of writing `TurnRightRequest.pending`. This branch is intentionally phase-only, matching the legacy `State::turn_right` game-over arm. `Runtime::sync_app_phase_state` mirrors only this direct `GameOver -> Menu` carve-out back into the legacy compatibility shell through `State::set_phase_from_app`, avoiding a broad AppPhaseRes-to-State phase sync while full phase transition side effects still live in legacy methods. Later phases 3.103–3.109 moved menu and active gameplay branches direct and deleted the request/Runtime consumer. Schedule-level and runtime tests pin direct phase mutation, request-free handling, and same-frame legacy convergence.
- [x] **Phase 3.54 / Navigation `TurnRight` paused-gameplay no-op carve-out** — `events::navigation::handle_turn_right` now treats `AppPhase::Playing` with `SessionStateRes.game_paused` as a direct no-op rather than writing `TurnRightRequest.pending`. This exactly matches the first guard in the legacy `State::turn_right` playing arm, but keeps a paused input entirely in the App world. Later phases 3.103–3.109 moved menu and unpaused gameplay branches direct and deleted the request/Runtime consumer. Schedule-level and runtime tests pin phase/pause preservation and the absence of a request.
- [x] **Phase 3.55 / Navigation practice-checkpoint removal ownership** — `GameRemovePracticeCheckpoint` now removes the latest checkpoint directly from `SessionStateRes.practice_checkpoints` when real, unpaused practice gameplay is active. `GameRemovePracticeCheckpointRequest`, its State re-export, resource registration, Runtime consumer, and public compatibility wrapper are deleted. A one-shot `practice_checkpoints_dirty` marker invokes `State::set_practice_checkpoints_from_app` only to rebuild the legacy checkpoint flag mesh; legacy checkpoint creation refreshes the App list after its remaining request-backed side effect. Schedule-level and runtime tests pin direct removal, marker consumption, and App/legacy checkpoint-list convergence.
- [x] **Phase 3.56 / App-world gameplay authority bootstrap** — Runtime seeds `eg-game` player/object entities and gameplay resources once from the legacy shell, then `run_state_schedule_in_world` runs the real `GameplaySchedule` for active unpaused play. `State::update` skips the duplicate legacy simulation when Runtime has enabled App gameplay authority; `GameState::sync_from_external_world` copies only compatibility fields back for rendering. Lifecycle boundaries mark one-shot re-seeding when legacy restart/playtest/phase code still runs. `GameSetPracticeCheckpoint` now captures player, clock, progress, and ordered object ECS state directly into `SessionStateRes`, so its request resource, Runtime consumer, and public legacy wrapper are deleted. Focused schedule and Runtime tests pin direct capture, App-world motion, and compatibility convergence.

- [x] **Phase 3.57 / FramePipeline deletion and App-owned presentation resources** — `EguiFrameResources` now owns the egui context, `EguiRenderer`, menu textures, and `BlockIconCache` as a non-send App resource. `Runtime::run_app_frame` inlines UI execution, command ingress, App/State scheduling, tessellation, texture upload, compatibility update, and present ordering; `platform/pipeline.rs` and `FramePipeline` are deleted. The remaining deletion gate is moving `State::update` and `State::render_egui` into scheduled App presentation systems. The runtime output regression still passes after the move.

- [x] **Phase 3.58 / Active gameplay restart ownership** — real-gameplay `GameRestartLevel` now records an App-world `GameplayResetRes` marker and an exclusive gameplay-world system respawns the authored object/player entities, resets `GameClock`, progress, pending turns, death state, and practice-session flags, and rebuilds `SpatialIndex`. Menu and editor-playtest branches retain `GameRestartLevelRequest` until their audio/phase side effects move. Schedule-level and runtime tests pin both the direct reset/no-request path and the compatibility fallback path.

- [x] **Phase 3.59 / Active gameplay quit-to-menu ownership** — real-gameplay `GameQuitToMenu` now mutates `AppPhaseRes`, `SessionStateRes`, and `MenuStateRes` directly, clears practice/lifecycle state, and records a one-shot App audio-stop intent. Runtime mirrors the App phase into the compatibility shell, consumes only that external-audio intent, and seeds the post-update menu preview boundary. Editor/playtest quit branches retain `GameQuitToMenuRequest` until their transition/audio ownership moves. Schedule-level and fresh-target runtime tests pin both direct and fallback paths.

- [x] **Phase 3.60 / Editor Escape-to-menu ownership** — editor Escape at the timeline origin now mutates `AppPhaseRes`, `SessionStateRes`, and `MenuStateRes` directly, with Runtime consuming the one-shot audio-stop intent; the existing editor selection/playback and playtest Escape branches remain local or request-backed until their remaining legacy side effects move. Schedule-level and fresh-target runtime tests pin the direct transition and fallback behavior.

- [x] **Phase 3.61 / Editor ToggleEditor-to-menu ownership** — `ToggleEditor` now returns a real editor session to App-world menu/session resources directly, while bare worlds and playtest/editor-entry paths retain the typed request fallback. Schedule-level and fresh-target runtime tests pin the direct transition without a `ToggleEditorRequest`.

- [x] **Phase 3.62 / Editor camera pan schedule ownership** — held editor camera keys now advance `EditorCameraRes` from `EditorInputRes` in `StateSchedule`. Runtime syncs that result into the render compatibility shell and marks the legacy update path to skip its duplicate pan for the frame. Schedule-level and fresh-target Runtime tests pin App ownership and exactly-once movement.

- [x] **Phase 3.63 / App-owned UI view storage** — `UiStateRes` now owns the data consumed by the egui helpers. `UiStateRes::from_world` populates the model directly from App-world phase, session, menu, auth, perf, editor, timeline, camera, and selection resources; Runtime refreshes it before presentation, the editor view path returns input-blocking rectangles instead of mutating `State`, and Runtime writes those rectangles directly to `EditorInputRes`. Phase 3.158 removed the stale snapshot module naming; direct resource/query reads remain the final UI gate.

- [x] **Phase 3.64 / Editor camera transition schedule ownership** — `EditorCameraRes` transitions now advance in `StateSchedule` beside key panning, and Runtime marks the compatibility update to skip both duplicate camera operations. Existing camera resource tests, the Runtime exactly-once transition regression, and the full eg-state suite cover the transition path.

- [x] **Phase 3.65 / Scheduled color-space uniform upload** — `upload_color_space_uniform_system` now writes the App-owned `RenderUniformsRes.color_space` in the render-present set. Runtime records that scheduled upload before the compatibility update, so `State::update` retains a direct-call fallback but no longer duplicates the production GPU write.

- [x] **Phase 3.66 / Direct UI message sink** — egui helpers now accept `UiCommandSink`; production Runtime frames use `WorldUiCommandSink` to write Bevy messages directly. The vector-returning helper is retained solely as a focused test adapter.

- [x] **Phase 3.67 / UI queue deletion** — `UiCommandQueue`, enqueue/drain APIs, and the Update drain system are deleted. Runtime and tests write typed messages immediately, leaving no second UI command buffer between egui and `StateSchedule`.

- [x] **Phase 3.68 / App-owned render configuration and uniform uploads** — Runtime seeds `RenderConfigRes` and production-only `RenderUniformUploadRes` from the compatibility GPU context. `RenderExtractSchedule` now computes uniforms without requiring the full GPU resource and uploads through cloned wgpu handles owned by the App world.

- [x] **Phase 3.69 / Render mirror deletion and fast extraction tests** — the render thread-local mirror, compatibility snapshot helper, and mirror schedule system are deleted. `TestApp::tick_state` runs the production schedule sequence, and focused App-world fixtures validate frame-input and uniform extraction without constructing the full UI/GPU compatibility stack or loading built-in levels.

- [x] **Phase 3.70 / Editor camera update fallback deletion** — `update_editor_camera_pan_system` is the sole per-frame owner of key panning and camera transitions. `EditorFrameRes`, Runtime's scheduled-work handoff, the two `State` booleans, and the legacy `State::update` fallback methods are deleted; App-world and Runtime tests gate direct scheduling and exactly-once behavior.

- [x] **Phase 3.71 / Render-uniform fallback deletion** — `RenderExtractSchedule` is the sole owner of camera, line, and color-space uniform computation and GPU uploads. `State::update` no longer writes those buffers; the compatibility `PlayerRenderState.line_uniform`, scheduled-upload flags, Runtime-to-State uniform sync, and duplicate menu/editor/playing camera builders are deleted. Focused `TestApp` extraction tests cover all phases, viewport/gamma, and playing line values.

- [x] **Phase 3.72 / Legacy gameplay simulation deletion** — `eg_game::GameplaySchedule` now runs for every active, unpaused App-world `Playing` phase without an authority marker. `GameplayWorldRes`, `State.gameplay_app_authoritative`, Runtime's conditional sync, and the legacy fixed-step/audio-target/queued-turn simulation helpers are deleted. Runtime tests gate App-owned motion and turn consumption; `State::update` temporarily retains only transform-trigger projection beside the still-legacy mesh builder.

- [x] **Phase 3.73 / Legacy frame-clock fallback deletion** — `advance_frame_time_system` is the sole frame-clock owner and `reset_menu_editor_frame_accumulator_system` owns non-playing accumulator reset. `EditorFrameState.scheduled_delta_available`, `State::frame_delta_for_legacy_update`, the fallback profiler/clock advance, and duplicate menu/editor resets in `State::update` are deleted; the compatibility shell receives only the current App delta needed by remaining editor/effect code.

- [x] **Phase 3.74 / Performance overlay mirror deletion** — `handle_toggle_perf_overlay` now owns both `PerfRes.0.overlay_enabled` and the `puffin::set_scopes_on` side effect. Runtime no longer seeds perf state from `State`, mirrors the toggle back into `State`, or carries bridge-only integration tests; legacy perf accessors remain only for the borrowed `UiStateSnapshot` compatibility path.

- [x] **Phase 3.75 / Auth result mirror deletion** — auth request handling and async results remain authoritative in `AuthStateRes`; Runtime no longer copies session, pending, message, refresh, or navigation fields back into legacy `State`. The constructor still seeds the resource and shares the auth channel until `State` construction is replaced by the App builder.

- [x] **Phase 3.76 / Legacy State UI snapshot producer deletion** — production UI state is derived exclusively through `UiStateRes::from_world`; `State::ui_state_snapshot()` is deleted. UI rendering tests now use explicit view fixtures, and the remaining borrowed adapter is `UiStateView`, returned by App-owned `UiStateRes::view()` until UI helpers read resources directly.

- [x] **Phase 3.77 / Legacy State perf subsystem deletion** — `EditorSubsystem` no longer stores `PerfState`, and frame-time sync no longer copies App-owned FPS into `State`. State-only overlay/backend/frame-view accessors and their compatibility test are deleted; `PerfRes` now exclusively owns overlay state, FPS smoothing, and puffin frame data.

- [x] **Phase 3.78 / Auth view logic ownership** — display-name derivation now belongs to `AuthStateRes` and `UiStateRes::from_world` reads it there. The obsolete `State` display-name, pending, and message accessors are deleted, and the focused auth test runs against the App resource without constructing GPU-backed State.

- [x] **Phase 3.79 / Auth subsystem state transfer** — `AuthSubsystem` now holds the initial `AuthStateRes` plus its receiver, and `Runtime::new` takes that resource into the App world instead of cloning a duplicate auth state into both owners. Test-only State auth request/result implementations are deleted; scheduled `AuthStateRes` systems are the sole auth behavior path.

- [x] **Phase 3.80 / Test-only pre-schedule mirror deletion** — channel and schedule tests now drive `TestApp` directly. `State::prepare_app_world_for_state_schedule`, its object/history/timeline/clipboard copies, and the `tick_state_with_app` compatibility harness are deleted; production preparation retains only still-required platform/session bridges.

- [x] **Phase 3.81 / One-time async channel installation** — Runtime installs the five channel resources once during construction. `prepare_app_world_for_runtime` no longer repeats the idempotent State-to-World channel wiring every frame; `StateSchedule` polls the installed resources directly.

- [x] **Phase 3.82 / App-authoritative settings preparation** — `SessionStateRes` is seeded once and remains authoritative for UI scale, keybinds, graphics backend selection, restart-required state, and backend locking. Runtime no longer restores those fields from legacy `State` before or after schedules, and presentation reads UI scale directly from the App world.

- [x] **Phase 3.83 / Dead fixed-step accumulator deletion** — after `GameplaySchedule` became the sole gameplay simulator, the manual frame accumulator had no remaining consumer. `FrameTimeRes.accumulator`, the legacy `EditorFrameState` mirror, the phase-specific reset system, and the two Playing-branch resets in `State::update` are deleted.

- [x] **Phase 3.84 / Scheduled settings persistence** — graphics backend, UI scale, and keybind handlers now persist changed `SessionStateRes.app_settings` directly and retain no-op guards. Runtime no longer mirrors those fields into legacy `State` for persistence; post-construction settings ownership is one-way and App-authoritative.

- [x] **Phase 3.85 / Runtime preparation hook deletion** — Runtime construction seeds session state once, and the post-legacy lifecycle refresh captures pause/playtest changes made by remaining compatibility side effects. The redundant pre-schedule pause/playtest mirror, `State::prepare_app_world_for_runtime`, its Runtime call, and its bridge-only test are deleted; App pause state now wins over stale legacy values.

- [x] **Phase 3.86 / App-owned waveform result presentation** — `waveform_handler_system` now applies current-level waveform samples, sample rate, window size, loading state, and completion directly to `EditorTimingRes`, which is the source read by UI. Stale-level results cannot overwrite current presentation; the temporary closure is reduced to legacy audio-cache and persistent-cache maintenance.

- [x] **Phase 3.87 / App-owned audio-import loading state** — accepted `AudioImportResultEvent`s now clear and mark the App-owned waveform presentation as loading before the compatibility closure inserts imported bytes and starts decoding. Cancellation remains a no-op, and UI no longer waits on a legacy timing mutation it cannot observe.

- [x] **Phase 3.88 / Loaded-level waveform lifecycle refresh** — the one-shot Runtime refresh after a remaining legacy level-load side effect now replaces `EditorTimingRes` from the newly loaded editor state. Timing-mode loads therefore clear or restore waveform presentation immediately, before scheduled waveform results continue direct App updates.

- [x] **Phase 3.89 / App-owned async level import** — `level_import_handler_system` now decodes `.egz` and binary payloads inside `StateSchedule` and replaces App-owned editor objects, input/selection, timeline, timing, spawn, camera, trigger selection, history, and session metadata directly. Binary imports no longer install a thread-local `State` closure; `.egz` imports retain a cache-only closure only when bundled audio bytes are present.

- [x] **Phase 3.90 / App-owned level picker launch** — `CompleteImport` now invokes the platform picker from `StateSchedule` through `LevelImportSenderRes`, whose receiver is already App-owned by the async channel bridge. `CompleteImportRequest`, Runtime request consumption, and `State::complete_import` are deleted.

- [x] **Phase 3.91 / App-owned audio picker launch** — `TriggerAudioImport` now invokes the platform picker from `StateSchedule` through `AudioImportSenderRes`, sharing the channel already polled by the App async bridge. `TriggerAudioImportRequest`, Runtime request consumption, and `State::trigger_audio_import` are deleted.

- [x] **Phase 3.92 / Shared audio-cache ownership and async closure deletion** — `EditorAudioCacheRes`, `RuntimeAudioPreloadRes`, and `WaveformLoadSenderRes` expose the authoritative caches and waveform work sender to App systems while remaining legacy playback reads the same handles. Audio import, waveform, preload, and bundled level-audio results now apply directly in `StateSchedule`. The thread-local async-result closure queue, Runtime drain, duplicate `State::update_*` channel consumers, and their compatibility tests are deleted.

- [x] **Phase 3.93 / App-owned built-in level loading** — `BuiltinLevelCatalogRes` owns the metadata catalog used by `LoadLevel`. The scheduled handler now replaces editor objects, selection, spawn, timeline, timing, camera, history, and session metadata directly, clears playback, records the existing audio-stop intent, and restores or launches timing waveform loading from shared caches. `LoadLevelRequest`, Runtime consumption, the legacy session refresh helper, and its Runtime test are deleted.

- [x] **Phase 3.94 / App-owned level export** — `ExportLevel` now builds metadata and bundles shared editor audio directly from App resources, then invokes `LevelExportServiceRes`. Production launches the platform save task while tests capture and decode the `.egz` request without filesystem I/O. `ExportLevelRequest` and Runtime consumption are deleted, and load-before-export ordering is explicit for same-frame messages.

- [x] **Phase 3.95 / App-owned audio backend selection** — `AudioBackendRes` is the authoritative platform backend handle installed in the App world and temporarily shared with remaining compatibility playback callers. `SetAudioBackend` validates and applies the host selection directly, updates and persists `SessionStateRes.app_settings`, and honors the App-owned lock. Winit device initialization now reads the App resource. `SetAudioBackendRequest`, Runtime consumption, `State::ensure_device`, and `State::set_preferred_audio_backend` are deleted.

- [x] **Phase 3.96 / App-owned gameplay quit-to-menu** — `GameQuitToMenu` now transitions active real gameplay directly through `AppPhaseRes` and `SessionStateRes`, with optional menu preview cleanup no longer gating the transition. Other phases and editor playtests are explicit no-ops. `GameQuitToMenuRequest`, Runtime consumption, and `State::apply_game_quit_to_menu` are deleted.

- [x] **Phase 3.97 / App-owned gameplay restart** — `GameRestartLevel` now requests the required `GameplayResetRes` only during active real gameplay; other phases and editor playtests are explicit no-ops. The scheduled reset remains the atomic owner of gameplay entity/resource replacement. `GameRestartLevelRequest`, Runtime consumption, `State::apply_game_restart_level`, and the orphaned pause adapter are deleted.

- [x] **Phase 3.98 / App-owned playtest exit through ToggleEditor** — `ToggleEditor` now returns from an editor playtest by changing `AppPhaseRes` back to Editor, clearing the App-owned playtest/pause flags, and recording the shared audio-stop intent. Real gameplay and game-over are direct no-ops. Phase 3.102 subsequently moved menu-to-editor entry into the App world too.

- [x] **Phase 3.99 / App-owned playtest exit through Escape** — playtest `Escape` now returns directly to Editor, clears the App-owned playtest/pause and timeline playback state, and records the shared audio-stop intent. The Runtime session sync now mirrors the App-owned playtest flag into the compatibility shell before the legacy refresh, preventing stale state from reclaiming authority. Phase 3.101 subsequently removed the remaining fallback request.

- [x] **Phase 3.100 / App-owned playtest entry** — `Playtest` now derives its transition from App-owned editor resources, updates phase/session/camera state directly, and seeds the shared atomic gameplay reset for the ECS world. `PlaytestRequest`, Runtime consumption, and the production `State::editor_playtest` bridge are deleted.

- [x] **Phase 3.101 / App-owned Escape fallback removal** — menu and game-over Escape branches now resolve directly in `StateSchedule`, while editor selection, playback, timeline reset, real-gameplay pause, and playtest exit remain direct App-world branches. `EscapeRequest`, its Runtime consumer, and the legacy Escape compatibility bridge are deleted. Focused schedule and Runtime tests cover all seven Escape contexts.

- [x] **Phase 3.102 / App-owned ToggleEditor menu entry** — menu `ToggleEditor` now selects from `MenuStateRes`, loads metadata through `BuiltinLevelCatalogRes`, resets App-owned editor/session resources, warms the shared `AudioBackendRes`, and schedules shared runtime-audio preload. `ToggleEditorRequest` and its Runtime consumer are deleted. Focused schedule and Runtime tests cover menu entry, editor return, and playtest exit.
- [x] **Phase 3.103 / App-owned TurnRight menu start** — menu `TurnRight` now selects the active level from `MenuStateRes`, loads its metadata through `BuiltinLevelCatalogRes`, resets App-owned session/editor/gameplay resources, warms the shared audio backend, queues runtime-audio preload, and seeds the atomic ECS gameplay reset. Focused schedule and Runtime tests cover menu start without a leftover request.
- [x] **Phase 3.104 / App-owned TurnRight gameplay startup** — unstarted `Playing` TurnRight now marks the App-owned `GameClock` started and starts editor-cache, preloaded-runtime, or built-in audio directly through `AudioBackendRes`, preserving the legacy playtest offset rules. Focused schedule and Runtime tests cover startup without Runtime request consumption.
- [x] **Phase 3.105 / App-owned TurnRight airborne input** — active gameplay TurnRight now queues the App-owned `PendingTurnInputs` tap regardless of grounded state, matching the legacy airborne input behavior. Schedule coverage pins both grounded and airborne App-world taps.
- [x] **Phase 3.106 / App-owned TurnRight non-practice game-over reset** — active non-practice gameplay TurnRight now requests the existing App-owned `GameplayResetRes` when the ECS clock is in `game_over`, matching the ordinary restart path without invoking `State::restart_level`. Focused schedule and Runtime tests cover the direct reset.
- [x] **Phase 3.107 / App-owned TurnRight practice respawn** — practice-mode game-over TurnRight now restores the latest App-owned checkpoint through the atomic gameplay reset, preserving checkpoint progress, practice mode, checkpoint history, trigger-base objects, and the reset timestamp while stopping shared audio and clearing pending turns. Focused schedule and Runtime tests cover checkpoint respawn without Runtime request consumption.
- [x] **Phase 3.108 / App-owned TurnRight empty-practice restart** — practice-mode game-over TurnRight without a checkpoint now reuses the atomic App reset from current `LevelMeta`, preserves practice mode, clears the empty checkpoint list, and stops shared audio. Focused schedule and Runtime tests cover the no-checkpoint restart.
- [x] **Phase 3.109 / TurnRight request bridge deletion** — all TurnRight phase and gameplay branches now resolve in App-world handlers; level-complete input is a direct no-op and active gameplay queues `PendingTurnInputs` without requiring a legacy player query. `TurnRightRequest`, its resource registration, Runtime consumer, and stale request assertions are deleted. Focused schedule and Runtime tests cover the complete direct path.
- [x] **Phase 3.110 / App-owned gameplay completion transition** — after `GameplaySchedule` consumes the completion hold timer, `apply_completed_gameplay_transition_system` records real-level progress into App-owned settings, persists changed settings, transitions real gameplay to App-owned menu state or editor playtests back to App-owned editor state, marks shared audio stop, clears preview/playback session data, and resets the App gameplay world before Runtime mirrors compatibility state. The completion branch is removed from production `State::update`; focused Runtime coverage pins the real-gameplay menu transition.
- [x] **Phase 3.111 / App-owned non-practice GameOver side effects** — `apply_game_over_transition_system` now consumes and clears `DeathSfxFlag`, plays the death SFX through `AudioBackendRes`, persists non-practice progress, stops shared audio, and clears queued turns directly from the App world. `State::update` retains only the temporary practice-mode checkpoint respawn compatibility path; the deleted legacy death-audio test is replaced by focused Runtime coverage.
- [x] **Phase 3.112 / App-owned practice GameOver respawn** — automatic practice-mode GameOver now reuses the App-owned atomic reset for checkpoint restore and empty-practice restart, including checkpoint progress, trigger-base objects, audio warmup, and pending-turn cleanup. The final GameOver mutation branch is removed from `State::update`; Runtime coverage continues to pin the reset behavior.
- [x] **Phase 3.113 / App-owned editor timeline playback** — editor playback clock advancement, timeline simulation, seek debounce/resync, metronome SFX, audio start/stop, completion, and consumed-block effect production now run in `StateSchedule` from App-owned resources. Runtime mirrors the App preview/runtime state and forwards dirty flags/effects to the compatibility presenter; the per-frame playback mutation block is removed from `State::update`. Focused runtime and schedule tests cover playback propagation and handler ownership.
- [x] **Phase 3.114 / App-owned menu preview selection** — `MenuPreviewObjectsRes` and `refresh_menu_preview_system` now make the selected menu level, preview camera, sky color, and preview object list an App-world result of the scheduled menu state. The per-frame `refresh_menu_level_preview_if_needed` call is removed from `State::update`; Runtime performs one guarded compatibility mirror and rebuilds the legacy menu mesh only when the scheduled preview changes. The legacy refresh helper is test-only, and menu transition code now invalidates the preview for the App schedule to repopulate.
- [x] **Phase 3.115 / App-owned block-effect lifecycle** — `BlockEffectsRes` and `update_block_effects_system` now consume playback effect output, age active effects, and expire completed effects in `StateSchedule`. Runtime mirrors the active list for compatibility vertex upload; `State::update` no longer mutates effect age or lifetime and only builds the remaining legacy effect mesh.
- [x] **Phase 3.116 / App-owned gameplay trail extraction** — `RenderTrailVerticesRes` and the gameplay branch of `extract_trail_vertices_system` now build gameplay trail vertices after the App-owned gameplay tick from `PlayerPosition`, `PlayerDirection`, `Grounded`, `Trail`, and `GameClock`. Runtime uploads that extracted list for the compatibility renderer; the gameplay trail builder is removed from `State::update`.
- [x] **Phase 3.117 / App-owned editor timeline cache and trail extraction** — `rebuild_editor_timeline_snapshot_cache_system` now owns the editor timeline snapshot/trail cache rebuild in `StateSchedule`, and `extract_trail_vertices_system` builds playback and scrub trail vertices from App-owned timeline resources during `RenderExtractSchedule`. Runtime uploads the shared extracted vertex list for compatibility rendering; production `State::update` no longer builds or clears editor trails, leaving only test-only legacy helpers and the remaining editor mesh/upload paths.
- [x] **Phase 3.118 / App-owned editor camera-arrow extraction** — `extract_editor_camera_arrow_vertices_system` now derives the editor preview camera arrow from `EditorCameraRes`, `EditorObjectsRes`, and `EditorTimelineRes` during `RenderExtractSchedule`. Runtime uploads that result into the compatibility mesh slot; the per-frame `State::update` camera-arrow rebuild is deleted.
- [x] **Phase 3.119 / App-owned editor tap-indicator extraction** — `extract_editor_tap_indicator_vertices_system` now derives saved taps, timing-division previews, and timing-division markers from App-owned editor resources during `RenderExtractSchedule`. Runtime uploads that result into the compatibility mesh slot; the production dirty path no longer rebuilds tap indicators from `State::update`.
- [x] **Phase 3.120 / App-owned editor selection and hover overlays** — `extract_editor_selection_overlays_system` now derives selection/hover stencil geometry, outlines, and selection instances from App-owned editor objects/input during `RenderExtractSchedule`. Runtime uploads those buffers into the compatibility renderer; the production dirty path no longer rebuilds selection or hover outlines from `State::update`.
- [x] **Phase 3.121 / App-owned transform-trigger markers** — `extract_editor_transform_trigger_marker_vertices_system` now derives transform-trigger source/target marker geometry from App-owned objects, trigger selection, and timeline time during `RenderExtractSchedule`. Runtime uploads that result into the compatibility mesh slot; the production dirty path no longer rebuilds transform-trigger markers from `State::update`.
- [x] **Phase 3.122 / App-owned block-effect extraction** — `extract_block_effect_vertices_system` now converts the App-owned `BlockEffectsRes` lifecycle into effect vertices during `RenderExtractSchedule`. Runtime uploads the result directly into the compatibility mesh slot; the legacy effect-list mirror and `State::update` effect mesh builder are deleted.
- [x] **Phase 3.123 / App-owned playing block extraction** — `extract_playing_block_meshes_system` now derives time-dependent gameplay block batches or static chunk meshes from App-owned gameplay entities, the App clock, and editor triggers during `RenderExtractSchedule`. Runtime uploads the CPU result into the compatibility block slots; the non-editor block rebuild branch is deleted from production `State::update`.
- [x] **Phase 3.124 / App-owned editor block split extraction** — `extract_editor_block_meshes_system` now derives the editor static chunk mesh and selected expanded geometry from App-owned editor objects, selection, playback runtime, and camera-trigger time. Runtime uploads both buffers into the compatibility renderer; production dirty processing no longer rebuilds editor block meshes in `State::update`.
- [x] **Phase 3.125 / App-owned editor spawn-marker extraction** — `extract_editor_spawn_marker_vertices_system` now derives the editor spawn marker from `EditorSpawnRes` during `RenderExtractSchedule`. Runtime uploads the result into the compatibility mesh slot so scheduled spawn edits no longer depend on legacy marker rebuild side effects.
- [x] **Phase 3.126 / App-owned editor gizmo extraction** — `extract_editor_gizmo_vertices_system` now derives transform-gizmo geometry from App-owned editor selection, trigger, interaction, camera-uniform, and viewport resources during `RenderExtractSchedule`. Runtime uploads the result into the compatibility mesh slot; production `State::update` and dirty processing no longer rebuild the gizmo mesh.
- [x] **Phase 3.127 / App-owned editor dirty lifecycle** — render extraction now consumes editor dirty flags after rebuilding App-owned presentation resources. The production `State::update` dirty-processing and gizmo-throttling paths, plus their no-op preview/camera marker helpers, are test-only compatibility code; production `State::update` retains only the remaining editor drag interaction.
- [x] **Phase 3.128 / Production `State::update` deletion** — all remaining `State::update` behavior was already test-only after App-owned extraction and scheduled lifecycle systems landed. Runtime no longer calls it; the method is retained only under `cfg(test)` for legacy unit coverage while editor pointer interaction is migrated into App-world handlers.
- [x] **Phase 3.129 / App-owned pointer interaction** — pointer messages now perform placement, transform selection, marquee selection, block/gizmo dragging, cursor updates, and drag release directly against App resources. Mouse press actions write typed `TurnRight`/`PrimaryClick` messages with explicit schedule ordering; Runtime no longer needs `State::update` for pointer mutation.
- [x] **Phase 3.130 / App-owned tapping pointer interaction** — Tapping clicks now use App-owned timeline, timing, camera, object, and history resources for tap selection, two-click timing-division placement, cursor movement, pending click state, and undo history. The pointer handler no longer drops Tapping clicks while the legacy helper remains test-only.
- [x] **Phase 3.131 / Legacy render wrapper narrowing** — standalone `State::render_egui`, `State::render`, `State::render_with_overlay`, and `State::render_to_view` entry points are now test-only because Runtime only uses the frame-input egui compatibility path. The next render slice is isolated to deleting `State::render_egui_with_frame_input` after App-owned surface/mesh presentation is wired.
- [x] **Phase 3.132 / Production editor mirror deletion** — Runtime no longer copies App-owned editor input, config, timeline, runtime/clipboard, spawn, camera, trigger, object, or menu-selection resources into legacy `State` during production frames, and the post-present refresh no longer overwrites those App resources from legacy fields. The old editor/menu sync helpers remain test-only for existing compatibility tests; the narrow App-phase bridge remains only for legacy pause/practice/audio side effects.
- [x] **Phase 3.133 / App-owned menu preview extraction** — Menu preview objects now build a cached `RenderMenuPreviewBlockMeshesRes` during `RenderExtractSchedule`. Runtime only uploads that extracted chunk mesh into the compatibility scene slot; it no longer asks legacy `State` to copy menu metadata, replace gameplay objects, rebuild behavior caches, or rebuild block vertices.
- [x] **Phase 3.134 / App-owned render resource transfer** — Production `Runtime::new` moves `GpuContext`, `SceneMeshes`, and `SurfaceHost` out of `State` into `RenderGpuRes`, `RenderMeshesRes`, and `RenderSurfaceHostRes`. `State` keeps an empty compatibility slot only for test-only legacy render callers.
- [x] **Phase 3.135 / Scheduled scene and egui presentation** — `RenderEguiRes` owns the renderer and current paint jobs; `RenderExtractSchedule` uploads extracted meshes and runs `present_render_frame_system`, which consumes App-owned GPU, mesh, surface, uniform, frame-input, and egui resources. Runtime no longer calls a State render method or mirrors extracted mesh buffers.
- [x] **Phase 3.136 / UI view adapter rename** — `UiStateRes::view()` now returns the named `UiStateView`, and every eg-ui helper/test fixture uses that type.
- [x] **Phase 3.137 / App-owned UI execution** — Runtime now queues raw egui input and runs egui UI, command emission, tessellation, texture upload, and UI input-blocking capture inside App Update. Render presentation consumes the resulting App-owned paint jobs and renderer state; Runtime only handles platform output and texture frees.
- [x] **Phase 3.138 / App-owned native window access** — `Runtime::window()` now reads the native window from App-owned `RenderSurfaceHostRes`; the winit event and redraw paths no longer dereference the emptied State render shell after resource transfer.
- [x] **Phase 3.139 / App-owned gameplay audio pause bridge** — `StateSchedule` now consumes `SessionStateRes.audio_stop_pending` through `AudioBackendRes`, pauses/resumes real gameplay audio when the App pause flag changes, and clears queued turn inputs; Runtime no longer invokes State for those production side effects.
- [x] **Phase 3.140 / App-owned practice checkpoint presentation and restart** — `RenderExtractSchedule` now derives practice checkpoint flag vertices from `SessionStateRes.practice_checkpoints` and uploads them into the App-owned scene mesh storage. Disabling real gameplay practice mode queues `GameplayResetRes` directly, preserving the legacy restart behavior without invoking State; the obsolete practice checkpoint dirty marker and Runtime copy bridge are deleted; the remaining legacy practice accessor is test-only.
- [x] **Phase 3.141 / Production State ownership deletion** — production `Runtime` no longer stores the legacy `State` or runs any State synchronization during frames. Editor UI hit testing reads `EditorInputRes` directly, the complete session state is seeded once into `SessionStateRes`, and broad State mirrors remain compiled only for compatibility tests.
- [x] **Phase 3.142 / Direct App-world production bootstrap** — `StateAppBootstrap` now consumes the shared native/WASM GPU and platform initializer and installs render, async, audio, auth, gameplay, menu, editor, session, and frame-time resources directly into the App world. Production application entry points call `Runtime::from_bootstrap` and never construct `State`; the legacy State consumer remains only for GPU-backed compatibility tests. The dead Runtime render-transfer branch and `State::take_render_resources` are deleted.
- [x] **Phase 3.143 / Test phase and menu mirror deletion** — Runtime compatibility tests now assert phase and menu selection against authoritative `AppPhaseRes` and `MenuStateRes`. The per-frame `sync_app_phase_state` / `sync_app_menu_state` calls and methods plus `State::set_phase_from_app` are deleted, shrinking the test-only mirror layer without changing production behavior.
- [x] **Phase 3.144 / Test frame-time mirror deletion** — Runtime compatibility coverage now asserts scheduled frame timing directly from `FrameTimeRes`. The per-frame `sync_app_frame_time_state` bridge and `State::apply_frame_time_resource` are deleted.
- [x] **Phase 3.145 / Test gameplay mirror deletion** — Runtime compatibility coverage now asserts player motion and restart behavior directly from App-world gameplay entities and resources. The per-frame `sync_app_gameplay_state` bridge, `State::sync_gameplay_from_app_world`, and the obsolete legacy position accessor are deleted.
- [x] **Phase 3.146 / Test trigger-selection mirror deletion** — Runtime compatibility coverage now writes and asserts `EditorTriggerRes` directly. The per-frame `sync_app_editor_trigger_state` bridge and the legacy trigger-selection getter/setter are deleted; camera event documentation now records App-world ownership.
- [x] **Phase 3.147 / Test spawn mirror deletion** — Runtime compatibility coverage now drives and asserts `EditorSpawnRes` directly, while one constructor fixture uses real legacy editor operations to verify one-time seeding. The per-frame `sync_app_editor_spawn_state` bridge and mirror-only spawn getters/setter are deleted.
- [x] **Phase 3.148 / Test camera mirror deletion** — Runtime camera coverage now drives and asserts `EditorCameraRes` directly for zoom, orientation, held-key pan, transitions, and no-op frames. The per-frame `sync_app_editor_camera_state` bridge and mirror-only camera getters are deleted; the remaining legacy setter is test-only construction support.
- [x] **Phase 3.149 / Test editor-runtime mirror deletion** — `EditorRuntimeRes` now preserves clipboard and dirty-flag state directly across Runtime test frames. The per-frame `sync_app_editor_runtime_state` copy, reverse frame-end resource overwrite, and mirror-only State clipboard/dirty methods are deleted; ordered object presentation remains isolated in its own bridge.
- [x] **Phase 3.150 / Test editor-timing mirror deletion** — Runtime BPM, timing-point, selection, playback-speed, and waveform coverage now reads authoritative `EditorTimingRes` fields directly. The per-frame `sync_app_editor_timing_state` bridge is deleted; legacy timing setters remain only for state-level tests and one-time compatibility fixtures.
- [x] **Phase 3.151 / Ordered editor-object bridge deletion** — `EditorObjectsRes.objects` is the authoritative ordered object list for commands, history, selection, import, and render extraction. The bridge-only `history_restore_pending` / `object_mutation_pending` flags, all producers/tests, Runtime State scene rebuild, and frame-end reverse object refresh are deleted.
- [x] **Phase 3.152 / Test editor-input mirror deletion** — `EditorInputRes` is authoritative for modifiers, held pan controls, hitbox visibility, editor mode, cursor, hover, and selection. Runtime compatibility tests now assert App resources directly; `sync_app_editor_input_state`, the frame-end `State::editor_input_resource()` overwrite, bridge-only hitbox State APIs, and stale legacy-convergence assertions are deleted. Timeline compatibility writes its derived selected-tap cursor directly to `EditorInputRes`, so no reverse input refresh remains.
- [x] **Phase 3.153 / Editor-config bridge deletion** — snap handlers now normalize values, update and persist `SessionStateRes.app_settings`, resnap the selected App-owned object, and record App history directly. Block-id/recent/pick handlers maintain `EditorConfigRes.recent_block_ids` directly. Runtime config convergence tests were reduced to App-authority coverage; `sync_app_editor_config_state` and bridge-only recent-block/placement-rotation State accessors are deleted.
- [x] **Phase 3.154 / Editor-timeline bridge deletion** — timeline messages, playback audio/seek systems, tap arrays, selection, cursor updates, and render extraction are App-owned. Runtime timeline convergence tests were replaced by compact App-resource and one-time-seed coverage; `sync_app_editor_timeline_state`, the frame-end timeline overwrite, `State::sync_editor_timeline_playback_from_app`, and bridge-only State tap-array/selection methods are deleted.
- [x] **Phase 3.155 / Test session bridge deletion** — `SessionStateRes` is authoritative for session metadata, settings, pause, playtest, practice, and editor-window state. Runtime convergence tests now assert App resources directly; `sync_app_session_state`, the frame-end pause/playtest reverse mirror, and bridge-only State pause/practice/place-window wrappers are deleted. Compatibility test construction still seeds `SessionStateRes` once.
- [x] **Phase 3.156 / Runtime State ownership deletion** — Runtime test construction consumes legacy `State` fixtures into App resources and no longer retains `State`. The final per-frame `sync_app_gameplay_world_if_needed` call, orphaned gameplay reseed dirty flag/producers, final `runtime.state` assertions, and resize `applied_size` compatibility payload are deleted. `Runtime` now owns only `bevy_app::App` in production and tests.
- [x] **Phase 3.157 / Dead State compatibility adapter deletion** — App-world navigation and Escape systems own pause/resume, practice-mode mutation, selection clearing, timeline reset, and phase transitions. The unreferenced `escape_compat.rs` and `gameplay_compat.rs` State-only state machines are deleted.
- [x] **Phase 3.158 / UI snapshot naming cleanup** — The old `UiStateSnapshot` type and State producer were already gone. The remaining App-owned view model module is renamed from `ui_snapshot.rs` to `ui_state.rs`, its uncalled `UiStateData::from_snapshot` converter is deleted, and eg-ui test fixtures now use `editor_ui_view` naming.
- [x] **Phase 3.159 / Legacy frame callback deletion** — Production had already stopped calling `State::update`. The test-only callback and callback-specific tests are deleted; App-world `StateSchedule` playback and `RenderExtractSchedule` trail tests now gate the equivalent frame behavior directly.
- [x] **Phase 3.160 / Legacy egui wrapper deletion** — Production presents through App-owned render resources in `RenderSet::Present`. The test-only `State::render_egui`, `State::render_egui_with_frame_input`, and `State::create_egui_renderer` wrappers plus their redundant no-surface compatibility test are deleted.
- [x] **Phase 3.161 / Startup constructor unification** — Native and WASM startup now use `StateAppBootstrap` in both production and test builds. The platform `State::new` and `State::new_native` compatibility constructors are deleted; `StateInitialization::into_state` remains test-support-only for shared GPU fixtures.
- [x] **Phase 3.162 / Legacy surface-wrapper deletion** — The test-only `State::render`, `State::render_with_overlay`, `State::recreate_surface`, and State surface-dimension helpers plus their redundant no-surface test are deleted. App-owned `present_render_frame_system` retains the bare-world no-GPU gate; `State::render_to_view` remains temporarily for pixel-level compatibility tests.
- [x] **Phase 3.163 / Obsolete update module deletion** — After `State::update` was removed, the remaining test-only duplicate playback, trail, and trigger helpers moved to `editor_playback`, `editor_trail`, and App render extraction. Their replacement checks are synchronous and resource/pure-function based; `state/update.rs` is deleted.
- [x] **Phase 3.164 / State block-icon wrapper deletion** — Production block-icon generation already runs through `eg_render::BlockIconCache` and App-owned `RenderGpuRes`. The forwarding `State::render_block_icon_snapshot` API and its GPU-backed compatibility tests are deleted with `state/render/block_icon.rs`.
- [x] **Phase 3.165 / App-native Runtime test construction** — TestApp now seeds default menu and gameplay resources and can transfer its configured Bevy App into Runtime. All eg-app Runtime and application tests initialize authoritative App resources directly; the Runtime constructor that consumed State fixtures and all eg-app State imports are deleted.
- [x] **Phase 3.166 / State snapshot-constructor surface deletion** — Removing Runtime State fixture construction made the one-time auth, menu, editor, session, timing, spawn, camera, gameplay, frame-time, phase, and async-channel snapshot/install methods unreachable. Those methods and the now-dead State frame-runtime field are deleted; StateInitialization retains frame timing only for direct App bootstrap installation.
- [x] **Phase 3.167 / Direct editor-domain fixture migration** — Trigger, selection-normalization, camera-input, camera-track, snap-override, and picking coverage now constructs EditorSubsystem directly and runs synchronously. The test-only State camera orientation wrappers and their redundant phase-gating test are deleted; App camera-message coverage remains the runtime parity gate.
- [x] **Phase 3.168 / Waveform resource-path unification** — Cached and uncached waveform loading now share one resource-level implementation used by the App session handler and the remaining compatibility caller. Audio coverage uses AudioState plus EditorTimingState directly, removing its GPU-backed State fixture and duplicate App/State loading logic.
- [x] **Phase 3.169 / Editor interaction fixture ownership** — Selection, gizmo, and history coverage now runs synchronously on direct EditorSubsystem fixtures. Wrapper-only State mesh/phase assertions are removed while immediate marquee selection, phase-gated gizmo drag, history limits, undo/redo, clipboard, and transform-dirty behavior remain covered at the domain boundary.
- [x] **Phase 3.170 / Editor action fixture ownership** - Editor action and editor-state coverage now runs synchronously on direct EditorSubsystem fixtures. Transform-trigger creation is owned by EditorSubsystem, while the remaining State method only applies compatibility presentation updates.
- [x] **Phase 3.171 / Level-management fixture ownership** - Legacy GPU-backed State level-management fixtures are deleted after import, export, settings, navigation, practice-checkpoint, and preview-camera behavior moved under App-world handler coverage. The two pure menu-preview camera geometry tests remain beside their helpers.
- [x] **Phase 3.60 / Editor Escape-to-menu ownership** — editor `Escape` at timeline origin now performs the same App-owned phase/session/menu transition directly; selection, playback-stop, and timeline-reset Escape branches remain direct, while playtest/fallback Escape branches retain `EscapeRequest`. Schedule-level and runtime tests pin the request-free editor transition and one-shot audio-intent consumption.

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
(`State::dispatch` / `State::dispatch_editor`) has since been deleted:
UI commands and platform input now translate into typed messages through
`events::*` and `state::input_bridge` before the App-world handlers run.
Future phases migrate UI / winit / egui code to write
Message<T>::write directly, then delete the legacy enums + dispatch
files in one final sweep.

### What Phase 3 established

- 15 event submodules under events/ (
  avigation, auth,
  editor_mode, snap, block, selection, imeline, iming,
  bpm, spawn, history, camera, misc, session,
  input_state, pointer).
- One Message struct per semantic command (~85 distinct message
  types).
- Per-message handler systems in `StateSchedule`.
- Initially, handlers staged commands in `PendingCommands`; the active branch
  has since moved all message handlers to direct resource writes or typed
  request resources.
- Runtime consumes typed App-world requests through compatibility methods for
  side effects that still live on legacy `State`; the App world is now
  the only schedule host for `StateSchedule`.
- A HistorySnapshot marker event for direct-manipulation UI (gizmo
  drag / marquee / block drag) that reserves the PreCommandHistorySet
  slot in the schedule. A follow-up phase wires a real
  pre-mutation history snapshot system to it.

### Bevy 0.19 API notes

Bevy 0.19 uses Message (not Event) for buffered per-frame
payloads; observer-based Events fire immediately. We use
Message here because we want pull-based consumption by the
per-message handler systems in [events::CommandHandlerSet], with
side-effect requests consumed by Runtime in the same frame. The
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
| session | 22 | LoadLevel, ExportLevel, SetKeybind |
| input_state | 7 | SetShiftHeld, SetPanUpHeld (carve-out — see below) |
| pointer | 5 | MouseButton, PrimaryClick (carve-out — see below) |
| **Total** | **~102** | Plus 7 in input_state + 5 in pointer are deprecated |

### Input-state carve-out

Per the migration plan, the 7 input*state::* events and 5
pointer::_ events are **not** commands — they are \_input state_.
They now write directly to App-world resources (`EditorInputRes` and
`EditorCameraRes`) instead of staging `PendingCommands` for
`dispatch_editor`. Platform `InputEvent`s are queued in `PlatformInputQueue`
and drained by the App-world state schedule driver before semantic command
handlers.

REFACTOR POINT(full-bevy): the input-state + pointer events
disappear in the full-Bevy migration; the corresponding bevy_input
Input<KeyCode> / ButtonInput<MouseButton> resources handle
held-state + click events natively.

### Snap carve-out

The 4 `events::snap::*` messages (`SetSnapToGrid`, `SetSnapStep`,
`SetSnapRotation`, `SetSnapRotationStep`) are now a **carve-out** in the
same shape as the input-state carve-out: the per-message handlers in
`crates/eg-state/src/events/snap.rs` write directly to
`EditorConfigRes.0.snap_*` instead of staging a `PendingCommands`
entry.

`Runtime::run_app_frame` adds a new `sync_app_editor_config_state`
helper that copies `EditorConfigRes.0.snap_*` to the legacy
`State.editor.config.snap_*` fields before any `dispatch_editor`
runs. The sync is **change-guarded**: it only invokes the legacy
`set_editor_snap_*` setter when the resource value actually
differs from the legacy field. This is required because the
legacy setters have side effects (persist `app_settings` to
storage, re-snap the selected block); re-invoking them every
frame would re-spawn the persist task and churn the block
positions on every tick. The guard pins the side-effect-free
property while keeping the App world as the source of truth.

Tests in `crates/eg-state/src/events/mod.rs` (4 in `events::tests`

- the existing helper-resource fix) exercise the carve-out at the
  schedule level; tests in `crates/eg-app/src/platform/runtime.rs`
  (`runtime_carve_out_*`) exercise the full
  message → resource → sync → legacy-state pipeline through the
  `Runtime::run_app_frame` driver.

### Block-id carve-out + `EditorConfigRes` seeding

`events::editor_mode::handle_set_block_id` is the second stateful
config carve-out: it writes `EditorConfigRes.0.selected_block_id`
directly instead of staging `EditorCommand::SetBlockId`. The
`sync_app_editor_config_state` helper gained a change-guarded
`selected_block_id` sync whose legacy setter (`set_editor_block_id`)
has richer side effects than the snap setters: it remembers the
block in `recent_block_ids`, persists `app_settings`, and rebuilds
the editor cursor vertices when in the `Editor` phase. The
change guard ensures those side effects fire only on real
user-driven changes. The `recent_block_ids` side effect stays
legacy-authoritative (the UI reads it from the legacy
`UiStateSnapshot`, not from the resource), so the resource's
`recent_block_ids` mirror is not kept in lockstep yet — it is
refreshed when the resource is next seeded/rebuilt.

The same change closed a **latent first-frame clobber bug** that
affected the snap carve-out too: `EditorPlugin` Default-initializes
`EditorConfigRes`, but in real (`State::new_native` / `new_web`)
usage the legacy `State` loads user `app_settings` whose snap/block
config may differ from the defaults. Without seeding, the very
first frame's change-guarded sync would see `resource default !=
legacy user value`, fire the legacy setter, overwrite the user
value, and re-persist it — silently resetting the user's
preferences on every fresh launch. `Runtime::new` now seeds the
resource from the legacy `State` via `State::editor_config_resource()`
(`EditorConfigState: Clone`), so the first frame is a true no-op
for unchanged fields. `runtime_seeds_editor_config_res_from_legacy_state_no_first_frame_clobber`
is the regression test: it sets non-default snap values on the
legacy state before `Runtime::new` and asserts both the seeded
resource and the post-frame legacy state keep the user values.

Tests: 1 schedule-level test in `events::tests`
(`carve_out_set_block_id_mutates_editor_config_resource`) + 3
end-to-end tests in `runtime::tests`
(`runtime_carve_out_set_block_id_propagates_to_legacy_state`,
`runtime_carve_out_no_op_frame_does_not_resync_block_id`,
`runtime_seeds_editor_config_res_from_legacy_state_no_first_frame_clobber`).

### BPM carve-out + `EditorTimingRes` seeding

`events::bpm::handle_bpm_tap` and `handle_bpm_tap_reset` are the
**third stateful-resource carve-out** (after snap + block-id).
The handler performs the full bpm-tap calculation in the App
world (read wall-clock time, append to `bpm_tap_times`,
recompute running BPM as `60 / average diff`, truncate to 16
entries) and writes the result directly to
`EditorTimingRes.0.bpm_tap_times` and `bpm_tap_result`. The
legacy `State.editor_bpm_tap` path stays reachable via the
preserved `ToLegacy` impl.

`Runtime::run_app_frame` adds a new `sync_app_editor_timing_state`
helper that propagates the resource values back to the legacy
`State.editor.timing` fields. The sync is change-guarded so a
no-op frame does not re-invoke the legacy setters.

`Runtime::new` seeds `EditorTimingRes` from the legacy state via
`State::editor_timing_resource()` (which required adding
`Clone` to `EditorTimingState` and the `EditorSubsystem::timing`
field). Without seeding, the change-guarded sync would clobber
an in-flight tap history on the first frame: the default
resource is empty, the legacy state has taps, the diff fires,
and the user value is overwritten with the default. The
`runtime_seeds_editor_timing_res_from_legacy_state_no_first_frame_clobber`
test pins this.

Tests: 2 schedule-level tests in `events::tests`
(`carve_out_bpm_tap_mutates_editor_timing_resource`,
`carve_out_bpm_tap_reset_clears_editor_timing_resource`) + 4
end-to-end tests in `runtime::tests` (BpmTap propagation,
BpmTapReset propagation, no-op resync guard, first-frame
no-clobber regression).

### Session carve-out + `SessionStateRes` seeding

Five `events::session::*` messages are the **fourth
stateful-resource carve-out** (after snap, block-id, bpm):
`SetShowMetadata`, `SetShowSettings`, `ToggleSettings`,
`SetSettingsSection`, and `SetKeybindCapture`. Their handlers
write directly to `SessionStateRes` fields
(`editor_show_metadata`, `editor_show_settings`,
`editor_settings_section`, `editor_keybind_capture_action`).
At this historical point, the other session events (`LoadLevel`,
`SetAudioBackend`, `TriggerAudioImport`, ...) kept the legacy
`PendingCommands` path because their legacy setters had
out-of-process side effects (level load, file IO, settings
persist, audio backend switch). The active branch now routes those
messages through typed request resources instead.

The active branch treats `SessionStateRes` as authoritative. Runtime test
construction seeds it once through `State::session_state_resource()`, and
Phase 3.155 deleted all per-frame State/session mirrors.

Tests now use schedule-level message coverage plus compact Runtime checks for
direct App-resource mutation and one-time fixture seeding.

### Timing carve-out (extends `EditorTimingRes`)

All 6 `events::timing::*` messages are carve-outs (the
**fourth stateful-resource carve-out** after snap, block-id, and
bpm). The handlers write `EditorTimingRes.0.timing_points` and
`timing_selected_index` directly. The bpm-tap fields (Phase 3.7)
and the timing-points fields share the same `EditorTimingRes`
resource and the same `Runtime::sync_app_editor_timing_state`
change-guarded sync. The handlers mirror the legacy
`EditorSubsystem::add_timing_point` / `update_timing_point_time` /
`update_timing_point_bpm` / `update_timing_point_time_signature` /
`set_timing_selected_index` behavior (sort ascending by
`time_seconds`, clamp BPM to `>= 1.0`, clamp time-signature
numerator/denominator to `>= 1`, bump `revision` on every
change). Phase 3.38 removed `ToLegacy` impls from these
carved-out messages; direct `EditorCommand` tests continue to
exercise the legacy dispatch methods separately.

`Runtime::new` already seeds `EditorTimingRes` from the legacy
state via `State::editor_timing_resource()` (added in Phase 3.7
for the bpm carve-out); the new fields ride on the same
resource, so the same seeding covers them. Without seeding, the
change-guarded sync would clobber a user-edited timing-point
list on the first frame, just like the bpm-tap clobber bug from
Phase 3.7. The `runtime_seeds_editor_timing_res_for_timing_points_no_first_frame_clobber`
test pins the no-clobber property for both `timing_points` and
`timing_selected_index`.

Tests: 7 schedule-level tests in `events::tests` (one per
carve-out message, plus an out-of-range `RemoveTimingPoint` no-op
test) + 8 end-to-end tests in `runtime::tests` (per-message
propagation, no-op resync guard, first-frame no-clobber
regression that exercises both `timing_points` and
`timing_selected_index`).

### Spawn carve-out + `EditorSpawnRes` resource

All 3 `events::spawn::*` messages are carve-outs. The
`SetSpawnHere` handler reads `EditorInputRes.0.cursor` and writes
`EditorSpawnRes.0.position`; the `RotateSpawnDirection` handler
cycles `EditorSpawnRes.0.direction` (using the existing
`eg_editor_domain::toggle_spawn_direction` helper). The
`RotatePlacementPreview` handler increments
`EditorConfigRes.0.selected_block_rotation_degrees[1]` by 90
degrees (wrapping into `0..360`), extending the existing
config carve-out.

A new `EditorSpawnRes` resource wraps `SpawnMetadata` (derived
`Default` for the `position = [0, 0, 0]`, `direction = Forward`
case). Compatibility test construction seeds it once from legacy
editor data. Phase 3.147 moved runtime coverage to the authoritative
resource and deleted the per-frame legacy spawn mirror. `sync_app_editor_config_state` is extended with a
change-guarded `selected_block_rotation_degrees` sync to keep
the legacy `State.editor.config.selected_block_rotation_degrees`
field in lockstep with the resource (without re-running the
legacy setters' side effects every frame).

Tests: 3 schedule-level tests in `events::tests` (one per
carve-out message) + 5 end-to-end tests in `runtime::tests`
(per-message propagation + a no-op resync guard + a first-frame
no-clobber regression that exercises the spawn position +
direction).

### Camera + trigger-selected carve-out

All 5 `events::camera::*` messages are carve-outs (the
**fifth stateful-resource carve-out** after snap, block-id, bpm,
session, timing, and spawn). The carved-out messages:

- `AdjustZoom(delta)` writes `EditorCameraRes.0.editor_pan` and
  `.editor_target_z` (mirroring `EditorSubsystem::adjust_zoom`).
- `SetCameraOrientation { rotation, pitch, transition_seconds }`
  writes `EditorCameraRes.0.editor_yaw`, `.editor_pitch`, and
  optionally installs a `CameraTransition` (mirroring
  `EditorSubsystem::set_editor_camera_orientation` in
  `state/editor_interaction/camera.rs`).
- `SetTriggerSelected(Option<usize>)` writes
  `EditorTriggerRes.0.selected_index` (mirroring
  `EditorSubsystem::set_trigger_selected`).
- `AddCameraTrigger` records App-world editor history, captures
  `EditorCameraRes` pose into a generated static camera trigger object,
  appends it to `EditorObjectsRes`, syncs trigger selection against the
  App-world object list, and marks `object_mutation_pending`.
- `AddCameraFollowTrigger` records App-world editor history, appends a
  generated follow-mode camera trigger object to `EditorObjectsRes`, and
  derives the visible trigger block pose from the App-world timeline
  snapshot cache before marking `object_mutation_pending`.

To support the carve-out, `EditorCameraState` and
`CameraTransition` now derive manual `Clone` impls (the previous
hand-rolled `Default` impl was already in place; `Clone` was
missing). `EditorCameraState::set_transition(...)` is a small
public helper that lets callers outside the `editor_camera`
module install a transition without depending on the private
`CameraTransition` type. `State::editor_camera_resource()` and
`State::editor_trigger_resource()` seed compatibility test Apps once.
Phases 3.146 and 3.148 moved runtime coverage to the authoritative
resources and deleted both per-frame legacy mirrors. The `EditorTriggerState` type
also gained `Clone` (it was `Default` but not `Clone`; the
`EditorTriggerRes` resource needs to be cloned by the seeding
helper).

Tests: 7 schedule-level tests in `events::tests` (one per
camera/trigger-selection carve-out, a `SetCameraOrientation` with
transition test, plus a phase guard for trigger creation) + 6
end-to-end tests in `runtime::tests` (per-message propagation + a
no-op resync guard + a first-frame no-clobber regression that
exercises both `EditorCameraRes` and `EditorTriggerRes`).

### Schedule graph (post Phase 3)

`Per-frame schedule order (StateSchedule):
  transitions::phase_transition_system   // AppPhase change detection
  events::handle_* (per-message)         // MessageReader<T> → PendingCommands`

Future phases add more sets at the top of the chain (InputSet,
ClickHandlingSet, ModeChangeSet, PreCommandHistorySet,
CommandHandlerSet, SimulationSet, UiSet, HistorySet,
RebuildMeshSet, ExtractSet, RenderSet). Phase 3 keeps the
schedule minimal so the existing legacy dispatch path is exercised
end-to-end on every frame.

**Historical verification:** the original Phase 3 pipeline was covered by
message → `PendingCommands` tests. The active branch replaces those with
direct-resource and request-resource tests in `events::tests`.

## Phase 4a — rendering extraction scaffolding

The plan calls for lifting `RenderSubsystem` (`crates/eg-state/src/state/render/mod.rs`) into Bevy resources (`GpuContextRes`, `SceneMeshesRes`, `SurfaceHostRes`, `CameraUniform`, `LineUniform`, `ColorSpaceUniform`) plus an `ExtractSet` / `RenderSet` system pair. Phase 4a is the **non-invasive scaffold** that establishes the boundary without touching the legacy `State::render` field (which has ~440 call sites).

### Phase 4a history and current replacement

Phase 4a originally introduced the mirror described below. Phase 3.69 superseded that bridge: `state::render::mirror`, `compute_render_uniforms_snapshot`, and the mirror schedule system are now deleted. The current `RenderExtractSchedule` derives both POD resources directly from App-world state.

- **New resource types** in `crates/eg-state/src/state/render/resources.rs`:
    - `RenderUniformsRes` — POD bundle of `CameraUniform` + `LineUniform` + `ColorSpaceUniform` (the per-frame uniform values).
    - `RenderFrameInputRes` — POD copy of `RenderFrameInput` (phase, editor mode, sky color, grid center, gamma toggle).
    - `RenderConfigRes` — App-owned viewport size and gamma configuration used during extraction.
    - `RenderUniformUploadRes` — production-only cloned queue and uniform-buffer handles used by scheduled uploads without moving surface ownership.
    - `RenderResizeRequest` — platform-side resize queue consumed by `state::render::plugin::apply_resize_surface_system`; it also carries the scheduled accepted-size handoff that Runtime mirrors into legacy `State` until heavy render resources move fully into the App world.
    - `RenderGpuRes`, `RenderMeshesRes`, `RenderSurfaceHostRes` — heavy types defined for the final ownership move; the compatibility renderer still owns the production surface and meshes.
- **`RenderPlugin`** in `crates/eg-state/src/state/render/plugin.rs` registers the POD resources and exposes the `RenderSet` enum (`RenderSet::Extract`, `RenderSet::Present`).
- **`update_render_frame_input_system` / `update_render_uniforms_system`** build the per-frame render inputs and camera/line/color-space uniforms from App-world resources. Runtime passes the resulting `RenderFrameInputRes` into the compatibility renderer, while scheduled camera and line uploads happen before present.
- **Schedule wiring**: `TestApp::tick_state` and production Runtime run `StateSchedule`, `GameplaySchedule`, then `RenderExtractSchedule`; no render closure is installed or drained.
- **Tests** in `state::tests` use `TestApp` resources directly to cover frame input plus menu, editor, and playing uniforms. These tests avoid full `State::new_test` GPU/UI setup and built-in-level loading.

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

The 4b-3 + 4b-4 commits covered the mirror-driven approach for all four `AppPhase` values + time + gamma. The heavy render ownership and present cut-over are now App-owned; remaining work is deleting the State compatibility accessors and old render-focused update helpers.

- **Phase 4c-scaffold** — `RenderMeshesRes` + `RenderSurfaceHostRes` are inserted with default values by `register_render_pod_resources`. Bevy systems can query for their presence. Two `Default` impls were added to `eg-render`: `MeshSlot::default() = MeshSlot::Empty` and `SceneMeshes::default()` (all 25 `MeshSlot` fields set to `Empty`, the two chunked/instanced slots use their existing `#[derive(Default)]` impls, `editor_selection_outline_instances = Vec::new()`).

- **Phase 4c-final** — complete. Production `Runtime::new` moves the actual `SceneMeshes` / `SurfaceHost` and `GpuContext` into `RenderMeshesRes` / `RenderSurfaceHostRes` / `RenderGpuRes`. The compatibility `State` slot is empty after transfer; only test-only legacy callers use its accessors.

- **Phase 4d-scaffold** — added the `RebuildMeshSet` system set + the `RebuildMeshEvent` / `RebuildMeshKind` types in `crates/eg-state/src/events/mesh_rebuild.rs`. The 9 `RebuildMeshKind` variants correspond 1:1 to the 9 bits on the legacy `EditorDirtyFlags` bitfield.

- **Phase 4d-final** — complete. `eg-game::ObjectBundle` now includes authored mirror components (`Position`, `Rotation`, `Size`, `BlockId`, `ColorTint`) alongside `ObjectData`. `mark_mesh_dirty_system` uses Bevy `Changed<>` filters over those components and emits `SyncGameObjects` + `RebuildBlockMesh` events. `EditorDirtyFlags` remains as the legacy editor-side consumer until editor mutations move into the shared World.

- **Phase 4e-scaffold** — replaced by Phase 4e-final. The old placeholder is gone.

- **Phase 4e-final** — complete at the custom-render boundary. `present_render_frame_system` runs after extraction and consumes `RenderGpuRes`, `RenderMeshesRes`, `RenderSurfaceHostRes`, `RenderUniformsRes`, `RenderFrameInputRes`, and `RenderEguiRes`. Runtime no longer calls a State render method; it only prepares the egui frame resource before the App schedule.

### Schedule graph (post Phase 4)

`Per-frame schedule order:
  transitions::phase_transition_system        // AppPhase change detection
  mirror::install_mirror                      // tick_state installs the closure
  render::mark_mesh_dirty_system              // Changed<> authored object detection -> RebuildMeshEvent
  events::handle_* (per-message)              // MessageReader<T> -> PendingCommands
  async_bridge::poll_channels_system          // drain 5 shared receivers -> 5 result events
  async_bridge::add_apply_async_results_systems // consume events -> install closures
  render::mirror_pod_state_via_bridge         // compatibility uniform snapshot -> RenderUniformsRes
  render::present_render_frame_system         // read authoritative render resources
  GameplaySchedule                            // advance App-world gameplay
  RenderExtractSchedule                       // rebuild POD values + upload camera/line uniforms
  // tick_state then drains pending async result closures -> mutates State
  // PendingCommands remain staged; Runtime owns the App-world compatibility drain`

The order matches the legacy `render_to_view` / `render_with_overlay` /
`State::update` call sequence: simulation → mesh rebuild → extract →
present. Future commits add real systems into each set.

### Schedule graph (post Phase 4a)

`Per-frame schedule order (StateSchedule):
  transitions::phase_transition_system        // AppPhase change detection
  mirror::install_mirror                      // tick_state installs the closure
  events::handle_* (per-message)              // MessageReader<T> → PendingCommands
  render::mirror_pod_state_via_bridge         // drain closure → RenderUniformsRes / RenderFrameInputRes
  render::present_render_frame_placeholder    // no-op in Phase 4a`

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

- `crates/eg-ui/src/plugin.rs` defines `UiPlugin`, `EguiContextRes`, and the exhaustive immediate `AppCommand` / `EditorCommand` -> Bevy `Message` writer.
- `show_editor_ui_from_view` renders the existing editor UI through a `UiCommandSink`; production uses `WorldUiCommandSink` to write messages immediately, while `show_editor_ui_commands_from_view` remains a focused vector adapter for tests.
- `Runtime::run_app_frame` renders editor/menu/pause UI using App-owned egui presentation resources and writes typed messages directly into the App world; no intermediate UI queue remains.
- `crates/eg-app/src/platform/runtime.rs` writes production UI commands through `WorldUiCommandSink`, runs `app.update()`, prepares App-world mirror resources from legacy `State`, runs `StateSchedule`, then applies remaining typed requests to the legacy `State` before the compatibility update/render path.
- The visual UI code now reads `UiStateSnapshot` during the compatibility window, but `Runtime::run_app_frame` mirrors the live egui context into App-world `EguiContextRes` before the frame update. The remaining UI cut-over is replacing snapshot reads with App-world resources/queries.

**Verification:** `cargo check -p eg-ui` and `cargo check -p eg-app` pass. Both runs reported recurring Windows incremental-cache finalization warnings (`Access is denied`) but exited successfully.

## Phase 6a — async bridging scaffold

The plan calls for wrapping the five `mpsc` channels the legacy `State` struct owns (auth service, audio import, waveform load, audio preload, level import) in Bevy `Resource`s and adding a `PollChannelsSet` system that drains them and emits per-channel result events. Phase 6a established the scaffold; Phase 6b added handler systems; the current runtime frame now drains and applies these async results directly to App-world resources through `StateSchedule` before the remaining `State::update` branch logic runs.

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
- **Schedule wiring**: `register_state_command_pipeline` (in `state::plugins`) chains `add_poll_channels_systems` after `add_command_systems`. The poll system runs once per frame alongside the message handlers; render extraction runs later in `RenderExtractSchedule`.
- **Tests** (3 integration + 8 unit + 5 module = 16 new tests):
    - `tick_state_poll_channels_emits_audio_import_result_event` — end-to-end: write to a channel, `tick_state`, assert the event is in `Messages<AudioImportResultEvent>`.
    - `tick_state_poll_channels_emits_level_import_result_event` — same for level import.
    - `tick_state_poll_channels_emits_auth_service_result_event` — same for auth service.
    - 7 unit tests in `events::async_results::tests` cover the event constructors + payload preservation.
    - 5 unit tests in `state::async_bridge::tests` cover `drain_one` (Empty/Disconnected handling), `register_state_async_bridge_world` (resource initialization), and `install_async_bridge_from_state` (idempotency).

### Design notes

- **Why messages and not direct resource writes?** — distinct `Message<T>` types give each handler its own `MessageReader<T>`, so multiple readers of the same channel can be added without cross-reader nondeterminism. Tests can write a synthetic event into the `Messages<...>` buffer and assert the handler behavior without going through the platform layer.
- **Why a `Mutex<Receiver<T>>` wrapper?** — `mpsc::Receiver<T>` is `Send` but not `Sync` (its `try_recv` API is `&self` but the value can't be shared between two consumers without a lock). Bevy `Resource` requires `Send + Sync + 'static`. The wrapper's `Mutex` is uncontended in practice.
- **Why is the install not auto-wired?** — `mpsc::Receiver<T>` is not `Clone`. The legacy fields now store `Arc<Mutex<Receiver<T>>>`, and the World resources hold clones of those same arcs. Runtime installs them before `StateSchedule`, so `poll_channels_system` drains the actual legacy receivers once per production frame. Tests that exercise the bridge can still insert resources manually.
- **Idempotent install** — `install_async_bridge_from_state` is a no-op for already-installed resources, so calling it from multiple frames is safe.

### Schedule graph (post Phase 6a)

`Per-frame schedule order (StateSchedule):
  transitions::phase_transition_system        // AppPhase change detection
  mirror::install_mirror                      // tick_state installs the closure
  events::handle_* (per-message)              // MessageReader<T> -> PendingCommands
  render::mirror_pod_state_via_bridge         // drain closure -> RenderUniformsRes / RenderFrameInputRes
  render::present_render_frame_placeholder    // no-op in Phase 4a
  async_bridge::poll_channels_system          // drain 5 receivers -> 5 result events`

### Bevy 0.19 API notes (Phase 6a)

- `#[derive(Message)]` is used for the 5 new event types. `WaveformResultEvent` is intentionally **not** `Clone` because `WaveformLoadMessage` doesn't implement `Clone`. The other 4 events derive `Clone` for ergonomic test usage.
- `Mutex<T>` is `Send + Sync` when `T: Send`, so the channel wrappers satisfy Bevy's `Resource` bound. The receivers are owned by value (no `Arc`); the wrappers are inserted at most once per world (idempotent install).
- The `poll_channels_system` takes `Option<Res<...>>` for each wrapper so it works whether or not the wrapper is installed. This is the same pattern Bevy uses for optional system params and avoids the need for separate "registered" / "unregistered" code paths.

## Phase 6b — async result handler systems

Phase 6a established the channel wrappers and the `poll_channels_system` that emits 5 result events. Phase 6b originally routed non-auth side effects through a temporary thread-local `State` closure bridge. Phase 3.92 superseded that bridge: all five handlers now mutate App-world resources directly, and the historical details below document the intermediate implementation.

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
    - `auth_handler_system` — consumes `AuthServiceResultEvent` and mutates `AuthStateRes` directly. Runtime mirrors that resource back to legacy `State` until UI/auth state reads leave the compatibility shell.
    - `audio_import_handler_system` — consumes `AudioImportResultEvent`, installs a closure that mirrors the body of `State::update_audio_imports` (insert into `EditorAudioCache`, reload waveform).
    - `waveform_handler_system` — consumes `WaveformResultEvent`, installs a closure that mirrors the body of `State::update_waveform_loading` (per `WaveformLoadMessage` variant: `Started` / `Chunk` / `Cached` / `Finished` / `Failed`). Because `WaveformResultEvent` is not `Clone`, this system takes `&mut World` and `drain`s the `Messages<>` buffer directly.
    - `audio_preload_handler_system` — consumes `AudioPreloadResultEvent`, installs a closure that mirrors the body of `State::update_runtime_audio_preloads` (remove from `preloading_source_keys`, insert into `preloaded_audio`).
    - `level_import_handler_system` — consumes `LevelImportResultEvent`, installs a closure that calls `State::import_level_egz` / `State::import_level` (with the same error logging as the legacy `update_level_imports`).

- **`ApplyAsyncResultsSet`** system set + `add_apply_async_results_systems` registration helper.

- **Thread-local bridge**: `install_async_result_fn(F: FnOnce(&mut State) + 'static)` + `drain_pending_async_result_fns(state: &mut State)`. The handler systems install `FnOnce(&mut State)` closures into a `thread_local!` `RefCell<Vec<Box<dyn FnOnce(&mut State)>>>` slot. `State::tick_state` drains the slot and applies each closure after the schedule finishes. The former render mirror that used this pattern has been deleted; async result handlers are the remaining thread-local bridge.

- **`install_async_bridge_from_state_arc`** helper. `State::tick_state` calls this on every frame to wire the 5 `Arc<Mutex<Receiver<T>>>` fields into the World's resources (idempotent — no-op if already installed). This replaces the Phase 6a `install_async_bridge_from_state` (which took owned `Receiver<T>` and is still used by tests that build a world from scratch with fresh channels).

- **Visibility bumps**:
    - `AuthStateRes::complete_auth_sign_in` / `complete_auth_refresh` / `complete_auth_sign_out` are used by the auth handler system; the legacy `State::complete_auth_*` methods remain for direct `State::update_auth_results` tests.
    - `state::audio_state::persist_waveform_cache_entry` made `pub(crate)` so the waveform handler's `apply_waveform_message` helper can call it from a closure.

- **Tests** (15 new unit + 0 integration = 15 new tests):
    - `install_async_bridge_from_state_arc_shares_receivers` — `Arc::ptr_eq` confirms the resource holds the same `Arc` we passed in.
    - `install_async_bridge_from_state_arc_is_idempotent` — second install with fresh receivers doesn't overwrite the first.
    - `async_result_fn_slot_drains_in_order` — the thread-local slot drains closures in insertion order.
    - `async_result_fn_slot_drain_is_idempotent` — second drain with no new closures is a no-op.
    - `auth_handler_system_updates_auth_resource_for_sign_out` — write `SignedOut(Ok(()))`, run system, verify `AuthStateRes.session = None` and `pending = false`.
    - `audio_import_handler_system_runs_with_cancellation` — write `bytes = None`, run system, verify no panic.
    - `waveform_handler_system_drains_events` — write `Failed` event, run system (drains the buffer), verify no panic.
    - `audio_preload_handler_system_inserts_bytes` — write `Some(bytes)`, run system, verify `preloaded_audio` contains the entry.
    - `audio_preload_handler_system_handles_failure` — write `None`, run system, verify nothing inserted.
    - `level_import_handler_system_is_safe_on_invalid_bytes` — write invalid bytes, run system, verify no panic.

- **Existing test updates** (no behavior change): the 3 Phase 6a integration tests (`tick_state_poll_channels_emits_*`) now insert `Arc<Mutex<Receiver<T>>>` resources instead of `Mutex<Receiver<T>>` to match the new resource type.

### Design notes

- **Why a thread-local bridge?** `State` is `!Send` (it owns `mpsc::Receiver<T>` fields, even when wrapped — see `Arc<Mutex<Receiver<T>>>: !Send` because `MutexGuard` is `!Send`). Bevy schedules run on the main thread today; the thread-local slot is the simplest correct mechanism for handler systems to install closures that take `&mut State` and have them run on the same thread. The full-Bevy migration replaces this with `bevy_tasks` async event streams and direct resource mutation — see refactor point #5.

- **Why one less `Mutex` layer than Phase 6a?** Phase 6a wrapped the receiver in `Mutex<Receiver<T>>` for the resource. Phase 6b shares the existing `Arc<Mutex<Receiver<T>>>` directly with the resource (no second `Mutex`). `poll_channels_system` locks that shared receiver once per scheduled frame. The cost is the same (one `lock()` per `try_recv`); the benefit is no double-wrapper.

- **Why the `drain` pattern in `waveform_handler_system`?** `WaveformResultEvent` carries a `WaveformLoadMessage` which is not `Clone`. `MessageReader<T>::read` returns an iterator over `&T`; we can't move the message out without taking `&mut World` and using `Messages<T>::drain`. The other 4 handler systems use the standard `MessageReader<T>` pattern (their payloads are `Clone`).

- **Frame semantics**: production frames run App update, prepare the App-world mirrors, run `StateSchedule` (poll + handler systems), drain the async-result closure slot into legacy `State`, then `Runtime` supplies the remaining pre-render compatibility `State::update` call before render. Direct `update_*` helper calls remain available for focused compatibility tests, but `State::update` no longer polls those async channels.

    The scheduled systems are the intended path; the legacy methods exist for compatibility with tests that drive the State directly without going through the schedule. Running both paths on the same frame must not double-process async channel messages.

### Schedule graph (post Phase 6b)

`Per-frame schedule order (StateSchedule):
  transitions::phase_transition_system        // AppPhase change detection
  mirror::install_mirror                      // tick_state installs the closure
  events::handle_* (per-message)              // MessageReader<T> -> PendingCommands
  async_bridge::poll_channels_system          // drain 5 shared receivers -> 5 result events
  async_bridge::add_apply_async_results_systems // consume events -> install closures
  render::mirror_pod_state_via_bridge         // drain closure -> RenderUniformsRes / RenderFrameInputRes
  // tick_state then drains pending async result closures -> mutates State
  // PendingCommands remain staged; Runtime owns the App-world compatibility drain`

### Bevy 0.19 API notes (Phase 6b)

- `thread_local!` with `RefCell<Vec<Box<dyn FnOnce(&mut State)>>>` is the bridge slot. The closures don't need to be `Send` because the schedule + the drain both run on the main thread. The slot is process-global; tests that don't go through `tick_state` should not leave stale closures (the slot is drained on every `tick_state` call).
- `pub(crate)` visibility is used for `persist_waveform_cache_entry` and legacy `complete_auth_*` methods that remain covered by direct compatibility tests. Auth handler systems now mutate `AuthStateRes`; the non-auth async handlers still install temporary closures.
- `Arc<Mutex<Receiver<T>>>` is `Send + Sync` (the inner `Receiver<T>: Send`), so it satisfies Bevy's `Resource` bound. The resource holds a clone of the `Arc`; the legacy field holds its own clone; both consumers race for messages via the same `Mutex`.

### Verification

- `cargo test -p eg-state --lib` is green: **434 tests pass** (was 424 before Phase 6b, +10 new unit tests).
- `cargo build -p eg-state` is clean.
- `cargo clippy -p eg-state --all-targets` showed no **new** warnings introduced by Phase 6b (pre-existing baseline warnings in `transitions.rs` doc lists, `events/mod.rs` let-bindings, and `gizmo.rs` field-assignments were not Phase 6b regressions — they predated that phase).

## Phase 6c — frame timing system

Phase 6c starts decomposing `State::update` itself. `FrameTimeRes` is now an
App-world resource registered by `EditorPlugin` and seeded by `Runtime::new`.
`advance_frame_time_system` runs in `StateSchedule` and owns:

- `puffin::GlobalProfiler::new_frame()`
- frame delta calculation
- global render time advancement
- FPS smoothing mirrored into `PerfRes`

`Runtime::run_app_frame` syncs the scheduled `FrameTimeRes` into the legacy
`State` before Runtime supplies the remaining pre-render compatibility
`State::update` callback. Direct `State::update` callers still have a fallback
timing path so legacy tests and focused state methods keep working while the
rest of the update branches migrate.

**Verification:** `cargo test -p eg-state test_app_tick_state_advances_frame_time_resource --target-dir target\codex-test` and `cargo test -p eg-app runtime_run_app_frame_updates_app_state_schedule --target-dir target\codex-test` pass. Some Windows runs reported recurring incremental-cache finalization warnings (`Access is denied`) but exited successfully.

## Phase 6d — color-space uniform system

Phase 6d moves the per-frame color-space time value out of the inline
`State::update` computation. The render POD mirror still supplies the
compatibility gamma and viewport values, then `update_color_space_uniform_system`
runs after the mirror and after `advance_frame_time_system` so
`RenderUniformsRes.color_space.time_seconds` follows App-world `FrameTimeRes`.

`Runtime::run_app_frame` copies the scheduled `RenderUniformsRes` color-space POD
value into the legacy `State` before Runtime supplies the pre-render
compatibility `State::update` callback.
`State::update` then uploads that scheduled uniform to the current wgpu buffer.
Direct `State::update` callers keep a fallback that computes and uploads the
uniform from legacy fields until the rest of the update/render path is scheduled.

**Verification:** `cargo test -p eg-state test_app_tick_state_advances_frame_time_resource --target-dir target\codex-test` and `cargo test -p eg-state tick_state_mirrors_color_space_time_and_gamma --target-dir target\codex-test` pass. Both runs reported recurring Windows incremental-cache finalization warnings (`Access is denied`) but exited successfully.

## Phase 7 — App wiring complete

Phase 7 flips the redraw boundary so the winit loop drives the Bevy `App` every frame. App update, schedule-local service requests, egui frame preparation, render extraction, and custom scene/egui presentation now sit in `Runtime::run_app_frame(raw_input)` plus the App schedules: UI writes typed messages through `WorldUiCommandSink`, Runtime prepares `RenderEguiRes`, then `StateSchedule` and `RenderExtractSchedule` perform mutation, mesh upload, and present. `EditorObjectsRes`, `EditorHistoryRes`, `EditorTimelineRes`, and `EditorRuntimeRes` are no longer overwritten during production preparation. Auth request/result handling already runs inside `StateSchedule` through `AuthStateRes`. The duplicate `State::world` schedule, `State::tick_state`, `PendingCommands` command drain, and `FramePipeline` wrapper have been deleted.

### What's in Phase 7 today

- **`Runtime` owns a `bevy_app::App` field** alongside the remaining lifecycle compatibility `state: State` field. The App contains the non-send egui and wgpu presentation resources. Runtime runs `app.update()`, then explicitly runs the App-world `StateSchedule` and post-extraction present schedule once per frame.

- **`Runtime::new` builds and drives the App**:

    ```rust
    let mut app = bevy_app::App::new();
    setup_state_app(&mut app);
    register_state_command_pipeline(app.world_mut());
    register_ui_resources_world(app.world_mut());
    ```

    Both helpers are already public from Phases 2 and 3. The App's `World` now has:
    - Every `StatePlugins` resource (`AppPhaseRes`, `SessionStateRes`, the 9 `Editor*Res`, `PerfRes`).
    - The platform input queue (`PlatformInputQueue`) registered by `EditorPlugin`.
    - Every `RenderPlugin` POD resource (`RenderUniformsRes`, `RenderFrameInputRes`, `RenderResizeRequest`) + the `Messages<RebuildMeshEvent>` buffer.
    - The live UI context resource (`EguiContextRes`) registered by `eg-ui`.
    - The `StateSchedule` registered by `register_state_command_pipeline`.
    - A Runtime-owned frame helper (`run_state_schedule_in_world`) that runs `world.try_run_schedule(StateSchedule)` after `app.update()`; `StateSchedule` now owns phase transition detection.

    - **`Runtime::process_input_event(event)`** — queues platform input into App-world `PlatformInputQueue`. The state schedule driver drains the queue into `EditorInputRes` and typed semantic messages before command handlers run. Production winit/touch/resize/key/mouse callers use this Runtime method instead of reaching into legacy `State`.

    - **`Runtime::run_app_frame(raw_input)`** — mirrors the live egui context into App-world `EguiContextRes`, runs the App-owned UI preparation, writes UI messages through `WorldUiCommandSink`, stores paint jobs and texture deltas in `RenderEguiRes`, runs `app.update()`, then runs the App-world `StateSchedule` and post-gameplay `RenderExtractSchedule` so extracted meshes and the custom scene/egui presenter execute from App-owned resources. The old `Runtime::run_frame` and `FramePipeline` wrappers are gone; production callers use the App-driving entry point.

- **`ApplicationHandler::RedrawRequested` calls `runtime.run_app_frame(raw_input)`** on native and wasm. This makes the production redraw path exercise the Bevy `App` before rendering.

- **`StatePlugins` now own App-world defaults** — `MenuPlugin`, `SessionPlugin`, `EditorPlugin`, `GameplayPlugin`, and `RenderPlugin` install their default resources directly. `GameplayPlugin` now adds `eg_game::GamePlugin`, so the App world owns `GameClock` and `GameplaySchedule`. The bare-`World` `register_state_resources` helper remains only for focused tests.

- **Re-export** of `register_state_command_pipeline` and `run_state_schedule_in_world` from `eg_state::state` (the `plugins` module stays private; only the helpers are re-exported for `eg-app` to call).

- **`bevy_ecs` added to `eg-app/Cargo.toml`** — the runtime tests inspect the App's `World` via `World::contains_resource`, `World::resource`, and `World::try_run_schedule`. Those methods need the `World` type in scope.

- **Tests** in `crates/eg-app/src/platform/runtime.rs::tests`:
    - `runtime_app_has_state_plugin_resources` — every `StatePlugins` resource is registered on the App's World.
    - `runtime_app_registers_state_schedule` — `StateSchedule` is reachable via `try_run_schedule` on the App's World.
    - `runtime_run_app_frame_returns_output` — `run_app_frame` produces normal egui output.
    - `runtime_run_app_frame_syncs_egui_context_to_app_world` — `run_app_frame` mirrors the live egui context into App-world `EguiContextRes`.
    - `runtime_process_input_event_queues_app_world_input` — platform input enters App-world `PlatformInputQueue` for scheduled draining into resources/messages.
    - `runtime_run_app_frame_updates_app_state_schedule` — `run_app_frame` drives `app.update()`, then explicitly runs the App-world `StateSchedule`.
    - `runtime_app_update_does_not_run_state_schedule_directly` — plain `app.update()` no longer advances the mirror `StateSchedule`; Runtime owns that order.
    - `runtime_run_app_frame_dispatches_app_world_commands` — an App-world message drains through `StateSchedule` and is applied to the legacy `State` during the compatibility window.
    - `runtime_app_world_setup_does_not_mutate_state` — App setup does not mutate legacy `State` until `Runtime` explicitly applies queued commands.

    - **Tests** in `crates/eg-state/src/state/tests.rs`:
        - `test_app_tick_state_writes_command_messages_to_app_resources` — the App-world schedule runs the per-message systems and leaves request consumption to Runtime.
        - `test_app_tick_state_writes_input_state_directly` — input-state messages mutate `EditorInputRes` through `TestApp` without constructing legacy `State`.
        - `test_app_tick_state_writes_pointer_state_directly` — pointer/camera messages mutate `EditorInputRes` and `EditorCameraRes` through `TestApp` without constructing legacy `State`.
        - `test_app_tick_state_applies_undo_to_app_resources` — undo restores `EditorObjectsRes` from `EditorHistoryRes` through `TestApp`.
        - `test_app_tick_state_practice_mode_mutates_session_resource` — practice-mode navigation writes `SessionStateRes` through `TestApp`.

- **Dead-code allowances removed** from `Runtime.app` and `Runtime::run_app_frame` because both are now production-used.

### Schedule graph (post Phase 7)

The production redraw path now writes UI commands into the App world from `Runtime`, prepares `RenderEguiRes`, runs the App `Update` schedule, explicitly runs the App-world `StateSchedule`, and then runs the post-extraction mesh upload and custom scene/egui present system. UI command ingress is App-world first and the render pipeline no longer owns the App bridge or directly invokes `State::update`.

Production platform input is also App-world owned: winit/touch/resize/key/mouse events call `Runtime::process_input_event`, which writes to `PlatformInputQueue(App::world)` for the state schedule driver to drain into `EditorInputRes` and typed command messages before `StateSchedule` runs.

```
RedrawRequested:
  Runtime::run_app_frame(raw_input)
    sync_egui_context(App::world, egui::Context)
    Runtime App-owned UI preparation
      egui UI -> WorldUiCommandSink(App::world) -> Messages<T>
      compatibility callers -> write_ui_command(App::world) -> Messages<T>
      RenderEguiRes <- tessellated paint jobs + texture deltas
      app.update()
      run_state_schedule_in_world(App::world)
        app.world.try_run_schedule(StateSchedule)
          transitions::phase_transition_schedule_system
        GameplaySchedule
        RenderExtractSchedule -> extracted meshes -> RenderSet::Present
      Runtime mirrors only remaining session compatibility fields
```

### Why legacy State still coexists with the App world

The App now owns the only ECS `World` and all production render handles, but legacy `State` still owns subsystem fields during the compatibility window:

- `State` is `!Send` (it owns `mpsc::Receiver<T>` fields), while Bevy's `App` expects `World` resources to be `Send + Sync`.
- `State` is constructed in `State::new_test` / `new_native` / `new_web` with custom initialization logic. Production `Runtime::new` immediately transfers the heavy render handles into App non-send resources; test-only State callers retain their slot.
- Async result handlers mutate App-world resources directly. Shared audio-cache handles let remaining compatibility playback code read authoritative bytes without placing `State` in the App `World`; the former thread-local async and render bridges no longer exist.

The remaining post-Phase-7 gates are moving typed request side effects and UI preparation out of Runtime, replacing `UiStateSnapshot` reads with App-world resources/queries, and deleting the remaining State subsystem fields.

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
    - `TestApp::tick_state()` — Bevy-native equivalent of `State::tick_state()`. Runs `StateSchedule`, including phase transition detection.
    - `TestApp::run_state_schedule()` — runs `StateSchedule` and returns whether it completed successfully.
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

- **Why keep `phase_transition_system` as a free function?** Focused transition tests still call the free function directly, but production and `TestApp` frames now use the `phase_transition_schedule_system` wrapper registered in `StateSchedule`.

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

- Production is App-world authoritative for rendering, input, async work, gameplay, editor mutation, and frame timing. Legacy State is now a GPU-backed test-support fixture only.
- The `AppCommand` (15 variants) + `EditorCommand` (~87 variants) enums are **still defined** for non-message legacy call sites and compatibility methods, but no event handler converts through `LegacyCommand`/`ToLegacy`; those bridge types are deleted.
- Production and test Runtime own only bevy_app::App. Native and WASM startup install StateAppBootstrap directly, while unit coverage uses App-native TestApp fixtures.
- **Phases 3.5–3.11 carve-out status** (as of `be4d965` +
  this branch's continuation): 23 of the ~85 legacy-routed
  `EditorCommand` messages have been carved out to direct
  resource writes. The carved-out domains and their App-world
  resources are:
    - `events::snap` (4 messages) → `EditorConfigRes.0.snap_*`
    - `events::editor_mode::handle_set_block_id` (1 message) →
      `EditorConfigRes.0.selected_block_id`
    - `events::bpm` (2 messages) → `EditorTimingRes.0.bpm_tap_*`
    - `events::session::SetShowMetadata / SetShowSettings / ToggleSettings / SetSettingsSection / SetKeybindCapture` (5 messages) → `SessionStateRes` fields
    - `events::timing` (6 messages) → `EditorTimingRes.0.timing_points` + `timing_selected_index`
    - `events::spawn` (3 messages) → `EditorSpawnRes.0` (SetSpawnHere, RotateSpawnDirection) + `EditorConfigRes.0.selected_block_rotation_degrees[1]` (RotatePlacementPreview)
    - `events::camera` (5 messages) → `EditorCameraRes.0`, `EditorTriggerRes.0.selected_index`, and `EditorObjectsRes` camera-trigger creation
    - Runtime tests initialize carved-out resources directly in the App world. Runtime::new(State), per-frame State synchronization, and one-time State snapshot seeding are deleted.
    - The active branch removes all `ToLegacy` impls and `LegacyCommand`; no message handler stages `PendingCommands`.

## Full-Bevy Next Roadmap

The project is now App-driven, but it is not ready for the Bevy umbrella yet.
The next work should remove the compatibility bridge first, then replace one
platform/rendering boundary at a time. Each step should keep native + wasm
green before moving on.

### 1. Finish direct App-world ownership

The duplicate `State::world` is gone, but authoritative subsystem fields still
live behind the legacy `State` object. Move those fields into the App world,
then make `State` either disappear or become a thin construction/helper layer.

- Render ownership has moved from `StateRenderResources` into App-world resources at startup; delete the empty State compatibility slot after legacy render tests migrate.
- Move session/editor/menu/audio/auth resources out of legacy subsystem fields and into App-world resources.
- Route UI, async results, and input through App-world handlers only; stop bridging through legacy dispatch.
- Delete legacy `State` accessors that only exist to bridge App-world resources back into subsystem fields.
- Keep `cargo test -p eg-state --lib` and `cargo test -p eg-app --lib runtime::` green throughout.

### 2. Replace legacy command dispatch

The message-to-legacy conversion, empty compatibility buffer, legacy dispatch
files, and direct `State::process_input_event` path are gone. Remaining work in
this area is moving the small Runtime request sync adapters (`gameplay_compat`
and `escape_compat`) into full App-world systems.

- Move the remaining Escape and session compatibility adapters into App-world systems.
- Use tests around undo/redo, selection, timeline scrub, and playtest enter/exit as the parity gate.

### 3. Promote phase handling to Bevy `States`

After legacy dispatch is gone, convert phase bookkeeping to Bevy state APIs.

- Derive or wrap `AppPhase` as a Bevy `States` type.
- Replace `AppPhaseRes.previous_app_phase` and `phase_transition_system` with `OnEnter` / `OnExit` schedules.
- Attach editor/play/menu systems to state-gated sets instead of manual phase checks where practical.
- Keep the shared object entity model: authored object components persist, while play-only runtime components are added/removed on transitions.

### 4. Move egui to `bevy_egui`

Do this before `bevy_render` if UI command flow is already direct, because it
removes the manual `EguiContextRes` coupling.

- Replace `EguiContextRes` and manual `ctx.run_ui` calls with `bevy_egui` systems.
- Delete `UiStateSnapshot` once UI reads resources/queries directly.
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
- Delete `Runtime` once no production code depends on the remaining winit/render shell.

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

The remaining gap is architectural, not just cleanup. The code still has a legacy `State` compatibility bridge:

- runtime.rs still owns `State` and `App` together, and still invokes the legacy custom scene/egui render method.
- runtime.rs still consumes remaining typed request resources by calling legacy `State` compatibility methods for side effects that have not moved into App-world systems, and uploads extracted buffers into the custom presenter.
- eg-ui still consumes the borrowed App-owned `UiStateView` adapter.
- the inline runtime UI still uses the snapshot-shaped view exposed by `UiStateRes`, but its source data, live egui context, renderer, menu textures, block-icon cache, and production command sink now live in the App world during the frame.
- application.rs still translates winit input to `InputEvent`, but those events now enter App-world `PlatformInputQueue` and are drained by scheduled App-world input handling.

So the status is: Phase 7 and Phase 8 are meaningfully improved, and the branch is now App-driven for frame/update, App-world egui context sync, queued UI command ingress, App-world platform input ingress, editor interaction, and render extraction. The custom scene/egui presenter and a small set of session/menu/external side-effect adapters still keep `State` alive, so this is not yet the single shared World rewrite described in the original plan. The next real milestone is moving that presenter onto App-owned surface and mesh resources.
