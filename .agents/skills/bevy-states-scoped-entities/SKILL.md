---
name: bevy-states-scoped-entities
description: "Use this skill when modeling Bevy 0.19 app modes and state-owned entity lifecycles. Covers States, NextState, OnEnter, OnExit, in_state, SubStates, ComputedStates, DespawnOnExit, DespawnOnEnter, and DespawnWhen. Triggers on: game state, menu state, loading, pause, level transition, state cleanup, scoped entities, SubStates, ComputedStates."
argument-hint: "Describe the app modes, transitions, or cleanup lifecycle"
---

# Bevy 0.19 States and Scoped Entities

## Quick Start

Create coarse app modes, gate gameplay, and clean up menu entities automatically:

```rust
use bevy::prelude::*;

#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum AppState {
    #[default]
    Menu,
    Playing,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .init_state::<AppState>()
        .add_systems(OnEnter(AppState::Menu), spawn_menu)
        .add_systems(Update, play_game.run_if(in_state(AppState::Playing)))
        .run();
}

fn spawn_menu(mut commands: Commands) {
    commands.spawn((
        DespawnOnExit(AppState::Menu),
        Node::default(),
        children![Text::new("Main Menu")],
    ));
}

fn start_game(mut next_state: ResMut<NextState<AppState>>) {
    next_state.set(AppState::Playing);
}

fn play_game() {}
```

`NextState` requests a transition. The state machinery applies it at the transition boundary, runs lifecycle schedules, and removes scoped entities.

---

## Core Concepts

### Global State vs Per-Entity State

Use `States` when a mode changes global scheduling or lifecycle: loading, menu, gameplay, pause, results, or editor mode.

Use a component enum or capability components when each actor can be in a different mode independently: locomotion, AI action, weapon phase, or animation state.

### Reading and Changing State

- `State<T>` exposes the current value.
- `NextState<T>` stores a pending transition request.
- `in_state(value)` gates systems or sets.
- `OnEnter(value)` and `OnExit(value)` run lifecycle systems.

State changes are not immediate assignment. Avoid writing `NextState` repeatedly every frame.

### State-Scoped Entities

Attach cleanup behavior to the root of state-owned content:

| Component                     | Cleanup Trigger                        |
| ----------------------------- | -------------------------------------- |
| `DespawnOnExit(value)`        | Leaving the value                      |
| `DespawnOnEnter(value)`       | Entering the value                     |
| `DespawnWhen::new(predicate)` | Matching a custom transition predicate |

Linked children are removed with the root. Persistent cameras, services, or overlays must live outside the scoped hierarchy.

### SubStates

A `SubStates` type exists only while its source state matches. Use it for nested modes such as pause or inventory screens that only make sense during gameplay.

```rust
#[derive(SubStates, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[source(AppState = AppState::Playing)]
enum PlayMode {
    #[default]
    Active,
    Paused,
}
```

When the source state exits, the substate lifecycle exits too.

### ComputedStates

Use `ComputedStates` when a mode is derived from one or more source states. This avoids a synchronization system and prevents contradictory duplicate state resources.

---

## Common Patterns

### Loading to Gameplay

Use a loading state to own the loading UI. Transition once the asset barrier succeeds or fails according to policy. Gameplay systems run only in the gameplay state.

```rust
fn finish_loading(mut next: ResMut<NextState<AppState>>) {
    next.set(AppState::Playing);
}
```

Do not assume startup ordering makes asynchronous assets ready.

### Pause as a Substate or Independent State

Use a substate when pause exists only inside gameplay. Gate simulation sets with `in_state(PlayMode::Active)` while UI and networking remain active.

Use an independent state only when pause must coexist with several source modes or has a separate lifecycle model.

### State-Owned Screen Root

Spawn one root in `OnEnter` and attach scoped cleanup to it. Descendant widgets do not each need a cleanup query.

### Transition Decision System

Centralize competing requests when multiple systems can request transitions. Convert domain facts into one next-state decision so the winner is deterministic and testable.

### Predicate Cleanup

Use `DespawnWhen` for enum variants or transition families where one exact `DespawnOnExit` value is too narrow.

---

## Gotchas

1. **States are global.** Do not create one app state per actor locomotion or AI mode.
2. **Transitions are deferred.** `NextState::set` does not mutate `State<T>` immediately inside the same system.
3. **Repeated requests compete.** Define one decision point when several systems may set the same next state.
4. **Scoped cleanup follows relationships.** Keep persistent entities outside a state-owned root.
5. **Manual cleanup queries are easy to broaden accidentally.** Prefer `DespawnOnExit` on owned roots.
6. **Duplicate mode resources drift.** Use `State`, `SubStates`, or `ComputedStates` instead of synchronizing a second enum resource.
7. **Pause should not freeze everything.** Gate authoritative simulation separately from pause UI, networking, or diagnostics.
8. **State-age counters often signal the wrong abstraction.** Use timers, lifecycle schedules, or per-entity action components.
9. **Transition tests need multiple updates.** Requesting and applying a transition occur at schedule boundaries.

## Validation Checklist

- Every state represents a coarse scheduling or lifecycle mode.
- Per-entity behavior remains in components rather than app states.
- Every transition has a clear requester and conflict policy.
- Temporary roots clean themselves up through scoped components.
- Persistent entities are intentionally outside scoped hierarchies.
- Tests cover enter, gated update, transition, exit, and cleanup.

## Related Skills

- [bevy-schedules-system-sets](../bevy-schedules-system-sets/SKILL.md) for state-gated phase sets.
- [bevy-assets-loading](../bevy-assets-loading/SKILL.md) for loading barriers and failure transitions.
- [bevy-ui-picking](../bevy-ui-picking/SKILL.md) for state-scoped menus and screens.
- [bevy-ecs-modeling](../bevy-ecs-modeling/SKILL.md) for per-entity state that should not use app `States`.
