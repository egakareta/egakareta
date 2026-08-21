---
name: bevy-scenes-reflection
description: "Use this skill when loading authored scenes, glTF models, reflected data, or serialized worlds in Bevy 0.19. Covers WorldAssetRoot, GltfAssetLabel, asynchronous scene instantiation, WorldInstanceReady, hierarchy post-processing, Reflect registration, DynamicWorld, entity mapping, and stable IDs. Triggers on: scene, glTF, prefab, reflection, serialization, save game, WorldAssetRoot, DynamicWorld."
argument-hint: "Describe the scene, serialization, or reflection workflow"
---

# Bevy 0.19 Scenes and Reflection

## Quick Start

Spawn a typed glTF scene label with the Bevy 0.19 scene root API:

```rust
use bevy::prelude::*;

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera3d::default());
    commands.spawn(WorldAssetRoot(asset_server.load(
        GltfAssetLabel::Scene(0).from_asset("models/hero.glb"),
    )));
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .run();
}
```

The root is inserted immediately, but scene descendants are instantiated asynchronously. Post-process gameplay components only after the instance is ready.

---

## Core Concepts

### Scene Root and Instance

`WorldAssetRoot` requests scene instantiation. The authored scene may create a hierarchy of entities over later world updates. Do not query expected descendants in the same system that spawns the root.

Use scene-ready notifications or a focused post-processing system to attach colliders, gameplay identity, animation controllers, and navigation data.

The readiness event is exported from `bevy::world_serialization`, rather than the general prelude:

```rust
use bevy::{prelude::*, world_serialization::WorldInstanceReady};

fn on_scene_ready(event: On<WorldInstanceReady>) {
    info!("scene root {:?} is ready", event.entity);
}
```

### Typed glTF Labels

Select glTF sub-assets with `GltfAssetLabel`:

| Label                           | Use                       |
| ------------------------------- | ------------------------- |
| `Scene(index)`                  | Authored entity hierarchy |
| `Mesh(index)`                   | Mesh asset                |
| `Primitive { mesh, primitive }` | Individual primitive      |
| `Animation(index)`              | Imported animation clip   |

Typed labels are safer than hand-built string fragments and make intent visible.

### Reflection Boundary

Reflection supports inspection, editor tooling, dynamic scenes, and serialization. Register only types that cross this boundary:

```rust
#[derive(Component, Reflect, Default)]
#[reflect(Component, Default)]
struct Health {
    current: u32,
}

// Registration:
// app.register_type::<Health>();
```

Keep ordinary gameplay systems statically typed. Reflection is not a replacement for normal Rust APIs.

### Dynamic Scenes and Entity Mapping

`DynamicWorld` stores reflected ECS data. Runtime `Entity` values are world-local, so serialized entity references require mapping when written and instantiated. Use stable authored IDs to reconnect domain references across sessions.

---

## Common Patterns

### Scene Post-Processing

Observe scene readiness, traverse the spawned hierarchy, and attach domain components based on stable metadata. Keep this logic in the owning gameplay or presentation plugin rather than mutating imported material data indiscriminately.

### Authored Visuals, Typed Gameplay

Let the scene own transforms, meshes, materials, lights, and named hierarchy. Let typed components own health, faction, interaction, collision roles, and simulation state.

### Animation Controller Discovery

Search descendants for `AnimationPlayer`; imported players are not guaranteed to live on the root. Attach the graph handle and controller after instantiation.

### Saveable and Runtime Components

Register and serialize durable gameplay data. Exclude task handles, GPU resources, caches, transient messages, and ephemeral entity IDs. Rebuild derived runtime data after loading.

### Typed Configuration Instead of Reflection

Use a custom asset and validator when a schema needs strong cross-reference checks, migrations, or precise errors. Use reflection when dynamic inspection and generic scene tooling are the main benefits.

---

## Gotchas

1. **Scene instantiation is asynchronous.** Descendants do not necessarily exist in the spawn tick.
2. **`SceneBundle` is obsolete.** Bevy 0.19 uses `WorldAssetRoot` for scene assets.
3. **The root may not own gameplay-relevant components.** Search or observe instantiated descendants.
4. **Runtime entity IDs are not stable save IDs.** Map them or reconnect through authored identifiers.
5. **Shared scene assets are shared.** Per-instance mutations must target instantiated entities or cloned assets intentionally.
6. **Reflection needs registration.** Unregistered component types cannot participate in generic reflected workflows.
7. **Reflecting everything increases ambiguity and maintenance.** Register only boundary types.
8. **Imported node names are authored metadata, not sufficient identity.** Convert them into typed IDs/components at the boundary.
9. **Runtime-only values should not be serialized.** Rebuild caches, tasks, and GPU state after load.

## Validation Checklist

- Scene readiness and post-processing timing are explicit.
- Typed glTF labels select the intended sub-assets.
- Gameplay identity survives re-instantiation without raw entity IDs.
- Reflected types are intentionally registered.
- Runtime-only components are excluded or reconstructed.
- A focused scene load or serialization round trip verifies the contract.

## Related Skills

- [bevy-assets-loading](../bevy-assets-loading/SKILL.md) for handle lifetime and loading barriers.
- [bevy-animation](../bevy-animation/SKILL.md) for imported clips and `AnimationPlayer` setup.
- [bevy-3d-rendering-materials](../bevy-3d-rendering-materials/SKILL.md) for meshes, materials, lights, and cameras.
- [bevy-ecs-modeling](../bevy-ecs-modeling/SKILL.md) for stable identity and typed gameplay components.
