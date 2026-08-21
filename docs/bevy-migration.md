# Bevy ECS Architecture

This document describes the current architecture. The project uses Bevy ECS
and Bevy App for authoritative state and scheduling, while presentation stays
hand-rolled with wgpu, egui, and winit.

Full-Bevy rendering is optional future work. It is not a migration prerequisite
and must not be used as a reason to reintroduce a parallel state machine.

## Non-Negotiable Rules

- `eg-game` is headless and owns gameplay components, resources, messages, and systems.
- `eg-state` owns App-world editor, session, menu, input, async, and phase resources.
- `eg-app` owns platform ingress and drives the Bevy `App`; it does not own gameplay state.
- The custom renderer reads App-world components/resources during scheduled extraction.
- Persistent facts live in components or resources, not in transient messages.
- Commands are semantic Bevy messages; handlers mutate the App world or enqueue a typed external-service request.
- No production hand-rolled `State` owner, `State::update`, legacy dispatch path, UI snapshot aggregate, or renderer dirty-bit resource is allowed.
- Render extraction derives its inputs from current ECS data and uses focused caches only for performance.
- `Changed<T>`, `Added<T>`, queries, and resource change detection are preferred over manual invalidation flags.

## Ownership

| Domain                                     | Authoritative owner                                                       | Presentation boundary                                      |
| ------------------------------------------ | ------------------------------------------------------------------------- | ---------------------------------------------------------- |
| Player                                     | `Player` entity and focused player components in `eg-game`                | Render extraction queries player components                |
| Level objects during gameplay              | `ObjectBundle` entities and `ObjectData`/projection components            | Gameplay mesh extraction queries gameplay objects          |
| Authored editor objects                    | `EditorAuthoredObject` entities with `EditorObjectOrder`                  | Editor extraction queries and sorts by authored order      |
| Gameplay clock/progress                    | `GameClock`, `PlayerProgress`, `PendingTurnInputs`, and related resources | UI/render systems read resources                           |
| Editor input/config/camera/timeline/timing | Focused `Editor*Res` resources                                            | UI and extraction systems read resources                   |
| Editor interaction                         | `EditorRuntimeRes` plus `EditorHistoryRes`                                | Interaction systems mutate resources and authored entities |
| Phase                                      | Bevy `State<AppPhase>` and `NextState<AppPhase>`                          | Runtime and extraction read Bevy state                     |
| Async results                              | Typed result messages and channel resources                               | App schedule applies results                               |
| GPU meshes/uniforms                        | `Render*Res` App resources                                                | Custom wgpu renderer presents them                         |

`LevelMeta.initial_objects` and transition seed values are level/bootstrap data
used to spawn or reset the ECS world. They are not a second gameplay object
authority and are never updated by gameplay consumption. Once spawned,
gameplay systems query object entities.

## Schedule Graph

The application uses standard Bevy schedules plus two focused custom schedules:

```text
App Update
  platform input ingress
  egui UI and semantic message production

PostUpdate
  StateSchedule
    input bridge
    command handlers
    async result application
    editor timeline/session systems
    gameplay reset/transition requests
  FixedUpdate
    GameplaySet::TurnInput
    GameplaySet::AdvancePlayer
    GameplaySet::Collision
    GameplaySet::ConsumeObjects
    GameplaySet::GroundGravity
    GameplaySet::Trail
    GameplaySet::Completion
    GameplaySet::Progress
  RenderExtractSchedule
    ECS/resource queries -> CPU mesh and uniform extraction resources
    custom renderer upload/present boundary
```

Gameplay systems are registered by `eg_game::GamePlugin` and can run in a
minimal headless App. Editor and platform systems live outside `eg-game`.

## Gameplay ECS

`eg-game` contains no wgpu, egui, winit, audio device, or file I/O dependency.
The player is represented by focused components such as `PlayerPosition`,
`PlayerDirection`, `PlayerSpeed`, `Grounded`, `VerticalVelocity`, and `Trail`.
Each authored object is an entity with `ObjectData` plus projection components
for position, rotation, size, block identity, and tint. `GameplaySet` expresses
the causal order of movement, collision, consumption, gravity, trail, and
completion systems.

Timeline preview uses the same ECS components and gameplay systems in an
isolated `World` and `FixedUpdate` schedule. It exposes a small value snapshot
to the editor because a preview API needs a value result; it does not duplicate
the production gameplay world.

## Editor ECS

Editor mutations enter through typed `Message` values. Systems query authored
entities in stable `EditorObjectOrder`, mutate focused resources, and use
`Commands` for structural replacement. Undo/redo is owned by
`EditorHistoryRes`; it is not embedded in the interaction runtime.

Mesh extraction runs from the current entity/resource state. A changed authored
object is visible because the query reads the changed component, not because a
producer remembered to set a renderer flag. Static mesh caches compare their
actual ECS-derived inputs and are performance caches, never authority.

## Custom Renderer Boundary

The renderer remains deliberately outside Bevy's render world:

- `RenderExtractSchedule` builds CPU geometry and POD uniforms from ECS data.
- `RenderMeshesRes`, `RenderUniformsRes`, and related resources hold the data needed for presentation.
- `RenderSet::Present` calls the custom wgpu/egui presenter.
- GPU handles are App-owned resources and are never stored in gameplay components.

This keeps the simulation headless while allowing the renderer to be replaced or
optimized independently later.

## Future-Bevy Refactor Points

These are optional platform replacements only. The comments in source are the
canonical pointers; remove each comment and row together if the replacement is
implemented.

| Source                                          | Current boundary                             | Possible future replacement      |
| ----------------------------------------------- | -------------------------------------------- | -------------------------------- |
| `crates/eg-app/src/platform/application.rs`     | Custom winit `ApplicationHandler`            | `bevy_winit` app runner          |
| `crates/eg-app/src/platform/input_mapping.rs`   | winit input mapped to project input messages | `bevy_input` action systems      |
| `crates/eg-app/src/platform/runtime.rs`         | Custom App bootstrap and egui frame shell    | Bevy app/window integration      |
| `crates/eg-ui/src/plugin.rs`                    | Manual egui presentation and command sink    | `bevy_egui` integration          |
| `crates/eg-state/src/state/lifecycle.rs`        | Custom wgpu/platform bootstrap               | Bevy asset/window/render plugins |
| `crates/eg-state/src/state/render/plugin.rs`    | Custom extraction/present schedule           | Bevy render-world extraction     |
| `crates/eg-state/src/state/render/resources.rs` | Custom GPU and mesh resources                | Bevy render resources/assets     |
| `crates/eg-state/src/state/audio_state.rs`      | Custom audio backend resources               | Bevy audio resources             |
| `crates/eg-state/src/state/async_bridge.rs`     | External mpsc receiver polling               | Bevy task/asset event streams    |
| `crates/eg-state/src/events/async_results.rs`   | Typed async result messages                  | Bevy task/asset messages         |
| `crates/eg-state/src/events/mod.rs`             | Project input/message taxonomy               | Bevy input/action sources        |
| `crates/eg-state/src/state/plugins.rs`          | Project domain plugin composition            | Standard Bevy plugin graph       |
| `crates/eg-state/src/state/test_support.rs`     | Minimal App test harness                     | Standard Bevy test fixtures      |
| `crates/eg-game/src/events.rs`                  | Gameplay consumed-event buffer               | Normal Bevy gameplay messages    |

## Verification

The architecture is validated by:

- headless `eg-game` App/world tests for gameplay systems;
- `eg-state` schedule tests asserting resources and authored entities;
- render extraction tests that provide minimal App worlds without a GPU;
- workspace clippy with `-D warnings`;
- native and wasm development builds.

When adding a feature, first decide whether it is an entity component, a unique
resource, a semantic message, or an external-service request. Do not add a
field to a global state aggregate or a renderer dirty flag to avoid making that
decision.
