---
name: bevy-animation
description: "Use this skill when implementing animation in Bevy 0.19. Covers AnimationGraph, AnimationGraphHandle, AnimationNodeIndex, AnimationPlayer discovery, transitions, masks, glTF clips, animation targets, events, one-shots, blending, and scene reconnection. Triggers on: animation, clip, graph, blend, transition, mask, AnimationPlayer, glTF animation, footstep event, locomotion animation."
argument-hint: "Describe the animation graph or controller behavior"
---

# Bevy 0.19 Animation

## Quick Start

Create a graph from a clip, then attach it when an imported player appears:

```rust
use bevy::prelude::*;

#[derive(Resource)]
struct AnimationAssets {
    graph: Handle<AnimationGraph>,
    idle: AnimationNodeIndex,
}

fn build_graph(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
) {
    let clip = asset_server.load(GltfAssetLabel::Animation(0).from_asset("models/hero.glb"));
    let (graph, idle) = AnimationGraph::from_clip(clip);
    commands.insert_resource(AnimationAssets {
        graph: graphs.add(graph),
        idle,
    });
}

fn start_players(
    mut commands: Commands,
    assets: Res<AnimationAssets>,
    mut players: Query<(Entity, &mut AnimationPlayer), Added<AnimationPlayer>>,
) {
    for (entity, mut player) in &mut players {
        commands.entity(entity).insert(AnimationGraphHandle(assets.graph.clone()));
        player.play(assets.idle).repeat();
    }
}
```

Imported `AnimationPlayer` components usually live on scene descendants, not the scene root.

---

## Core Concepts

### Graph-Driven Playback

An `AnimationGraph` owns graph structure. `AnimationNodeIndex` identifies semantic nodes, and `AnimationGraphHandle` connects a player to the graph. Store graph handles and node indices in controller data rather than resolving paths every frame.

### Gameplay Is Authoritative

Gameplay state drives animation. Animation is a visual projection and must not become the authority for deterministic movement, damage, or inventory changes.

### Transitions and Masks

Use transition controllers for blending instead of jumping clip time manually. Use masks or graph layers for independent upper-body actions, locomotion, and additive presentation.

### Animation Events

Animation events are suitable for cosmetic cues such as footsteps, particles, and sounds. Simulate authoritative timing independently when correctness matters.

---

## Common Patterns

### Semantic Controller

Store indices under names such as idle, run, jump, and attack. Translate gameplay components into graph requests in one presentation system.

### Scene Reconnection

On newly added `AnimationPlayer`, attach the graph and controller. Repeat this after respawn, hot reload, or scene replacement.

### One-Shot Action

Define interruption, replay, transition-out, and completion policy. Keep locomotion state ready beneath a one-shot attack or interaction.

### Event-Driven Cues

Place semantic events in clips and observe them to request audio or VFX. Keep networked gameplay independent of exact rendered frame timing.

### Missing-Asset State

Handle absent graphs, clips, and players gracefully while scenes load. Do not panic because presentation is not ready yet.

---

## Gotchas

1. **The player may not be on the root.** Discover scene descendants after instantiation.
2. **A player needs a graph.** Attach `AnimationGraphHandle` before graph-node playback.
3. **Handles must stay alive.** Retain graph and clip ownership.
4. **Animation timing is presentation timing.** Do not resolve authoritative combat from clip frames.
5. **One-shots need interruption policy.** Repeated requests can restart or stack unexpectedly.
6. **Scene replacement disconnects controllers.** Reattach on new players.
7. **Path lookups in update systems waste work.** Resolve semantic indices once.
8. **Duplicated animation state can drift.** Derive it from gameplay components.

## Validation Checklist

- Gameplay-to-animation data flow is one-way and semantic.
- Graph handles and node indices are retained and named by purpose.
- Scene-instantiated players initialize reliably.
- Interruptions, transitions, masks, and missing assets have policy.
- Authoritative gameplay is independent of visual playback timing.

## Related Skills

- [bevy-scenes-reflection](../bevy-scenes-reflection/SKILL.md) for glTF instantiation and player discovery.
- [bevy-assets-loading](../bevy-assets-loading/SKILL.md) for clip and graph handle lifetime.
- [bevy-audio](../bevy-audio/SKILL.md) for cosmetic animation-event cues.
- [bevy-ecs-modeling](../bevy-ecs-modeling/SKILL.md) for semantic gameplay state components.
