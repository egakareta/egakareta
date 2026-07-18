---
name: bevy-testing-diagnostics-performance
description: "Use this skill when testing, diagnosing, benchmarking, or profiling a Bevy 0.19 game. Covers pure tests, World tests, minimal App fixtures, deterministic updates, controlled time and RNG, diagnostics, schedule ambiguities, stress tests, tracing, and performance measurement. Triggers on: test, regression, minimal app, diagnostic, profiler, benchmark, FPS, ambiguity, performance, frame time."
argument-hint: "Describe the behavior, regression, or performance question"
---

# Bevy 0.19 Testing, Diagnostics, and Performance

## Quick Start

Test a plugin without a renderer or window:

```rust
use bevy::prelude::*;

#[derive(Resource, Default)]
struct Counter(u32);

struct CounterPlugin;

impl Plugin for CounterPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Counter>().add_systems(Update, increment);
    }
}

fn increment(mut counter: ResMut<Counter>) {
    counter.0 += 1;
}

#[test]
fn counter_advances_once_per_update() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, CounterPlugin));
    app.update();
    assert_eq!(app.world().resource::<Counter>().0, 1);
}
```

Install only the owning plugin and foundations the behavior actually needs.

---

## Core Concepts

### Test Ladder

1. Pure Rust test for calculations and validation.
2. `World` test for components, relationships, commands, and direct queries.
3. Minimal `App` test for plugins, schedules, messages, observers, and states.
4. Headless integration app for multiple simulation domains.
5. Rendered or end-to-end test only for visual and platform behavior.

### Deterministic Fixtures

Seed randomness, control time explicitly, define update count, and avoid wall-clock sleeps. Assert authoritative components and resources rather than logs or presentation side effects.

### Diagnostics

Use typed `DiagnosticPath` values for measurements that answer an operational question. Built-in diagnostics cover frame time, entity counts, and system information. Logging plugins can periodically report selected paths.

### Performance Layers

Separate CPU schedule cost, GPU rendering, asset I/O, frame pacing, and external latency before optimizing. Aggregate FPS alone does not identify the bottleneck.

---

## Common Patterns

### Direct World Test

Use `World::new()` for relationship and component invariants without schedules. Spawn the minimum data and query it directly.

### Reusable App Constructors

Provide `minimal_app`, `simulation_app`, or `interaction_app` helpers when many tests share exact plugin composition. Keep renderer and device plugins out unless they own the tested behavior.

### Controlled State Transition

Request a transition, call `app.update()` enough times for lifecycle schedules, then assert state and scoped cleanup. Make schedule boundaries explicit in the test.

### Representative Stress Case

Stress the actual scale dimension: entity count, archetype churn, hierarchy depth, material count, lights, UI nodes, or task results. Record a before/after measurement.

### Ambiguity Review

Enable schedule ambiguity reporting during development. Add ordering for real causality and document narrow intentional ambiguities rather than silencing all conflicts.

---

## Gotchas

1. **`DefaultPlugins` makes logic tests heavy.** Use minimal foundations.
2. **Wall-clock tests are flaky.** Advance virtual or fixed time explicitly.
3. **Random defaults hide regressions.** Seed RNG and control iteration when draw order matters.
4. **One `app.update()` may not complete lifecycle work.** Model the exact schedule boundaries.
5. **Logs are not authoritative assertions.** Inspect world state.
6. **FPS does not locate cost.** Profile CPU, GPU, I/O, and pacing separately.
7. **Premature parallel queries can regress performance.** Measure item work and overhead first.
8. **Silenced ambiguities can hide order bugs.** Resolve or document each meaningful conflict.

## Validation Checklist

- The test runs at the smallest layer that owns behavior.
- Time, randomness, plugin composition, and update count are controlled.
- Lifecycle and failure boundaries are covered.
- Diagnostics answer a defined question at acceptable cost.
- Performance changes include representative measurements.
- Focused tests run before broader crate or game suites.

## Related Skills

- [bevy-app-plugins](../bevy-app-plugins/SKILL.md) for minimal domain composition.
- [bevy-schedules-system-sets](../bevy-schedules-system-sets/SKILL.md) for update boundaries and ambiguity diagnostics.
- [bevy-fixed-simulation-movement](../bevy-fixed-simulation-movement/SKILL.md) for deterministic timing tests.
- [bevy-async-tasks](../bevy-async-tasks/SKILL.md) for testing task result application without sleeps.
