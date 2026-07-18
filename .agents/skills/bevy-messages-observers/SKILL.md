---
name: bevy-messages-observers
description: "Use this skill when communicating between Bevy 0.19 systems or reacting immediately to entity events. Covers Message, MessageReader, MessageWriter, MessageMutator, Event observers, EntityEvent, lifecycle observers, targeting, propagation, and same-frame ordering. Triggers on: message, event, observer, damage request, notification, trigger, entity event, lifecycle hook, EventReader migration."
argument-hint: "Describe the communication flow and timing requirements"
---

# Bevy 0.19 Messages and Observers

## Quick Start

Use a buffered message for queued work that independent systems may read:

```rust
use bevy::prelude::*;

#[derive(Message)]
struct DealDamage {
    target: Entity,
    amount: u32,
}

fn request_damage(mut writer: MessageWriter<DealDamage>, target: Single<Entity, With<Player>>) {
    writer.write(DealDamage {
        target: *target,
        amount: 10,
    });
}

fn apply_damage(mut reader: MessageReader<DealDamage>) {
    for damage in reader.read() {
        info!("Deal {} damage to {:?}", damage.amount, damage.target);
    }
}

#[derive(Component)]
struct Player;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_message::<DealDamage>()
        .add_systems(Update, (request_damage, apply_damage).chain())
        .run();
}
```

Use an observed entity event when the reaction must happen immediately for one target:

```rust
#[derive(EntityEvent)]
struct Hit {
    entity: Entity,
    amount: u32,
}

fn on_hit(hit: On<Hit>) {
    info!("{:?} was hit for {}", hit.entity, hit.amount);
}

// Registration:
// app.add_observer(on_hit);
// Triggering:
// commands.trigger(Hit { entity: target, amount: 10 });
```

---

## Core Concepts

### Choose by Timing and Targeting

| Requirement                                        | Mechanism                   |
| -------------------------------------------------- | --------------------------- |
| Buffered queue with independent readers            | `#[derive(Message)]`        |
| Producer and consumer may run on different frames  | Message                     |
| Buffered items must be mutated                     | `MessageMutator<T>`         |
| Immediate synchronous reaction                     | observed `#[derive(Event)]` |
| Immediate reaction aimed at one entity             | `#[derive(EntityEvent)]`    |
| React to component add, insert, replace, or remove | lifecycle observer          |
| Continuously queryable state                       | component or resource       |

Messages and observer events are separate Bevy 0.19 mechanisms. Do not call every notification an event.

### Message Cursors and Fan-Out

Each `MessageReader<T>` tracks its own cursor. Readers do not compete; audio, UI, logging, and gameplay can independently read the same message.

Messages are retained for a bounded period. They are not durable storage. Put current state in components or resources.

### Immediate Observers

Observers run when an event is triggered. Register global observers with `add_observer`, attach entity-local observers with `.observe`, and trigger through `Commands::trigger`.

Observer cascades are synchronous in intent but can queue deferred commands. Bound recursion and account for entities despawned during the cascade.

### Propagation

Entity events may propagate through a relationship such as UI hierarchy. The handler can continue or stop propagation. Use this for bubbling interactions, not as a general broadcast mechanism.

---

## Common Patterns

### Request and Fact Pair

Name requests imperatively and completed facts in past tense:

```rust
#[derive(Message)]
struct DealDamage { target: Entity, amount: u32 }

#[derive(Message)]
struct DamageApplied { target: Entity, amount: u32 }
```

The simulation consumes the request, validates it, mutates health, then emits the fact for presentation.

### Same-Frame Processing

Chain producer and consumer only when same-frame handling is required. Independent presentation readers can remain unordered and accept one-frame latency.

### Per-Entity Observer

Attach a reusable observer directly to an entity when only that entity owns the reaction:

```rust
commands.spawn(Player).observe(on_hit);
```

For many entities, a global observer with target-aware queries can avoid duplicating observer entities.

### Lifecycle Index Maintenance

Use `On<Add, T>` and `On<Remove, T>` observers to maintain spatial indexes, registries, or reverse maps as structure changes. Keep the work small and deterministic.

### Message Mutation

Use `MessageMutator<T>` when one buffered pipeline stage must read and edit messages of the same type. A separate reader and writer for the same type causes conflicting access.

---

## Gotchas

1. **Old buffered event APIs are invalid.** Bevy 0.19 uses `MessageReader`, `MessageWriter`, `.add_message()`, and `.write()`, not `EventReader`, `EventWriter`, `.add_event()`, or `.send()`.
2. **Messages are not state.** Late readers cannot reconstruct authoritative current values from a transient queue.
3. **Readers have independent cursors.** Reading a message does not consume it for other systems.
4. **Unordered consumers may run next frame.** Add ordering only when same-run handling is required.
5. **Observers can recurse.** Bound trigger cascades and avoid cycles.
6. **Despawn changes later assumptions.** An earlier observer may remove the target before another reaction runs.
7. **Removal timing is subtle.** Verify which data remains queryable inside `On<Remove, T>` before assuming the component is absent.
8. **Propagation can double-handle actions.** Stop propagation when a nested observer owns the event.
9. **Borrowed ECS data cannot be payload.** Messages and events carry owned, stable domain data.

## Validation Checklist

- Delivery timing and targeting select the mechanism.
- Payloads contain owned domain data and stable context.
- Same-frame ordering exists only where required.
- Observer cascades, despawn, and propagation are bounded.
- Registration lives in the owning plugin.
- Tests cover fan-out, no-listener behavior, and target-specific reactions.

## Related Skills

- [bevy-schedules-system-sets](../bevy-schedules-system-sets/SKILL.md) for same-run producer/consumer ordering.
- [bevy-ecs-modeling](../bevy-ecs-modeling/SKILL.md) for persistent state and lifecycle structure.
- [bevy-ui-picking](../bevy-ui-picking/SKILL.md) for semantic widget events and bubbling.
- [bevy-audio](../bevy-audio/SKILL.md) for projecting gameplay facts into sound effects.
