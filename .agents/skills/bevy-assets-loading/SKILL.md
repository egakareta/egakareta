---
name: bevy-assets-loading
description: "Use this skill when loading, retaining, validating, or hot-reloading assets in Bevy 0.19. Covers AssetServer, strong handles, Assets collections, custom Asset and AssetLoader types, loading states, load barriers, AssetEvent messages, dependencies, and runtime manifests. Triggers on: asset, Handle, AssetServer, loader, loading screen, hot reload, manifest, texture, model, audio file."
argument-hint: "Describe the asset type, loading flow, or failure"
---

# Bevy 0.19 Assets and Loading

## Quick Start

Load assets asynchronously and retain strong handles in an owning resource:

```rust
use bevy::prelude::*;

#[derive(Resource)]
struct GameAssets {
    player: Handle<Image>,
}

fn load_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(GameAssets {
        player: asset_server.load("textures/player.png"),
    });
}

fn spawn_player(mut commands: Commands, assets: Res<GameAssets>) {
    commands.spawn(Sprite::from_image(assets.player.clone()));
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, load_assets)
        .add_systems(Update, spawn_player.run_if(resource_added::<GameAssets>))
        .run();
}
```

`AssetServer::load` returns immediately. The handle identifies a value that may still be loading; retain it for as long as the asset must remain resident.

---

## Core Concepts

### Handles and Asset Storage

`Handle<T>` refers to an asset stored in `Assets<T>`. Components and resources usually store handles, not copied asset payloads.

- Strong handles keep assets alive.
- Dropping the final strong handle allows unloading.
- `Assets<T>::get` may return `None` while loading or after unloading.
- Mutating an asset changes every entity sharing that handle.

Clone the asset into a new handle before per-instance mutation.

### Custom Assets and Loaders

A custom asset derives `Asset` and `TypePath`. Its `AssetLoader` defines associated asset, settings, and error types, then asynchronously reads from `Reader`:

```rust
#[derive(Asset, TypePath)]
struct LevelData {
    spawn_count: u32,
}

// Register in the owning plugin:
// app.init_asset::<LevelData>()
//    .init_asset_loader::<LevelLoader>();
```

Use structured parsers such as serde formats and return errors with path and parse context.

### Dependencies

Declare nested loads through `LoadContext`. This lets Bevy track dependencies, readiness, and hot reload correctly. Do not hide dependent paths in an untracked side channel.

### Loading Groups

For a set of loads, create them through `asset_server.load_builder().with_guard(...)`. Transition out of loading only after the barrier completes or explicit failure policy resolves.

### Asset Events

`AssetEvent<T>` is a buffered message in 0.19. React to added, modified, removed, and unused assets to rebuild only affected derived data.

---

## Common Patterns

### Loading State

Enter a loading state, start all tracked loads, retain their handles, and show a state-scoped loading screen. On success, build validated runtime data and request gameplay state. On failure, surface the path and cause instead of waiting forever.

### Raw Data to Runtime Manifest

Deserialize authored names and loose references into a raw asset. Validate once into a runtime manifest with typed IDs, resolved cross-references, defaults, and invariant checks. Gameplay consumes the validated manifest rather than parsing strings repeatedly.

### Shared Asset Catalog

Store frequently reused handles in a resource:

```rust
#[derive(Resource)]
struct AudioAssets {
    jump: Handle<AudioSource>,
    hit: Handle<AudioSource>,
}
```

Presentation systems clone handles from the catalog when spawning players or materials.

### Hot-Reload Derived Data

Read `MessageReader<AssetEvent<MyAsset>>`, locate modified IDs, and rebuild only caches derived from those assets. Preserve the last valid runtime value when a reload fails, if product policy allows it.

### Generated Assets

Insert runtime-generated meshes, images, and materials through `Assets<T>::add`. Retain the returned handle exactly like a file-backed asset.

---

## Gotchas

1. **Loading is asynchronous.** Startup-system ordering does not make the value available immediately.
2. **Handles own lifetime.** Discarding the final strong handle can unload the asset.
3. **Shared mutation is global.** Editing one `Assets<T>` entry affects every user of that handle.
4. **Weak handles are not ownership.** Do not poll weak clones while expecting them to keep data resident.
5. **Failed loads need policy.** A loading screen must not wait forever after `Failed`.
6. **Dependencies must be declared.** Hidden nested loads break readiness and hot reload behavior.
7. **Asset events are messages in 0.19.** Do not use legacy `EventReader<AssetEvent<T>>`.
8. **Authored data may be invalid.** Validate references and ranges before simulation consumes them.
9. **Paths in recurring systems waste work.** Load once and retain typed handles.

## Validation Checklist

- Every load has a strong owner and failure policy.
- Group readiness tracks every dependent load.
- Custom loader errors preserve path and parse context.
- Raw authored data is validated into runtime-safe structures.
- Shared versus per-instance mutation is intentional.
- Valid, invalid, missing, and hot-reloaded assets have focused tests.

## Related Skills

- [bevy-states-scoped-entities](../bevy-states-scoped-entities/SKILL.md) for loading-state transitions and cleanup.
- [bevy-scenes-reflection](../bevy-scenes-reflection/SKILL.md) for scene and glTF assets.
- [bevy-audio](../bevy-audio/SKILL.md) for retained audio handles and playback entities.
- [bevy-async-tasks](../bevy-async-tasks/SKILL.md) for non-asset background computations.
