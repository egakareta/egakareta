---
name: bevy-fixed-simulation-movement
description: "Use this skill when implementing deterministic movement or fixed-timestep gameplay in Bevy 0.19. Covers Time<Fixed>, FixedUpdate, input accumulation, physical state, previous state, interpolation, camera timing, simulation phases, seeded RNG, and frame-pacing tests. Triggers on: fixed timestep, movement, physics, interpolation, jitter, determinism, rollback, FixedUpdate, camera follow."
argument-hint: "Describe the simulation, movement, or jitter problem"
---

# Bevy 0.19 Fixed Simulation and Movement

## Quick Start

Keep authoritative position separate from the rendered transform:

```rust
use bevy::prelude::*;

#[derive(Component, Clone, Copy, Deref, DerefMut)]
struct PhysicalPosition(Vec3);

#[derive(Component, Clone, Copy, Deref, DerefMut)]
struct PreviousPosition(Vec3);

#[derive(Component, Clone, Copy, Deref, DerefMut)]
struct Velocity(Vec3);

fn simulate(
    time: Res<Time<Fixed>>,
    mut query: Query<(&mut PreviousPosition, &mut PhysicalPosition, &Velocity)>,
) {
    for (mut previous, mut position, velocity) in &mut query {
        **previous = **position;
        **position += **velocity * time.delta_secs();
    }
}

fn interpolate(
    fixed: Res<Time<Fixed>>,
    mut query: Query<(&PreviousPosition, &PhysicalPosition, &mut Transform)>,
) {
    let alpha = fixed.overstep_fraction();
    for (previous, position, mut transform) in &mut query {
        transform.translation = previous.lerp(**position, alpha);
    }
}
```

Register `simulate` in `FixedUpdate` and `interpolate` in `RunFixedMainLoopSystems::AfterFixedMainLoop`.

---

## Core Concepts

### Fixed Simulation

Configure `Time<Fixed>` from the stability and responsiveness needs of gameplay. Bevy may run zero, one, or several fixed ticks per rendered frame to catch up.

Use fixed time for authoritative movement, collision, simulation timers, and physics-dependent gameplay.

### Simulation vs Presentation

Authoritative physical state advances on fixed ticks. `Transform` can be an interpolated visual projection. Never feed the interpolated value back into simulation.

Keep previous and current states so presentation can interpolate with the fixed overstep fraction.

### Input Timing

Collect continuous and edge input before the fixed loop. Consume it during fixed simulation. Clear consumed edges after the loop according to whether a tick actually ran.

### Camera Timing

Update camera rotation before fixed simulation when movement depends on facing. Update follow translation after interpolation. Cameras updated only on fixed ticks visibly stutter.

### Determinism

Use seeded RNG resources, controlled input, fixed time, and stable iteration when draw order matters. Avoid wall-clock time, thread-local randomness, renderer visibility, and unordered external data in authoritative logic.

---

## Common Patterns

### Simulation Phase Sets

Use a fixed pipeline such as:

`CollectCommands -> ChooseActions -> Integrate -> Detect -> Resolve -> Finalize`

Order only phases that exchange same-tick results.

### Frame-Rate-Independent Velocity

Express velocity in units per second and multiply by the schedule's delta. Normalize directional input only when diagonal and axial speed should match.

### Grid and Render Coordinates

Keep logical tile, navigation, or physics coordinates separate from pixel-snapped or interpolated transforms. Synchronize presentation after authoritative updates.

### Catch-Up Policy

Define what happens when simulation falls far behind: continue catching up, cap ticks, pause, or disconnect. Do not hide the problem in a custom accumulator that silently discards time.

### Frame-Pacing Test

Feed identical inputs through different rendered-frame delta sequences and assert identical final simulation state after the same fixed ticks.

---

## Gotchas

1. **`FixedUpdate` can run zero or many times per frame.** Never assume one fixed tick per render.
2. **Frame edges can be lost or repeated.** Buffer them outside fixed simulation.
3. **Interpolated transforms lag by design.** This trades one tick of visual latency for smoothness.
4. **Camera updates on fixed ticks stutter.** Present cameras every rendered frame.
5. **Variable delta destabilizes collision.** Keep physics and dependent resolution on the same fixed cadence.
6. **Custom accumulators duplicate Bevy.** Use fixed schedules unless a proven requirement cannot fit them.
7. **Random draw order can break determinism.** Control iteration when RNG consumption depends on entity order.
8. **Presentation state must not become authoritative.** Do not simulate from interpolated `Transform`.

## Validation Checklist

- Simulation advances only from fixed delta and controlled input.
- Input edges are consumed exactly according to policy.
- Previous/current physical state is distinct from visual transforms.
- Camera timing follows its actual dependencies.
- Catch-up and overload behavior are explicit.
- Frame-pacing tests compare authoritative final state.

## Related Skills

- [bevy-input-actions](../bevy-input-actions/SKILL.md) for semantic input and edge buffering.
- [bevy-schedules-system-sets](../bevy-schedules-system-sets/SKILL.md) for fixed phase sets and loop boundaries.
- [bevy-ecs-modeling](../bevy-ecs-modeling/SKILL.md) for physical-state components.
- [bevy-testing-diagnostics-performance](../bevy-testing-diagnostics-performance/SKILL.md) for deterministic fixtures and timing tests.
