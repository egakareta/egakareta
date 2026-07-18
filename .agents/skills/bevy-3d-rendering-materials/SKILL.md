---
name: bevy-3d-rendering-materials
description: "Use this skill when building or optimizing a Bevy 0.19 3D scene. Covers Mesh3d, MeshMaterial3d, StandardMaterial, cameras, PBR lights, shadows, environment maps, exposure, visibility, Material, MaterialExtension, glTF presentation, and asset sharing. Triggers on: 3D, PBR, mesh, material, light, shadow, camera, environment map, custom material, glTF rendering."
argument-hint: "Describe the 3D scene, material, or lighting goal"
---

# Bevy 0.19 3D Rendering and Materials

## Quick Start

Create a lit PBR scene with shared mesh and material assets:

```rust
use bevy::prelude::*;

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mesh = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let material = materials.add(Color::srgb(0.2, 0.7, 0.9));

    commands.spawn((Mesh3d(mesh), MeshMaterial3d(material)));
    commands.spawn((PointLight::default(), Transform::from_xyz(4.0, 8.0, 4.0)));
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(3.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}
```

Bevy 0.19 uses rendering components directly; `PbrBundle` is obsolete.

---

## Core Concepts

### PBR Components

`Mesh3d` selects geometry and `MeshMaterial3d<StandardMaterial>` selects surface properties. `Transform`, visibility, cameras, and lights are separate components.

### StandardMaterial

Configure base color, metallic, perceptual roughness, emissive, normal maps, alpha mode, culling, and texture maps deliberately. Keep physical scale, light units, exposure, and tonemapping coherent.

### Asset Sharing

Meshes and materials are assets. Clone handles to share them. Mutating `Assets<StandardMaterial>` changes all entities using that handle; create a new material for independent variation.

### Lighting Pipeline

Choose directional, point, spot, area, environment, and ambient lighting from scene needs. Shadows, transparency, and post effects have significant cost and platform constraints.

### Escalation Order

1. Built-in camera, light, and `StandardMaterial` components.
2. Existing post-process and rendering feature components.
3. `MaterialExtension` or custom `Material`.
4. Render sub-app only for requirements earlier hooks cannot express.

---

## Common Patterns

### Shared Prototype Assets

Create common meshes and materials once in setup or an asset resource, then clone handles for spawned entities.

### Per-Instance Material Variation

Clone the material value and add it back to `Assets<StandardMaterial>` before modifying one instance. Cloning only the handle still references the shared asset.

### Authored Scene Plus Gameplay

Load complex glTF hierarchy through `WorldAssetRoot`. After instantiation, attach typed gameplay identity, collision roles, and animation controllers to descendants.

### Quality Settings

Centralize shadow quality, anti-aliasing, visibility range, post effects, and environment resolution in a presentation configuration resource or plugin fields.

### Separate Collision Geometry

Use simplified collision or navigation meshes when render meshes are too detailed or update at a different cadence.

---

## Gotchas

1. **`PbrBundle` is obsolete.** Spawn `Mesh3d`, `MeshMaterial3d`, and related components directly.
2. **Cloning a handle does not clone an asset.** Shared mutation remains global.
3. **Old light values do not transfer blindly.** Bevy 0.19 uses HDR and physical-ish units.
4. **Transparency and shadows are expensive.** Apply them with a quality and platform policy.
5. **Material variants can multiply pipelines.** Measure specialization and batching cost.
6. **Gameplay identity does not belong in material assets.** Use typed components.
7. **Imported scene descendants arrive asynchronously.** Post-process after readiness.
8. **Custom pipelines are a last resort.** Prefer material hooks.

## Validation Checklist

- Physical scale, camera, lighting, exposure, and tonemapping agree.
- Mesh and material sharing is intentional.
- Per-instance asset mutation uses independent assets.
- Quality features have performance and platform policy.
- The simplest sufficient material extension point is used.
- Representative scenes are measured on target hardware.

## Related Skills

- [bevy-scenes-reflection](../bevy-scenes-reflection/SKILL.md) for glTF hierarchy and post-processing.
- [bevy-assets-loading](../bevy-assets-loading/SKILL.md) for mesh, texture, and material lifetime.
- [bevy-render-extensions](../bevy-render-extensions/SKILL.md) for custom materials and render passes.
- [bevy-animation](../bevy-animation/SKILL.md) for animated scene descendants.
