---
name: bevy-ui-picking
description: "Use this skill when building Bevy 0.19 menus, HUDs, inventories, semantic widgets, focus navigation, accessibility, or pointer interaction. Covers Node layout, Button and Activate, authoritative UI state, picking observers, propagation, drag and drop, and state-scoped screens. Triggers on: UI, menu, HUD, widget, button, focus, accessibility, picking, click, hover, drag, drop."
argument-hint: "Describe the UI workflow or picking interaction"
---

# Bevy 0.19 UI and Picking

## Quick Start

Create a menu button that transitions into gameplay:

```rust
use bevy::{
	prelude::*,
	ui_widgets::{observe, Activate, Button},
};

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
		.run();
}

fn spawn_menu(mut commands: Commands) {
	commands.spawn(Camera2d);
	commands.spawn((
		DespawnOnExit(AppState::Menu),
		Node {
			width: percent(100),
			height: percent(100),
			align_items: AlignItems::Center,
			justify_content: JustifyContent::Center,
			..default()
		},
		children![(
			Button,
			Node {
				width: px(180),
				height: px(56),
				align_items: AlignItems::Center,
				justify_content: JustifyContent::Center,
				..default()
			},
			BackgroundColor(Color::srgb(0.15, 0.15, 0.15)),
			children![Text::new("Play")],
			observe(
				|_activate: On<Activate>, mut next: ResMut<NextState<AppState>>| {
					next.set(AppState::Playing);
				},
			),
		)],
	));
}
```

Use semantic widget events such as `Activate` instead of handling raw mouse input. This lets pointer, keyboard, and gamepad activation share one action path.

---

## Core Concepts

### UI Is an ECS Hierarchy

Bevy 0.19 UI entities are composed from ordinary components:

- `Node` controls layout, sizing, positioning, overflow, and flex/grid behavior.
- `BackgroundColor`, `BorderColor`, `BorderRadius`, and shadows control visual styling.
- `Text`, `TextFont`, and `TextColor` define text content and appearance.
- `ImageNode` displays images and atlas regions.
- `Children` and `ChildOf` form the UI tree.
- `UiTransform` applies visual translation, rotation, and scale without changing layout.

Spawn component tuples directly. Bevy 0.19 does not use the old `NodeBundle`, `ButtonBundle`, or monolithic `Style` component.

### Authoritative State and Projection

The game state owns values such as health, volume, selected inventory slot, and key bindings. UI entities project those values and emit semantic requests to change them.

Do not infer state from colors, text strings, visibility, or child order. Add marker components to locate presentation entities and update them when authoritative data changes.

### Semantic Widgets

`bevy_ui_widgets` provides behavior components such as `Button`, `Checkbox`, and `Slider`. Widget actions are immediate entity events handled by observers:

```rust
commands.spawn((
	Button,
	children![Text::new("Apply")],
	observe(|_activate: On<Activate>| {
		info!("Apply requested");
	}),
));
```

`InteractionDisabled`, `Checked`, `Pressed`, focus components, and accessibility metadata communicate semantic state. Preserve these behaviors when implementing a custom control.

### Picking and Propagation

Picking converts pointer input into target-aware events such as `Pointer<Click>`, `Pointer<Over>`, `Pointer<Drag>`, and `Pointer<Drop>`. Observers can live globally or directly on an entity.

Entity events can propagate through relationships. This is useful for nested controls, but a child may accidentally activate a parent unless propagation is stopped by the handler.

Set `Pickable::IGNORE` on decorative descendants that should never become the hit target.

### Focus and Navigation

Pointer support is not enough for a game UI. Interactive controls need focus order, keyboard/gamepad activation, visible focus state, disabled semantics, and accessibility roles. Use tab or directional navigation according to the game platform and layout.

---

## Common Patterns

### State-Scoped Screens

Spawn one root entity when entering a screen and attach `DespawnOnExit` to that root. Its linked descendants are removed automatically:

```rust
fn spawn_pause_menu(mut commands: Commands) {
	commands.spawn((
		DespawnOnExit(AppState::Menu),
		Node {
			width: percent(100),
			height: percent(100),
			..default()
		},
		children![Text::new("Paused")],
	));
}
```

Keep persistent cameras, notifications, and overlays outside the scoped hierarchy when they must survive the transition.

### Synchronizing UI from Game State

Use a marker and change detection to update only the affected presentation:

```rust
#[derive(Resource, Default)]
struct Score(u32);

#[derive(Component)]
struct ScoreLabel;

fn update_score_label(
	score: Res<Score>,
	mut label: Single<&mut Text, With<ScoreLabel>>,
) {
	if score.is_changed() {
		**label = format!("Score: {}", score.0);
	}
}
```

Use `set_if_neq` when writing state so unchanged values do not trigger presentation systems.

### Pointer Interaction on World Entities

Attach an observer to a pickable sprite or mesh:

```rust
commands
	.spawn((
		Sprite::from_color(Color::srgb(0.2, 0.7, 0.9), Vec2::splat(64.0)),
		Pickable::default(),
	))
	.observe(|click: On<Pointer<Click>>| {
		info!("Selected {:?}", click.event_target());
	});
```

Use the picking backend that matches UI nodes, sprites, meshes, or custom hit data. Handle multiple cameras and windows explicitly when converting cursor positions yourself.

### Drag State

Store drag ownership and payload as explicit components or resources. Define cancellation for pointer release, focus loss, entity despawn, and app-state transitions. Apply hover previews reversibly; commit authoritative state only on a valid drop.

---

## Gotchas

1. **Widgets are experimental in 0.19.** Preserve project-local widget conventions and do not assume `bevy_ui_widgets` APIs stay unchanged across releases.
2. **Visuals are not state.** `BackgroundColor`, label text, and visibility must not become the source of truth.
3. **Decorative children can intercept input.** Add `Pickable::IGNORE` when an image or text child should not receive pointer hits.
4. **Pointer-only controls are incomplete.** Add focus, keyboard/gamepad activation, disabled state, and accessibility metadata.
5. **Propagation can double-activate controls.** Stop entity-event propagation when a nested handler owns the action.
6. **Rebuilding trees causes churn.** Keep stable entities and synchronize changed components instead of despawning the whole UI every frame.
7. **Layout and visual transforms differ.** Use `Node` for layout and `UiTransform` for visual motion that should not cause reflow.
8. **Drag operations need cancellation.** A state change or despawn can otherwise leave stale drag ownership behind.
9. **Old UI APIs are invalid.** Do not use `NodeBundle`, `ButtonBundle`, or `Style` from older Bevy versions.

## Validation Checklist

- The domain value has one authoritative owner outside presentation details.
- Pointer, keyboard, and gamepad paths emit the same semantic action.
- Focused, hovered, pressed, disabled, selected, loading, and empty states are covered.
- Screen roots clean up automatically with app-state transitions.
- Decorative entities and nested propagation have intentional hit behavior.
- The layout is checked at representative resolutions and scale factors.

## Related Skills

- [bevy-states-scoped-entities](../bevy-states-scoped-entities/SKILL.md) for screen lifecycle and automatic cleanup.
- [bevy-messages-observers](../bevy-messages-observers/SKILL.md) for semantic widget events and observer propagation.
- [bevy-input-actions](../bevy-input-actions/SKILL.md) for keyboard, gamepad, and remappable action input.
- [bevy-ecs-modeling](../bevy-ecs-modeling/SKILL.md) for authoritative resources, marker components, and change detection.
