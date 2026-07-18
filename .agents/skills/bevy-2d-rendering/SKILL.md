---
name: bevy-2d-rendering
description: "Use this skill when building or optimizing 2D presentation in Bevy 0.19. Covers Sprite, texture atlases, Mesh2d, MeshMaterial2d, Material2d, TilemapChunk, pixel-art sampling, 2D cameras, z ordering, render layers, handle reuse, and batching. Triggers on: sprite, sprite sheet, tilemap, 2D mesh, pixel art, parallax, 2D shader, draw order, batching."
argument-hint: "Describe the 2D visuals, camera, or performance target"
---

# Bevy 0.19 2D Rendering

## Quick Start

Spawn a camera and image-backed sprite with direct components:

```rust
use bevy::prelude::*;

fn setup(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(Camera2d);
    commands.spawn((
        Sprite::from_image(asset_server.load("textures/player.png")),
        Transform::from_xyz(0.0, 0.0, 1.0),
    ));
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .add_systems(Startup, setup)
        .run();
}
```

Use nearest sampling for pixel art deliberately; use the default sampler for smoothly filtered art.

---

## Core Concepts

### Choose the Representation

| Need                    | Start With                                    |
| ----------------------- | --------------------------------------------- |
| Textured quad           | `Sprite`                                      |
| Sprite sheet            | `Sprite` with texture atlas data              |
| Geometric shape         | `Mesh2d` plus `MeshMaterial2d<ColorMaterial>` |
| Custom surface shader   | `Material2d`                                  |
| Dense regular tile data | `TilemapChunk`                                |
| Debug shape             | `Gizmos`                                      |

### Direct Components

Bevy 0.19 uses `Sprite`, `Mesh2d`, and `MeshMaterial2d` directly. Old `SpriteBundle` and `MaterialMesh2dBundle` APIs are obsolete.

### Logical and Visual State

Keep logical grid coordinates, collision data, and animation state separate from pixel-snapped or interpolated transforms. Rendering projects authoritative state; it does not own it.

### Shared Assets

Reuse image, mesh, and material handles. Mutating one asset changes every entity sharing the handle. Clone the asset into a new handle when one instance must diverge.

### Ordering

Use z translation, camera order, and render layers intentionally. Spawn order is not a robust draw-order contract.

---

## Common Patterns

### Shared Mesh and Material

Create geometry once and clone handles when spawning many entities:

```rust
let mesh = meshes.add(Rectangle::new(32.0, 32.0));
let material = materials.add(Color::srgb(0.2, 0.8, 0.4));

commands.spawn((Mesh2d(mesh.clone()), MeshMaterial2d(material.clone())));
```

### Sprite Animation

Store frame index and timer in focused components. Update atlas selection when the timer finishes instead of rebuilding the entity.

### Dense Tile Worlds

Use `TilemapChunk` when many static or batch-updated tiles share one chunk. Keep gameplay tile data independent so rendering can rebuild chunks without changing simulation.

### Pixel-Perfect Presentation

Use nearest sampling, an appropriate camera scale, and pixel-grid snapping. Snap visual transforms only; preserve higher-precision simulation coordinates.

### Visibility Instead of Churn

For frequently toggled visuals, update visibility or culling-related components rather than despawning and recreating identical entities.

---

## Gotchas

1. **Old bundles are invalid.** Do not use `SpriteBundle` or `MaterialMesh2dBundle`.
2. **Shared material mutation is global.** Clone to a new asset for per-instance changes.
3. **One entity per tile can be expensive.** Use chunking when the update model fits.
4. **Spawn order is not draw order.** Set z, camera order, and layers explicitly.
5. **Pixel snapping is presentation.** Do not feed snapped transforms into simulation.
6. **Unique identical assets waste memory and batching.** Reuse handles.
7. **A custom render pipeline is usually unnecessary.** Try `Material2d` first.
8. **Sampling changes the art style.** Configure it intentionally.

## Validation Checklist

- The simplest suitable representation is used.
- Logical state is independent of atlas layout and render transforms.
- Shared assets and per-instance mutations are intentional.
- Sampling, z ordering, camera order, and layers match the design.
- Dense scenes are tested at representative entity and tile counts.

## Related Skills

- [bevy-assets-loading](../bevy-assets-loading/SKILL.md) for images and retained handles.
- [bevy-render-extensions](../bevy-render-extensions/SKILL.md) for custom `Material2d` and lower-level GPU work.
- [bevy-fixed-simulation-movement](../bevy-fixed-simulation-movement/SKILL.md) for interpolated visual transforms.
- [bevy-testing-diagnostics-performance](../bevy-testing-diagnostics-performance/SKILL.md) for representative stress measurements.
