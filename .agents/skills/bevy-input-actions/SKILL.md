---
name: bevy-input-actions
description: "Use this skill when handling or abstracting player input in Bevy 0.19. Covers ButtonInput, keyboard, mouse, accumulated motion, touch, gamepad entities, semantic actions, remapping, local multiplayer, dead zones, and fixed-tick edge buffering. Triggers on: input, keyboard, mouse, gamepad, touch, action map, key binding, just_pressed, cursor, local multiplayer."
argument-hint: "Describe the devices, player actions, or remapping needs"
---

# Bevy 0.19 Input and Actions

## Quick Start

Translate raw keyboard state into a semantic component:

```rust
use bevy::prelude::*;

#[derive(Component, Default)]
struct MoveIntent(Vec2);

#[derive(Component)]
struct LocalPlayer;

fn collect_keyboard(
    keys: Res<ButtonInput<KeyCode>>,
    mut player: Single<&mut MoveIntent, With<LocalPlayer>>,
) {
    let mut direction = Vec2::ZERO;
    direction.x = keys.pressed(KeyCode::KeyD) as i8 as f32
        - keys.pressed(KeyCode::KeyA) as i8 as f32;
    direction.y = keys.pressed(KeyCode::KeyW) as i8 as f32
        - keys.pressed(KeyCode::KeyS) as i8 as f32;
    player.0 = direction.normalize_or_zero();
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Update, collect_keyboard)
        .run();
}
```

Gameplay and movement systems consume `MoveIntent`; they do not know which key or device produced it.

---

## Core Concepts

### Raw Input vs Domain Actions

Raw resources describe devices. Domain actions describe intent such as move, jump, interact, aim, or pause. Resolve bindings, sensitivity, dead zones, and player-device ownership at one input boundary.

### Bevy 0.19 Input Sources

| Input          | API                                         |
| -------------- | ------------------------------------------- |
| Keyboard       | `Res<ButtonInput<KeyCode>>`                 |
| Mouse buttons  | `Res<ButtonInput<MouseButton>>`             |
| Mouse movement | `Res<AccumulatedMouseMotion>`               |
| Mouse wheel    | Accumulated scroll resource or raw messages |
| Touch          | `Touches` and touch messages                |
| Gamepad        | Query gamepad entities/components           |
| Text/IME       | Character and IME messages                  |

Use raw messages for rebinding, text entry, and device lifecycle. Use state resources for ordinary held/edge checks.

### Continuous and Edge Actions

Continuous intent can be overwritten each frame. Edge actions such as jump must survive until the simulation consumes them.

`just_pressed` is a rendered-frame edge. `FixedUpdate` may run zero or several times that frame, so reading the edge only inside fixed simulation can drop or duplicate it.

### Mouse and Analog Scaling

Accumulated mouse motion already represents displacement over the frame; do not multiply it by delta time. Apply dead zones before normalizing analog values and preserve magnitude when gameplay uses analog strength.

---

## Common Patterns

### Fixed-Tick Action Buffer

Sample frame edges before the fixed loop, set a queued flag, consume it once in fixed simulation, and clear it after the fixed loop only when a tick ran.

```rust
#[derive(Component, Default)]
struct ActionBuffer {
    jump: bool,
}

fn buffer_jump(keys: Res<ButtonInput<KeyCode>>, mut buffer: Single<&mut ActionBuffer>) {
    buffer.jump |= keys.just_pressed(KeyCode::Space);
}
```

### Remappable Bindings

Store bindings as configuration and map several physical inputs to the same action. Keep defaults, serialization, conflict detection, and display names outside movement or combat systems.

### Local Multiplayer

Associate each local player with an assigned keyboard partition or gamepad entity. Resolve ownership before writing intent so a device cannot drive multiple actors accidentally.

### Focus and Disconnect Reset

Clear held and buffered intent when the window loses focus, cursor capture changes, or a gamepad disconnects. Otherwise actors can remain stuck moving.

### UI Action Reuse

Map keyboard, gamepad, and pointer activation to the same semantic menu command. UI logic should not duplicate gameplay key branches.

---

## Gotchas

1. **`Input<T>` is obsolete.** Bevy 0.19 uses `ButtonInput<T>`.
2. **Old key names are invalid.** Use physical variants such as `KeyCode::KeyW`, not `KeyCode::W`.
3. **Fixed ticks miss frame edges.** Buffer `just_pressed` before the fixed loop.
4. **Mouse delta is already accumulated.** Multiplying by delta time makes rotation frame-rate dependent.
5. **Normalization can erase analog magnitude.** Apply it only when constant directional speed is desired.
6. **Raw input scattered across domains prevents remapping.** Keep device interpretation in an input plugin.
7. **Focus loss can leave held intent stale.** Reset action state on lifecycle changes.
8. **Text input is not gameplay input.** Preserve IME and character events for text widgets.

## Validation Checklist

- Gameplay consumes semantic actions rather than device-specific values.
- Edge actions survive zero fixed ticks and are consumed by policy.
- Device assignment, focus loss, and disconnect behavior are defined.
- Mouse and analog scaling are frame-rate independent.
- Tests map at least two bindings to the same action.

## Related Skills

- [bevy-fixed-simulation-movement](../bevy-fixed-simulation-movement/SKILL.md) for consuming action buffers in fixed simulation.
- [bevy-ui-picking](../bevy-ui-picking/SKILL.md) for semantic UI activation and focus.
- [bevy-schedules-system-sets](../bevy-schedules-system-sets/SKILL.md) for before/after fixed-loop timing.
- [bevy-ecs-modeling](../bevy-ecs-modeling/SKILL.md) for intent components and player-device relationships.
