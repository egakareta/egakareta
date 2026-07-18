---
name: bevy-app-plugins
description: "Use this skill when structuring a Bevy 0.19 application or splitting a game into plugins. Covers App construction, Plugin and PluginGroup boundaries, plugin configuration, simulation versus presentation separation, headless composition, lifecycle methods, and minimal plugin tests. Triggers on: app architecture, plugin, PluginGroup, module ownership, game structure, headless, simulation plugin."
argument-hint: "Describe the game architecture or plugin boundary to design"
---

# Bevy 0.19 App and Plugin Architecture

## Quick Start

Create a domain plugin and compose it at the application root:

```rust
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, CombatPlugin))
        .run();
}

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CombatRules>()
            .add_message::<DamageRequest>()
            .add_systems(FixedUpdate, resolve_damage);
    }
}

#[derive(Resource, Default)]
struct CombatRules {
    friendly_fire: bool,
}

#[derive(Message)]
struct DamageRequest {
    target: Entity,
    amount: u32,
}

fn resolve_damage() {
    // Domain logic lives in focused systems.
}
```

Keep `main` focused on runner configuration, top-level plugin composition, and `run()`.

---

## Core Concepts

### A Plugin Owns a Cohesive Domain

A plugin groups the components, resources, messages, states, systems, and setup for one reason to change. Useful boundaries include combat, inventory, world generation, player input, UI presentation, and diagnostics.

Expose the domain's public vocabulary and keep helper systems private. Avoid one global prelude that erases which plugin owns a type.

### Build-Time Configuration vs Runtime State

Plugin fields are immutable configuration consumed while `Plugin::build` constructs the app. Mutable game state belongs in resources and components:

```rust
pub struct MovementPlugin {
    pub fixed_hz: f64,
}

#[derive(Resource)]
struct MovementSettings {
    acceleration: f32,
}

impl Plugin for MovementPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_hz(self.fixed_hz))
            .insert_resource(MovementSettings { acceleration: 24.0 });
    }
}
```

Do not mutate plugin fields after `build`; the plugin object is not runtime storage.

### Plugin Lifecycle

`Plugin::build` registers almost everything. `ready` can delay finalization while asynchronous plugin setup completes. `finish` runs after all plugins are ready, and `cleanup` runs after finishing but before schedules execute.

Use `finish` and `cleanup` sparingly. Ordinary resource and system registration belongs in `build`.

### Simulation and Presentation

Authoritative simulation should run without windows, cameras, audio devices, or GPU resources where practical. Presentation plugins read simulation state and project it into rendering, audio, and UI.

This split enables deterministic tests, headless servers, replay tools, and faster iteration.

### PluginGroup

Use a `PluginGroup` for a curated suite whose members consumers may disable, replace, or configure. Use a tuple for ordinary local composition. A group is not required just because several plugins are added together.

---

## Common Patterns

### Domain Aggregator

Use one top-level game plugin to compose sibling domain plugins without absorbing their implementation:

```rust
pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            SimulationPlugin,
            InputPlugin,
            PresentationPlugin,
        ));
    }
}
```

The aggregator exposes the product composition. Each child plugin still owns its types and systems.

### Configurable Plugin

Pass values that affect app construction through plugin fields:

```rust
pub struct WorldPlugin {
    pub seed: u64,
}

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(WorldSeed(self.seed))
            .add_systems(Startup, generate_world);
    }
}

#[derive(Resource)]
struct WorldSeed(u64);
```

Use a constructor when several fields must satisfy an invariant.

### Headless Simulation Test

Install only the domain under test:

```rust
#[test]
fn combat_plugin_initializes_rules() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, CombatPlugin));
    app.update();

    assert!(app.world().contains_resource::<CombatRules>());
}
```

Add `AssetPlugin`, `StatePlugin`, `InputPlugin`, or other foundations explicitly when the domain genuinely requires them. Do not install `DefaultPlugins` by habit in logic tests.

### Optional Presentation

Keep simulation and presentation as sibling plugins so a server can add only simulation while a client adds both. Communicate through components, resources, and semantic messages rather than direct renderer dependencies.

---

## Gotchas

1. **Plugin insertion order is not system order.** Use system sets and ordering constraints for runtime causality.
2. **Plugins are unique by default.** Adding the same plugin type twice panics unless `is_unique` is overridden for a meaningful multi-instance plugin.
3. **Plugin fields are not runtime state.** Insert components or resources for values that change while the app runs.
4. **`DefaultPlugins` hides dependencies in tests.** Minimal apps reveal the real foundations a domain requires.
5. **A giant root plugin is not modularity.** Keep ownership in domain plugins even when one aggregator composes them.
6. **Presentation dependencies block headless use.** Keep cameras, windows, audio, and render assets out of authoritative simulation plugins.
7. **Old registration APIs are invalid.** Bevy 0.19 uses `.add_plugins` and `.add_systems`, not `.add_plugin`, `.add_system`, or `.add_startup_system`.
8. **Lifecycle hooks are not extra startup stages.** Use `Startup`, `OnEnter`, or ordinary systems unless plugin finalization itself is the concern.

## Validation Checklist

- Every public type and system has an obvious owning domain.
- Simulation composition works in a minimal or headless `App`.
- Build-time configuration and mutable runtime state are separate.
- Runtime ordering is expressed through schedules and sets, not plugin order.
- The target project is confirmed to use Bevy 0.19 before applying these APIs.

## Related Skills

- [bevy-ecs-modeling](../bevy-ecs-modeling/SKILL.md) for component, resource, and relationship boundaries.
- [bevy-schedules-system-sets](../bevy-schedules-system-sets/SKILL.md) for runtime ordering inside plugins.
- [bevy-testing-diagnostics-performance](../bevy-testing-diagnostics-performance/SKILL.md) for minimal app fixtures.
- [bevy-assets-loading](../bevy-assets-loading/SKILL.md) for plugins that own custom assets and loaders.
