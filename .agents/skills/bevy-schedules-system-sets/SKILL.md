---
name: bevy-schedules-system-sets
description: "Use this skill when organizing or debugging Bevy 0.19 system execution. Covers schedules, SystemSet pipelines, configure_sets, ordering, run conditions, deferred Commands, ApplyDeferred, ambiguity detection, and Update versus FixedUpdate. Triggers on: schedule, system order, SystemSet, run_if, before, after, chain, deferred commands, one-frame delay, ambiguity, FixedUpdate."
argument-hint: "Describe the systems, ordering constraint, or scheduling bug"
---

# Bevy 0.19 Schedules and System Sets

## Quick Start

Define a visible gameplay pipeline and assign systems to phases:

```rust
use bevy::prelude::*;

#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum GameState {
    #[default]
    Loading,
    Playing,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum GameplaySet {
    CollectInput,
    Simulate,
    Resolve,
}

pub struct GameplayPlugin;

impl Plugin for GameplayPlugin {
    fn build(&self, app: &mut App) {
        app.configure_sets(
            FixedUpdate,
            (
                GameplaySet::CollectInput,
                GameplaySet::Simulate,
                GameplaySet::Resolve,
            )
                .chain()
                .run_if(in_state(GameState::Playing)),
        )
        .add_systems(
            FixedUpdate,
            (
                collect_input.in_set(GameplaySet::CollectInput),
                integrate_velocity.in_set(GameplaySet::Simulate),
                resolve_collisions.in_set(GameplaySet::Resolve),
            ),
        );
    }
}

fn collect_input() {}
fn integrate_velocity() {}
fn resolve_collisions() {}
```

Order phases once with `configure_sets`. Individual systems declare membership without duplicating the entire ordering graph.

---

## Core Concepts

### Schedules Define Cadence

| Schedule             | Typical Use                                                |
| -------------------- | ---------------------------------------------------------- |
| `Startup`            | One-time world initialization                              |
| `PreUpdate`          | Input and engine preparation that must precede frame logic |
| `Update`             | Frame-rate gameplay, UI, and presentation decisions        |
| `FixedUpdate`        | Deterministic simulation ticks                             |
| `RunFixedMainLoop`   | Per-frame work directly before or after the fixed loop     |
| `PostUpdate`         | Transform-dependent projection and late frame work         |
| `OnEnter` / `OnExit` | App-state lifecycle work                                   |

A schedule controls when systems can run. A system set groups systems inside a schedule for ordering and shared conditions.

### Systems Are Unordered by Default

Bevy may run compatible systems in parallel. Data conflicts serialize access, but serialization does not establish semantic order. Add `.before`, `.after`, or `.chain()` only when one system must observe another system's same-run effects.

Prefer phase sets over a web of direct system-to-system edges:

`Collect -> Decide -> Simulate -> Resolve -> Present`

### Run Conditions

Run conditions are read-only systems that return `bool`. Attach a shared condition to a set when one decision should gate the whole phase:

```rust
app.configure_sets(
    FixedUpdate,
    GameplaySet::Simulate
        .run_if(in_state(GameState::Playing))
        .run_if(resource_exists::<WorldReady>),
);

#[derive(Resource)]
struct WorldReady;
```

Keep conditions cheap. A condition can be evaluated more than once depending on how it is attached.

### Deferred Commands

`Commands` queues structural mutation. Spawns, despawns, resource insertion, and component changes are not necessarily visible until deferred buffers are applied.

Use an explicit boundary only when a later system must observe the structural change in the same schedule:

```rust
app.add_systems(
    Update,
    (spawn_enemies, ApplyDeferred, initialize_new_enemies).chain(),
);
```

Avoid forcing a flush when a one-frame delay or message-based handoff is acceptable.

### Ambiguity Detection

An ambiguity is two systems that conflict on data without an ordering edge. Some conflicts are harmless; others reveal nondeterministic behavior. Inspect and resolve them during development instead of silencing everything globally.

---

## Common Patterns

### Domain Phase Pipeline

Expose a small public set enum from the owning plugin. Other plugins can place their systems relative to a phase without depending on private system functions.

```rust
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CombatSet {
    GatherRequests,
    ApplyDamage,
    Cleanup,
}
```

This is more stable than exposing every system as an ordering target.

### Same-Frame Message Processing

Buffered messages may be read on the next run unless producers and consumers are ordered. Chain only the portion that requires same-frame delivery:

```rust
app.add_systems(
    Update,
    (emit_damage_requests, apply_damage_requests).chain(),
);
```

Leave independent VFX and audio readers unordered when one-frame latency is acceptable.

### Set-Level Pause or Loading Gate

Gate the simulation set rather than repeating `run_if` on every system. Keep UI, networking, and pause-menu systems in separate sets if they must continue while simulation is paused.

### Before and After the Fixed Loop

Sample frame-edge input in `RunFixedMainLoopSystems::BeforeFixedMainLoop`, simulate in `FixedUpdate`, and interpolate visual transforms in `AfterFixedMainLoop`. Do not put all three jobs into one schedule.

### Custom Schedule

Create a custom schedule only when the work has a genuinely distinct cadence or execution boundary. If the requirement is just ordering, use a set inside a standard schedule.

---

## Gotchas

1. **Data conflicts are not semantic order.** Two mutating systems may serialize in either valid order unless constrained.
2. **Commands are deferred.** A later system may not see a newly inserted component without an application boundary.
3. **Over-ordering removes parallelism.** Do not chain unrelated systems for a vague sense of determinism.
4. **Conditions can evaluate separately.** Attach shared gates to configured sets when one decision should control a phase.
5. **Messages can arrive one run later.** Order only producers and consumers that require same-run handling.
6. **Plugin order is not schedule order.** Adding plugins in a tuple does not order their systems.
7. **`FixedUpdate` can run zero or many times per frame.** Frame-edge input and camera presentation need different timing.
8. **Old stage APIs are invalid.** Do not use `CoreStage`, `SystemLabel`, `.add_system`, or custom stage insertion from older Bevy releases.
9. **Silencing ambiguities can hide bugs.** Document intentional ambiguity narrowly.

## Validation Checklist

- Every ordering edge corresponds to a real data or side-effect dependency.
- The domain pipeline is readable from one `configure_sets` call.
- Shared pause, loading, and readiness conditions gate sets.
- Deferred mutation visibility is deliberate and tested.
- Fixed simulation, input sampling, and presentation use the correct schedules.
- Development builds inspect schedule ambiguities.

## Related Skills

- [bevy-app-plugins](../bevy-app-plugins/SKILL.md) for the plugins that own schedules and sets.
- [bevy-fixed-simulation-movement](../bevy-fixed-simulation-movement/SKILL.md) for fixed-loop timing and interpolation.
- [bevy-states-scoped-entities](../bevy-states-scoped-entities/SKILL.md) for state conditions and lifecycle schedules.
- [bevy-messages-observers](../bevy-messages-observers/SKILL.md) for same-run versus buffered communication.
- [bevy-testing-diagnostics-performance](../bevy-testing-diagnostics-performance/SKILL.md) for ambiguity diagnostics and schedule tests.
