---
name: bevy-ecs-modeling
description: "Use this skill when modeling gameplay data in Bevy 0.19. Covers components, resources, marker components, bundles, required components, typed relationships, query design, Single, component hooks, removal tracking, and change detection. Triggers on: ECS, component, resource, entity, query, bundle, relationship, required component, Changed, Added, system data model."
argument-hint: "Describe the gameplay data or ECS model to design"
---

# Bevy 0.19 ECS Modeling

## Quick Start

Model an actor from focused facts and query only what a system needs:

```rust
use bevy::prelude::*;

#[derive(Component)]
struct Player;

#[derive(Component, PartialEq)]
struct Health {
    current: u32,
    maximum: u32,
}

#[derive(Component, Deref, DerefMut)]
struct Velocity(Vec3);

#[derive(Resource)]
struct DamagePerTick(u32);

fn apply_damage(
    damage: Res<DamagePerTick>,
    mut players: Query<&mut Health, With<Player>>,
) {
    for mut health in &mut players {
        health.current = health.current.saturating_sub(damage.0);
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .insert_resource(DamagePerTick(5))
        .add_systems(Update, apply_damage)
        .run();
}
```

Components describe per-entity facts and capabilities. Resources describe values that are unique to the world. Systems declare data access through parameters, allowing Bevy to schedule compatible work in parallel.

---

## Core Concepts

### Components, Resources, and Locals

- Use a `Component` for a fact attached to zero or more entities.
- Use a `Resource` for one world-wide service, configuration, index, or aggregate.
- Use `Local<T>` for private state owned by one system instance.
- Use a zero-sized marker component for capabilities and query classification.

Split components by independent access patterns. A health system should not conflict with animation merely because both fields were placed in one large `Actor` component.

### Entities and Stable Identity

An `Entity` is a world-local runtime ID. It can be despawned and its generation can become invalid. Use typed relationships for maintained runtime links and stable authored IDs for save data, manifests, networking, or cross-session identity.

### Query Shape

Queries encode both data access and structural selection:

```rust
#[derive(Component)]
struct Grounded;

fn move_grounded_players(
    time: Res<Time>,
    mut query: Query<(&Velocity, &mut Transform), (With<Player>, With<Grounded>)>,
) {
    for (velocity, mut transform) in &mut query {
        transform.translation += **velocity * time.delta_secs();
    }
}
```

Use `Single` when exactly one matching entity is a real invariant. Use `Query::single()` and handle the `Result` when zero or multiple matches are expected during lifecycle transitions.

### Required Components

Required components establish structural invariants at insertion time:

```rust
#[derive(Component)]
#[require(Transform, Visibility)]
struct WorldActor;
```

Inserting `WorldActor` supplies missing required components. Explicitly supplied values win. Requirements are recursive, so avoid cycles and conflicting constructor trees.

### Relationships

Use derived relationships instead of manually synchronizing both sides of an entity link:

```rust
#[derive(Component)]
#[relationship(relationship_target = Inventory)]
struct InInventoryOf(Entity);

#[derive(Component)]
#[relationship_target(relationship = InInventoryOf)]
struct Inventory(Vec<Entity>);
```

The relationship component is the source of truth. Bevy maintains the target collection through hooks. `ChildOf` and `Children` are the built-in linked relationship for transform and UI hierarchy.

### Change Detection

`Added<T>` and `Changed<T>` are relative to the last time that system ran, not to the current rendered frame. A mutable dereference marks the entire value changed.

Use `set_if_neq` to preserve useful signals:

```rust
fn clamp_health(mut query: Query<&mut Health>) {
    for mut health in &mut query {
        let clamped = Health {
            current: health.current.min(health.maximum),
            maximum: health.maximum,
        };
        health.set_if_neq(clamped);
    }
}
```

Use `Ref<T>` or `Res<T>::is_changed()` when the system needs the value and its change metadata. Read removals through `RemovedComponents<T>`.

---

## Common Patterns

### Spawn Vocabulary with Bundles

Bundles group components for spawning, but they are not runtime types:

```rust
#[derive(Bundle)]
struct EnemyBundle {
    enemy: Enemy,
    health: Health,
    transform: Transform,
}

#[derive(Component)]
struct Enemy;
```

Query the contained components, never `Query<&EnemyBundle>`.

### Capability Components

Represent orthogonal capabilities independently:

```rust
#[derive(Component)]
struct CanFly;

#[derive(Component)]
struct Poisoned {
    damage_per_second: f32,
}

#[derive(Component)]
enum LocomotionMode {
    Idle,
    Walking,
    Falling,
}
```

Use an enum for mutually exclusive modes and separate components for facts that can coexist.

### Structural Filters

Prefer filters over branches inside broad loops:

```rust
#[derive(Component)]
struct Awake;

fn wake_new_enemies(query: Query<Entity, (Added<Enemy>, Without<Awake>)>) {
    for entity in &query {
        info!("Enemy {entity:?} entered the world");
    }
}
```

### Structural Hooks

Use component hooks or lifecycle observers for invariants, registration, and indexes. Use ordinary systems for recurring gameplay. Hooks run during structural mutation and should remain small and predictable.

---

## Gotchas

1. **Mutable access can create false changes.** Dereferencing `Mut<T>` or `ResMut<T>` marks the value changed even when the assigned value is equal.
2. **Bundles are not queryable identities.** They are only insertion and spawn vocabulary.
3. **Raw entity links become stale.** Use relationships or explicit invalidation when linked entities can despawn.
4. **Relationship target collections are derived.** Do not mutate them directly; insert or replace the source relationship.
5. **Required-component cycles overflow.** Keep requirement trees shallow and acyclic.
6. **Large components reduce parallelism.** Unrelated systems conflict when they request mutable access to the same oversized component.
7. **`Changed<T>` is not a frame flag.** It depends on the observing system's last run and can remain visible across skipped runs.
8. **Sparse-set storage is not automatically faster.** Use it only after measuring insertion/removal-heavy access patterns.
9. **Components should not mirror UI or asset state.** Keep one authoritative value and project it outward.

## Validation Checklist

- Every component describes one focused fact, capability, or local mode.
- Resources are truly unique and have explicit initialization ownership.
- Query filters express structural intent and cardinality assumptions are handled.
- Runtime entity links remain valid across replacement and despawn.
- Change-driven systems avoid false-positive writes.
- Core invariants have focused `World` or minimal-`App` tests.

## Related Skills

- [bevy-app-plugins](../bevy-app-plugins/SKILL.md) for assigning ECS ownership to domains.
- [bevy-schedules-system-sets](../bevy-schedules-system-sets/SKILL.md) for ordering systems that operate on the model.
- [bevy-messages-observers](../bevy-messages-observers/SKILL.md) for transient communication and lifecycle reactions.
- [bevy-testing-diagnostics-performance](../bevy-testing-diagnostics-performance/SKILL.md) for direct `World` and minimal `App` tests.
