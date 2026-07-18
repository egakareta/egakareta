---
name: bevy-audio
description: "Use this skill when implementing or debugging audio in Bevy 0.19. Covers AudioPlayer entities, PlaybackSettings, AudioSink, SpatialAudioSink, listeners, one-shot messages, looping emitters, soundtrack states, fades, volume categories, concurrency, and cleanup. Triggers on: audio, sound effect, music, ambience, spatial sound, AudioPlayer, AudioSink, fade, volume, playback."
argument-hint: "Describe the sound, music, spatial, or playback behavior"
---

# Bevy 0.19 Audio

## Quick Start

Spawn entity-owned one-shot and looping playback:

```rust
use bevy::prelude::*;

fn play_audio(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.spawn(AudioPlayer::new(asset_server.load("audio/hit.ogg")));

    commands.spawn((
        AudioPlayer::new(asset_server.load("audio/music.ogg")),
        PlaybackSettings::LOOP,
        Music,
    ));
}

#[derive(Component)]
struct Music;
```

Playback is an entity lifecycle. Despawning a playback entity stops it.

---

## Core Concepts

### Playback Categories

Classify sounds as one-shot effects, persistent emitters, ambience, voice, or music. Each category needs ownership, concurrency, volume, pause, and cleanup policy.

### AudioPlayer and Sink

Insert `AudioPlayer` to request playback. Add `PlaybackSettings` for looping, volume, speed, and lifetime behavior. `AudioSink` or `SpatialAudioSink` appears after backend initialization, so it may not exist in the spawn tick.

### Semantic Requests

Gameplay emits facts such as jump, hit, or music-state change. The audio plugin resolves handles and voice policy. Keep paths and backend details out of gameplay systems.

### Spatial Audio

Use emitter transforms and `SpatialListener`. Choose one world-to-audio scale and let normal transform propagation update positions. UI sounds should usually remain non-spatial.

---

## Common Patterns

### Sound-Effect Message

Read a buffered gameplay fact, choose a retained audio handle, and spawn a one-shot entity. Decide whether repeated requests overlap, deduplicate, rate-limit, restart, or steal an older voice.

### Music State and Cross-Fade

Mark long-lived music entities, model desired soundtrack state explicitly, and fade current/new sinks over time. Despawn only after fade-out completes.

### Loop Attached to Source

Spawn looping playback with the source transform or maintain a relationship to the source. Clean it up when the source despawns or leaves its state.

### Volume Layers

Separate global, category, and instance volume. Store user settings authoritatively and project them onto active sinks.

### Retained Audio Catalog

Load handles once in an audio asset resource. Clone handles when spawning playback instead of loading paths repeatedly.

---

## Gotchas

1. **`AudioBundle` is obsolete.** Bevy 0.19 uses `AudioPlayer` entities.
2. **Sinks are initialized later.** Query them on subsequent updates.
3. **Despawn stops playback.** Give long-lived loops an owner that survives appropriately.
4. **Repeated one-shots can create voice storms.** Define concurrency policy.
5. **Paths in update systems waste work.** Retain handles.
6. **Spatial scale affects attenuation.** Match it intentionally to world units.
7. **Duplicate listeners produce ambiguous output.** Spawn them deliberately.
8. **Cosmetic audio timing is not gameplay timing.** Do not drive authoritative actions from playback.

## Validation Checklist

- Every category has ownership, concurrency, volume, pause, and cleanup policy.
- Handles are retained and sinks are accessed after initialization.
- Spatial scale and listener behavior are explicit.
- Device loss, mute, fade, and repeated requests have behavior.
- Representative scenes verify audible and spatial output.

## Related Skills

- [bevy-assets-loading](../bevy-assets-loading/SKILL.md) for retained audio handles.
- [bevy-messages-observers](../bevy-messages-observers/SKILL.md) for semantic sound-effect requests.
- [bevy-animation](../bevy-animation/SKILL.md) for cosmetic clip-event cues.
- [bevy-states-scoped-entities](../bevy-states-scoped-entities/SKILL.md) for soundtrack and ambience lifecycle.
