---
name: bevy-render-extensions
description: "Use this skill when extending Bevy 0.19 rendering beyond built-in components. Covers WGSL, Material2d, Material, MaterialExtension, AsBindGroup, RenderApp, RenderStartup, extraction, GPU preparation, specialized pipelines, render phases, graph nodes, compute, and readback. Triggers on: shader, custom material, render pipeline, RenderApp, render graph, compute shader, GPU buffer, post-process."
argument-hint: "Describe the custom GPU effect or render-pipeline requirement"
---

# Bevy 0.19 Render Extensions

## Quick Start

Start with a custom material before entering the render sub-app:

```rust
use bevy::{
    prelude::*,
    render::render_resource::AsBindGroup,
    shader::ShaderRef,
};

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
struct ToonMaterial {
    #[uniform(0)]
    color: LinearRgba,
}

impl Material for ToonMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/toon.wgsl".into()
    }
}

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, MaterialPlugin::<ToonMaterial>::default()))
        .run();
}
```

Use `Material2d` for 2D surfaces and `MaterialExtension` when extending standard PBR is preferable to replacing it.

---

## Core Concepts

### Escalation Gate

1. Built-in camera, light, post-process, visibility, and material components.
2. `Material2d`, `Material`, or `MaterialExtension`.
3. Existing render-asset or extraction patterns.
4. `RenderApp`, phases, or render graph only for custom passes, queueing, compute, or readback.

### Main World and Render World

Authoritative state lives in the main world. Extract the smallest render snapshot into the render world. Ordinary render systems must not borrow main-world resources directly.

### Render Data Flow

`Main World -> Extract -> Prepare/Specialize -> Queue -> Render Phase/Graph -> Cleanup`

Each boundary has distinct ownership and timing. Queueing assumes required views, assets, buffers, and pipelines are prepared.

### GPU Resource Lifecycle

Initialize device-dependent resources in `RenderStartup` so render-device recovery can recreate them. Cache pipelines through Bevy's pipeline cache and rebuild bind groups only when dependencies change.

### Rust and WGSL Layout

Rust shader types, bind-group layouts, binding indices, shader visibility, texture formats, and WGSL declarations must agree exactly.

---

## Common Patterns

### Material Extension

Use `MaterialExtension` when custom fragment behavior should retain standard PBR lighting, shadows, and mesh pipeline behavior.

### Extracted Component

Keep a compact main-world component, extract it on change, prepare GPU data in the render world, and avoid cloning large gameplay structures every frame.

### Specialized Pipeline

Define a compact specialization key, request the pipeline from the cache, and skip queueing safely while asynchronous compilation is incomplete.

### Post-Process Node

Extract camera settings, prepare a bind group from view textures, add a render-graph node with explicit edges, and account for texture format, sample count, and resize.

### Compute and Readback

Declare buffer usages for every producer and consumer. Treat GPU completion and map/readback as asynchronous; never block the frame waiting for results.

---

## Gotchas

1. **Custom pipelines are often unnecessary.** Try built-ins and material hooks first.
2. **Main and render worlds are separate.** Extract data explicitly.
3. **Pipelines compile asynchronously.** Skip work safely until ready.
4. **Per-frame resource creation is expensive.** Cache pipelines, buffers, and bind groups.
5. **Queueing before preparation fails.** Respect `RenderSystems` ordering.
6. **Graph nodes need explicit edges.** Compatible attachments alone do not establish order.
7. **GPU resources must survive recovery and resize.** Initialize and recreate them through the correct lifecycle.
8. **WGSL layout mismatches fail at runtime.** Keep bindings and host types synchronized.
9. **Readback is asynchronous and costly.** Budget copies and mapping.

## Validation Checklist

- Built-in features and material hooks were considered first.
- Extracted data is minimal and world ownership is clear.
- Pipeline and bind-group creation is cached and change-driven.
- Resize, format, sample count, and device recovery are handled.
- Pipeline-not-ready behavior skips safely.
- The effect is compiled and visually verified on supported backends.

## Related Skills

- [bevy-2d-rendering](../bevy-2d-rendering/SKILL.md) for `Material2d` use and 2D presentation.
- [bevy-3d-rendering-materials](../bevy-3d-rendering-materials/SKILL.md) for `Material`, `MaterialExtension`, and PBR.
- [bevy-assets-loading](../bevy-assets-loading/SKILL.md) for shader and render-asset lifetime.
- [bevy-testing-diagnostics-performance](../bevy-testing-diagnostics-performance/SKILL.md) for profiling and representative validation.
