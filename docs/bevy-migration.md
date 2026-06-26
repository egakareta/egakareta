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
- [ ] **Phase 1** — `eg-game` ECS core (components, resources, query-based systems, `GamePlugin`).
- [ ] **Phase 2** — `eg-state` ECS shell (single shared `World`, plugins, `AppPhase` `States`).
- [ ] **Phase 3** — command/event fragmentation.
- [ ] **Phase 4** — rendering extraction plugin (keep custom wgpu).
- [ ] **Phase 5** — UI as Bevy systems.
- [ ] **Phase 6** — platform/audio/auth async bridging.
- [ ] **Phase 7** — `App` wiring (winit drives `app.update()`).
- [ ] **Phase 8** — test harness migration (`TestApp` builder).
- [ ] **Phase 9** — documentation pass.
