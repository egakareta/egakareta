/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Gameplay systems. These are pure Bevy ECS systems, grouped in
//! [`GameplaySet`] for ordering. The shared gameplay schedule drives them
//! directly from App-world resources and components.

use std::collections::HashMap;

use bevy_ecs::{prelude::*, schedule::SystemSet};
use eg_block_geometry::hitbox_cuboids;
use eg_blocks::BlockCollision;
use eg_types::{Direction, LevelObject, apply_timed_triggers_to_objects};

use crate::{
    components::{
        CachedBlockBehavior, Grounded, ObjectData, PlayerDirection, PlayerPosition, PlayerSpeed,
        Trail, VerticalVelocity,
    },
    events::{ConsumedObjectEvent, PlayerDied},
    physics::aabb_overlaps_cuboid_xz,
    resources::{
        FrameDelta, GameClock, GameRuntimeTuning, PendingTurnInputs, PlayerProgress, SpatialIndex,
        TimedTurnInput, TurnInputKind,
    },
};

/// Ordered gameplay system set. Run as a single tick to advance the
/// simulation by the explicit FrameDelta seconds resource.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameplaySet {
    /// Drain pending turn inputs into the player direction.
    TurnInput,
    /// Move the player by `speed * dt` along the current direction.
    AdvancePlayer,
    /// Detect collisions between the player AABB and object AABBs.
    Collision,
    /// Apply collision outcomes: portals (speed), collectibles (gems +
    /// despawn), hazards (game over).
    ConsumeObjects,
    /// Apply gravity, snap to support surface, update grounded state.
    GroundGravity,
    /// Manage trail segments based on grounded transitions.
    Trail,
    /// Check level duration + completion hold.
    Completion,
    /// Update UI-facing progress.
    Progress,
}

/// Per-entity collision outcome scratch. Lives as a [`Resource`] so the
/// [`GameplaySet::Collision`] and [`GameplaySet::ConsumeObjects`] systems can
/// share it without an event in between.
#[derive(Resource, Default)]
pub struct CollisionOutcome {
    pub entries: Vec<(Entity, CollisionResult)>,
}

impl CollisionOutcome {
    pub fn clear(&mut self) {
        self.entries.clear();
    }
    pub fn for_entity(&mut self, e: Entity) -> &mut CollisionResult {
        if let Some(idx) = self.entries.iter().position(|(ee, _)| *ee == e) {
            &mut self.entries[idx].1
        } else {
            self.entries.push((e, CollisionResult::default()));
            &mut self.entries.last_mut().expect("just pushed").1
        }
    }
}

#[derive(Default, Clone, Copy, Debug)]
pub struct CollisionResult {
    pub hit_death: bool,
    pub hit_portal: bool,
    pub hit_collectible: bool,
}

// ---------------------------------------------------------------------------
// 1. Turn input
// ---------------------------------------------------------------------------

/// Drain `PendingTurnInputs` whose timestamp has elapsed into the
/// `PlayerDirection`. Run before [`GameplaySet::AdvancePlayer`].
pub fn turn_input_system(
    mut pending: ResMut<PendingTurnInputs>,
    mut players: Query<(&PlayerPosition, &mut PlayerDirection, &mut Trail, &Grounded)>,
    clock: Res<GameClock>,
) {
    if !clock.started || clock.game_over || clock.level_complete {
        pending.taps.clear();
        return;
    }
    while let Some(front) = pending.taps.front() {
        if front.at_seconds <= clock.elapsed_seconds {
            let TimedTurnInput { kind, .. } = *front;
            pending.taps.pop_front();
            if let Ok((position, mut direction, mut trail, grounded)) = players.single_mut()
                && grounded.0
                && kind == TurnInputKind::TurnRight
            {
                // A grounded turn starts a new visual leg. Without this
                // anchor the renderer joins the old and new head positions
                // with a diagonal, making the snake appear to ignore turns.
                const MIN_DELTA: f32 = 0.001;
                if let Some(segment) = trail.segments.last_mut() {
                    let already_anchored = segment.last().is_some_and(|last| {
                        (last[0] - position.0[0]).abs() < MIN_DELTA
                            && (last[1] - position.0[1]).abs() < MIN_DELTA
                            && (last[2] - position.0[2]).abs() < MIN_DELTA
                    });
                    if !already_anchored {
                        segment.push(position.0);
                    }
                } else {
                    trail.segments.push(vec![position.0]);
                }
                direction.0 = match direction.0 {
                    Direction::Forward => Direction::Right,
                    Direction::Right => Direction::Forward,
                };
            }
        } else {
            break;
        }
    }
}

// ---------------------------------------------------------------------------
// 2. Advance player
// ---------------------------------------------------------------------------

/// Move the player by `speed * dt` along the current `PlayerDirection`.
pub fn advance_player_system(
    frame: Res<FrameDelta>,
    clock: Res<GameClock>,
    mut players: Query<(&mut PlayerPosition, &PlayerDirection, &PlayerSpeed)>,
) {
    if !clock.started || clock.game_over || clock.level_complete {
        return;
    }
    let step = frame.0.max(0.0);
    for (mut pos, direction, speed) in players.iter_mut() {
        let delta = match direction.0 {
            Direction::Forward => [0.0, 1.0],
            Direction::Right => [1.0, 0.0],
        };
        pos.0[0] += delta[0] * speed.0 * step;
        pos.0[2] += delta[1] * speed.0 * step;
    }
}

// ---------------------------------------------------------------------------
// 3. Collision query
// ---------------------------------------------------------------------------

/// Run spatial query + per-cuboid overlap test. Outcomes go into
/// [`CollisionOutcome`] for [`GameplaySet::ConsumeObjects`].
pub fn collision_query_system(
    spatial: Res<SpatialIndex>,
    clock: Res<GameClock>,
    tuning: Res<GameRuntimeTuning>,
    players: Query<&PlayerPosition>,
    objects: Query<(&ObjectData, &CachedBlockBehavior)>,
    mut collision_outcome: ResMut<CollisionOutcome>,
) {
    collision_outcome.clear();
    if !clock.started || clock.game_over || clock.level_complete {
        return;
    }
    let Ok(player) = players.single() else {
        return;
    };

    let pos = player.0;
    let x = pos[0];
    let y = pos[1];
    let z = pos[2];
    let pw = tuning.player_width;
    let ph = tuning.player_height;
    let ft = tuning.player_footprint_tolerance;
    let s_min_x = x - pw / 2.0 + ft;
    let s_max_x = x + pw / 2.0 - ft;
    let s_min_z = z - pw / 2.0 + ft;
    let s_max_z = z + pw / 2.0 - ft;
    let s_min_y = y + ft;
    let s_max_y = y + ph - ft;

    puffin::profile_scope!("GameCollisionQuery");
    let mut candidates: Vec<Entity> = Vec::new();
    spatial
        .grid
        .query_aabb_into(s_min_x, s_max_x, s_min_z, s_max_z, &mut candidates);

    puffin::profile_scope!("GameCollisionScan");
    for entity in candidates {
        let Ok((obj, behavior)) = objects.get(entity) else {
            continue;
        };
        let overlaps_hitbox = hitbox_cuboids(&obj.0).into_iter().any(|cuboid| {
            crate::physics::aabb_overlaps_cuboid_xz(
                s_min_x, s_max_x, s_min_z, s_max_z, &obj.0, cuboid,
            ) && s_max_y > cuboid.min[1]
                && s_min_y < cuboid.max[1]
        });
        if !overlaps_hitbox {
            continue;
        }
        let r = &mut collision_outcome.for_entity(entity);
        match behavior.collision {
            BlockCollision::Portal => r.hit_portal = true,
            BlockCollision::Collectible => r.hit_collectible = true,
            BlockCollision::Hazard => r.hit_death = true,
            BlockCollision::Solid => r.hit_death = true,
            BlockCollision::PassThrough => {}
        }
    }
}

// ---------------------------------------------------------------------------
// 4. Consume objects
// ---------------------------------------------------------------------------

/// Apply collision outcomes: despawn consumed objects, update player speed +
/// progress, and emit typed gameplay messages.
#[allow(clippy::too_many_arguments)]
pub fn consume_objects_system(
    mut commands: Commands,
    mut clock: ResMut<GameClock>,
    mut progress: ResMut<PlayerProgress>,
    mut outcome: ResMut<CollisionOutcome>,
    mut death_events: MessageWriter<PlayerDied>,
    mut consumed_events: MessageWriter<ConsumedObjectEvent>,
    mut speed: Query<&mut PlayerSpeed>,
    objects: Query<(&ObjectData, &CachedBlockBehavior)>,
) {
    if !clock.started || clock.game_over || clock.level_complete {
        outcome.clear();
        return;
    }

    let mut killed = false;
    for (_entity, result) in outcome.entries.iter() {
        if result.hit_death {
            killed = true;
            break;
        }
    }
    if killed {
        clock.game_over = true;
        death_events.write(PlayerDied);
        outcome.clear();
        return;
    }

    puffin::profile_scope!("GameConsumableOverlap");
    let mut to_despawn: Vec<(Entity, bool)> = Vec::new(); // (entity, emit_event)
    for (entity, result) in outcome.entries.iter() {
        let Ok((_obj, behavior)) = objects.get(*entity) else {
            continue;
        };
        if result.hit_portal {
            if let Ok(mut s) = speed.single_mut() {
                s.0 *= behavior.speed_multiplier.max(0.1);
            }
            if behavior.consumed_on_overlap {
                to_despawn.push((*entity, false));
            }
        }
        if result.hit_collectible {
            progress.gems_collected = progress
                .gems_collected
                .saturating_add(behavior.collectible_gem_value())
                .min(progress.gems_max);
            to_despawn.push((*entity, true));
        }
    }

    for (entity, emit_event) in to_despawn {
        let object_snapshot = objects.get(entity).ok().map(|(obj, _)| obj.0.clone());
        commands.entity(entity).despawn();
        if emit_event && let Some(obj) = object_snapshot {
            consumed_events.write(ConsumedObjectEvent { object: obj });
        }
    }
    outcome.clear();
}

// ---------------------------------------------------------------------------
// 5. Ground gravity
// ---------------------------------------------------------------------------

/// Apply gravity, snap to support surface, update `Grounded` + `VerticalVelocity`.
pub fn ground_gravity_system(
    frame: Res<FrameDelta>,
    mut clock: ResMut<GameClock>,
    tuning: Res<GameRuntimeTuning>,
    spatial: Res<SpatialIndex>,
    mut death_events: MessageWriter<PlayerDied>,
    mut players: Query<(&mut PlayerPosition, &mut VerticalVelocity, &mut Grounded)>,
    objects: Query<&ObjectData>,
) {
    if !clock.started || clock.game_over || clock.level_complete {
        return;
    }
    let step = frame.0.max(0.0);
    let gravity = tuning.gravity;
    let max_fall = tuning.max_fall_speed;
    let snap = tuning.snap_distance;
    let pw = tuning.player_width;
    let ft = tuning.player_footprint_tolerance;

    let Ok((mut pos, mut vvel, mut grounded)) = players.single_mut() else {
        return;
    };

    let s_min_x = pos.0[0] - pw / 2.0 + ft;
    let s_max_x = pos.0[0] + pw / 2.0 - ft;
    let s_min_z = pos.0[2] - pw / 2.0 + ft;
    let s_max_z = pos.0[2] + pw / 2.0 - ft;

    puffin::profile_scope!("GameSupportScan");
    let mut candidates: Vec<Entity> = Vec::new();
    spatial
        .grid
        .query_aabb_into(s_min_x, s_max_x, s_min_z, s_max_z, &mut candidates);

    // Legacy behavior: default support surface to y=0 when no objects
    // provide one. Tests rely on this for empty levels.
    let mut top_surface: Option<f32> = Some(0.0);
    for entity in candidates {
        let Ok(obj) = objects.get(entity) else {
            continue;
        };
        let behavior = CachedBlockBehavior::from_block_id_ref(&obj.0.block_id);
        if !behavior.support_surface {
            continue;
        }
        for cuboid in hitbox_cuboids(&obj.0) {
            // Broad-phase (spatial grid) found the entity, but it still needs
            // to actually overlap the player's XZ footprint. Without this
            // filter, supports in the same cell as the player but past the
            // player in x/z incorrectly ground them.
            if !aabb_overlaps_cuboid_xz(s_min_x, s_max_x, s_min_z, s_max_z, &obj.0, cuboid) {
                continue;
            }
            let top_y = cuboid.max[1];
            if top_y <= pos.0[1] + snap {
                if let Some(t) = top_surface {
                    if top_y > t {
                        top_surface = Some(top_y);
                    }
                } else {
                    top_surface = Some(top_y);
                }
            }
        }
    }

    if let Some(top) = top_surface {
        let close_enough = pos.0[1] <= top + snap && pos.0[1] >= top - snap;
        if vvel.0 <= 0.0 && close_enough {
            pos.0[1] = top;
            vvel.0 = 0.0;
            grounded.0 = true;
        } else {
            vvel.0 = (vvel.0 - gravity * step).max(-max_fall);
            pos.0[1] += vvel.0 * step;
            grounded.0 = false;
        }
    } else {
        vvel.0 = (vvel.0 - gravity * step).max(-max_fall);
        pos.0[1] += vvel.0 * step;
        grounded.0 = false;
    }

    if pos.0[1] < tuning.death_y {
        clock.game_over = true;
        death_events.write(PlayerDied);
    }
}

// ---------------------------------------------------------------------------
// 6. Trail
// ---------------------------------------------------------------------------

/// Manage trail segments based on `Grounded` changes:
///
/// - `grounded -> airborne` (just took off): append the current position to
///   the last trail segment, deduping within `MIN_DELTA`.
/// - `airborne -> grounded` (just landed): start a new segment containing
///   the current position.
///
/// `Changed<Grounded>` is not used directly because Bevy does not expose the
/// previous value of a changed component; we track it ourselves in a
/// `Local<HashMap<Entity, bool>>` so we know which transition occurred.
pub fn trail_system(
    mut players: Query<(Entity, &PlayerPosition, &mut Trail, &Grounded)>,
    mut previous_grounded: Local<HashMap<Entity, bool>>,
) {
    const MIN_DELTA: f32 = 0.001;

    for (entity, pos, mut trail, grounded) in players.iter_mut() {
        let is_grounded = grounded.0;
        let was_grounded = previous_grounded
            .get(&entity)
            .copied()
            .unwrap_or(is_grounded);
        previous_grounded.insert(entity, is_grounded);

        if was_grounded == is_grounded {
            continue;
        }

        if was_grounded && !is_grounded {
            // Just took off: extend the active ground segment with the
            // current position. Skip duplicates within MIN_DELTA so the
            // segment doesn't bloat on the same spot.
            if let Some(segment) = trail.segments.last_mut() {
                let dominated_by_last = segment
                    .last()
                    .map(|last| {
                        (last[0] - pos.0[0]).abs() < MIN_DELTA
                            && (last[1] - pos.0[1]).abs() < MIN_DELTA
                            && (last[2] - pos.0[2]).abs() < MIN_DELTA
                    })
                    .unwrap_or(false);
                if !dominated_by_last {
                    segment.push(pos.0);
                }
            } else {
                trail.segments.push(vec![pos.0]);
            }
        } else {
            // Just landed: begin a fresh air segment anchored at the
            // landing position. The render layer's "head" extension will
            // close the gap between the previous ground segment and this
            // new air segment.
            trail.segments.push(vec![pos.0]);
        }
    }
}

// ---------------------------------------------------------------------------
// 7. Completion
// ---------------------------------------------------------------------------

/// Advance the clock, check time-up, complete the level.
pub fn completion_system(
    frame: Res<FrameDelta>,
    mut clock: ResMut<GameClock>,
    mut progress: ResMut<PlayerProgress>,
) {
    let dt = frame.0.max(0.0);
    if clock.level_complete {
        clock.completion_hold_seconds = (clock.completion_hold_seconds - dt).max(0.0);
        return;
    }
    if clock.game_over {
        return;
    }
    if !clock.started {
        return;
    }
    let remaining = if clock.level_duration_seconds.is_finite() {
        (clock.level_duration_seconds - clock.elapsed_seconds).max(0.0)
    } else {
        f32::INFINITY
    };
    if remaining <= 0.0 {
        complete_level(&mut clock, &mut progress);
        return;
    }
    let step = remaining.min(dt);
    clock.elapsed_seconds += step;
    update_progress_percent(&clock, &mut progress, false);
    if clock.elapsed_seconds >= clock.level_duration_seconds {
        complete_level(&mut clock, &mut progress);
    }
}

fn complete_level(clock: &mut GameClock, progress: &mut PlayerProgress) {
    clock.level_complete = true;
    clock.completion_hold_seconds = 0.6;
    clock.started = false;
    update_progress_percent(clock, progress, true);
}

fn update_progress_percent(clock: &GameClock, progress: &mut PlayerProgress, completed: bool) {
    if clock.level_duration_seconds.is_finite() && clock.level_duration_seconds > 0.0 {
        progress.progress_percent =
            (clock.elapsed_seconds / clock.level_duration_seconds * 100.0).clamp(0.0, 100.0);
    }
    if completed {
        progress.progress_percent = 100.0;
        progress.completed = true;
    }
}

// ---------------------------------------------------------------------------
// 8. Progress (UI-facing)
// ---------------------------------------------------------------------------

/// Re-derive `PlayerProgress` from the current `GameClock`.
pub fn progress_update_system(clock: Res<GameClock>, mut progress: ResMut<PlayerProgress>) {
    if clock.level_duration_seconds.is_finite() && clock.level_duration_seconds > 0.0 {
        progress.progress_percent =
            (clock.elapsed_seconds / clock.level_duration_seconds * 100.0).clamp(0.0, 100.0);
    }
}

/// Run the timed-trigger pre-pass over a list of `LevelObject`s and return
/// the transformed copy. Wraps `eg_types::apply_timed_triggers_to_objects`
/// with an empty `triggers` slice for convenience.
pub fn apply_timed_triggers(objects: &[LevelObject], time_seconds: f32) -> Vec<LevelObject> {
    apply_timed_triggers_to_objects(objects, &[], time_seconds)
}
