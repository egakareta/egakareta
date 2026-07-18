---
name: bevy-async-tasks
description: "Use this skill when moving expensive work off Bevy 0.19's main thread. Covers AsyncComputeTaskPool, IoTaskPool, Task components, check_ready, channels, external producers, CommandQueue results, cancellation, stale requests, and bounded result application. Triggers on: async, background task, thread pool, world generation, pathfinding, procedural mesh, channel, network result, nonblocking."
argument-hint: "Describe the background work and result flow"
---

# Bevy 0.19 Async Tasks

## Quick Start

Store an owned task on an entity and poll it without blocking:

```rust
use bevy::{
    prelude::*,
    tasks::{futures::check_ready, AsyncComputeTaskPool, Task},
};

#[derive(Component)]
struct ComputePath(Task<Vec<Vec3>>);

fn start_path(mut commands: Commands) {
    let task = AsyncComputeTaskPool::get().spawn(async move {
        vec![Vec3::ZERO, Vec3::X, Vec3::new(2.0, 0.0, 1.0)]
    });
    commands.spawn(ComputePath(task));
}

fn finish_paths(mut commands: Commands, mut tasks: Query<(Entity, &mut ComputePath)>) {
    for (entity, mut task) in &mut tasks {
        if let Some(path) = check_ready(&mut task.0) {
            commands.entity(entity).insert(Path(path)).remove::<ComputePath>();
        }
    }
}

#[derive(Component)]
struct Path(Vec<Vec3>);
```

Extract owned plain data before spawning. Apply world mutation later through an ECS system or returned `CommandQueue`.

---

## Core Concepts

### Choose the Work Pattern

| Work                           | Pattern                               |
| ------------------------------ | ------------------------------------- |
| CPU-heavy finite computation   | `AsyncComputeTaskPool` plus `Task<T>` |
| File or blocking-oriented work | `IoTaskPool` or appropriate async I/O |
| Long-lived external producer   | Bounded channel drained by a system   |
| Several world mutations        | Return data or a `CommandQueue`       |

### ECS Ownership Boundary

Tasks receive owned `Send` data. Never hold `World`, query, component, or resource borrows across `.await`. Arbitrary threads must not mutate the ECS world directly.

### Task Lifetime

A task component ties work to an entity and lets despawn or replacement cancel ownership. A detached task requires a separate shutdown signal and result channel policy.

### Stale Results

Include a request ID, source version, or generation when newer work can supersede older work. Validate that the entity still exists and still wants that result before applying it.

---

## Common Patterns

### Return Plain Data

Prefer a domain result such as a path, generated chunk, or parsed response. A focused system validates and applies it.

### Return a CommandQueue

For complex world changes, build a `CommandQueue` in the task and append it on the main ECS side. Captured entity IDs still require existence and staleness checks.

### Channel Producer

Detach a long-lived producer only when it communicates through a bounded channel and has shutdown ownership. Drain a limited number of results per frame.

### Chunked Application

Large world-generation results can still stall when applied. Insert a queue resource and apply a frame-budgeted chunk each update.

### Deterministic Test

Test result application by inserting a completed domain result directly. Do not rely on wall-clock sleeps or thread scheduling for ordinary behavior tests.

---

## Gotchas

1. **Polling must not block.** Use `check_ready`, not `block_on(poll_once(...))`.
2. **ECS borrows cannot cross await points.** Extract owned data first.
3. **Late results can target stale entities.** Validate identity and request version.
4. **Detached tasks lose result ownership.** Detach only with explicit channel, failure, and shutdown policy.
5. **Unbounded spawning creates work storms.** Deduplicate requests and cap concurrency.
6. **Result application can still hitch.** Budget large ECS changes across frames.
7. **Pool choice matters.** CPU work and blocking I/O have different pools.
8. **Wasm concurrency differs.** Verify platform behavior before assuming native parallelism.

## Validation Checklist

- Inputs are owned plain data and world mutation stays on the ECS side.
- Completed, failed, canceled, and stale results have policy.
- Concurrency and channel growth are bounded.
- Main-thread application respects the frame budget.
- Tests inject results without timing sleeps.

## Related Skills

- [bevy-ecs-modeling](../bevy-ecs-modeling/SKILL.md) for task ownership and result components.
- [bevy-schedules-system-sets](../bevy-schedules-system-sets/SKILL.md) for polling and application phases.
- [bevy-assets-loading](../bevy-assets-loading/SKILL.md) when the work is an asset-loading concern.
- [bevy-testing-diagnostics-performance](../bevy-testing-diagnostics-performance/SKILL.md) for deterministic task-result tests.
