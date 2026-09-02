/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use bevy_ecs::{schedule::Schedule, world::World};
use eg_types::{Direction, LevelObject, apply_timed_triggers_to_objects};

use crate::{
    components::{
        BlockId, CachedBlockBehavior, ColorTint, Grounded, Object, ObjectBundle, ObjectData,
        ObjectId, Player, PlayerBundle, PlayerDirection, PlayerPosition, PlayerSpeed, Position,
        Rotation, Size, SpawnDirection, SpawnPosition, Trail, VerticalVelocity,
    },
    events::ConsumedObjectEvent,
    physics::BASE_PLAYER_SPEED,
    plugin::{build_fixed_update_schedule, initialize_gameplay_world},
    resources::{FrameDelta, GameClock, LevelMeta, SpatialIndex},
};

#[derive(Clone, Copy)]
pub struct TimelineSimulationState {
    pub position: [f32; 3],
    pub direction: Direction,
    pub elapsed_seconds: f32,
    pub speed: f32,
    pub vertical_velocity: f32,
    pub is_grounded: bool,
}

/// ECS-backed timeline preview runtime.
///
/// The preview uses the same GamePlugin and Bevy `FixedUpdate` schedule as the
/// production App world, while keeping its own short-lived App so scrubbing
/// does not mutate the editor's authoritative gameplay world.
pub fn center_spawn_position(position: [f32; 3]) -> [f32; 3] {
    [
        position[0].floor() + 0.5,
        position[1],
        position[2].floor() + 0.5,
    ]
}
pub struct TimelineSimulationRuntime {
    world: World,
    schedule: Schedule,
    state: TimelineSimulationState,
    trail_segments: Vec<Vec<[f32; 3]>>,
    objects: Vec<LevelObject>,
    game_over: bool,
    level_complete: bool,
    tap_times: Vec<f32>,
    tap_index: usize,
    elapsed_seconds: f32,
    simulation_dt: f32,
}

const TIMELINE_SIMULATION_DT: f32 = 1.0 / 240.0;
const TIMELINE_TAP_EPSILON_SECONDS: f32 = 1.0 / 480.0;

fn object_order_key(id: &str) -> (u8, usize, &str) {
    id.strip_prefix("__obj_")
        .and_then(|suffix| suffix.parse::<usize>().ok())
        .map(|index| (0, index, id))
        .unwrap_or((1, 0, id))
}

impl TimelineSimulationRuntime {
    pub fn new(
        spawn_position: [f32; 3],
        spawn_direction: Direction,
        objects: &[LevelObject],
        tap_times: &[f32],
    ) -> Self {
        Self::new_with_dt(
            spawn_position,
            spawn_direction,
            objects,
            tap_times,
            TIMELINE_SIMULATION_DT,
        )
    }

    pub fn new_with_dt(
        spawn_position: [f32; 3],
        spawn_direction: Direction,
        objects: &[LevelObject],
        tap_times: &[f32],
        simulation_dt: f32,
    ) -> Self {
        puffin::profile_scope!("TimelineRuntimeBuild");

        let spawn_position = center_spawn_position(spawn_position);
        let authored_objects = objects.to_vec();
        let mut world = World::new();
        initialize_gameplay_world(&mut world);
        let schedule = build_fixed_update_schedule();

        {
            let world = &mut world;
            for (index, object) in authored_objects.iter().enumerate() {
                world.spawn(ObjectBundle {
                    object: Object,
                    id: ObjectId(format!("__obj_{index}")),
                    data: ObjectData(object.clone()),
                    position: Position(object.position),
                    rotation: Rotation(object.rotation),
                    size: Size(object.size),
                    block_id: BlockId(object.block_id.clone()),
                    color_tint: ColorTint(object.color_tint),
                    cached_behavior: CachedBlockBehavior::from_block_id(&object.block_id),
                });
            }

            world.spawn(PlayerBundle {
                player: Player,
                position: PlayerPosition(spawn_position),
                direction: PlayerDirection(spawn_direction),
                speed: PlayerSpeed(BASE_PLAYER_SPEED),
                vertical_velocity: VerticalVelocity(0.0),
                grounded: Grounded(true),
                trail: Trail {
                    segments: vec![vec![spawn_position]],
                },
                spawn_position: SpawnPosition(spawn_position),
                spawn_direction: SpawnDirection(spawn_direction),
            });

            world.resource_mut::<GameClock>().started = true;
            {
                let mut level = world.resource_mut::<LevelMeta>();
                level.initial_objects = authored_objects.clone();
                level.spawn_position = spawn_position;
                level.spawn_direction = spawn_direction;
            }

            let mut spatial = world
                .remove_resource::<SpatialIndex>()
                .expect("GamePlugin must install SpatialIndex");
            spatial.rebuild(world);
            world.insert_resource(spatial);
        }

        let mut sorted_taps: Vec<f32> = tap_times
            .iter()
            .copied()
            .filter(|tap| tap.is_finite() && *tap >= 0.0)
            .collect();
        sorted_taps.sort_by(f32::total_cmp);

        let mut runtime = Self {
            world,
            schedule,
            state: TimelineSimulationState {
                position: spawn_position,
                direction: spawn_direction,
                elapsed_seconds: 0.0,
                speed: BASE_PLAYER_SPEED,
                vertical_velocity: 0.0,
                is_grounded: true,
            },
            trail_segments: vec![vec![spawn_position]],
            objects: authored_objects,
            game_over: false,
            level_complete: false,
            tap_times: sorted_taps,
            tap_index: 0,
            elapsed_seconds: 0.0,
            simulation_dt: simulation_dt.clamp(1.0 / 240.0, 1.0 / 30.0),
        };

        runtime.apply_pending_taps(TIMELINE_TAP_EPSILON_SECONDS);
        runtime.sync_snapshot();
        runtime
    }

    fn apply_pending_taps(&mut self, up_to_time: f32) {
        while self.tap_index < self.tap_times.len() && self.tap_times[self.tap_index] <= up_to_time
        {
            self.apply_turn_right();
            self.tap_index += 1;
        }
    }

    fn apply_turn_right(&mut self) {
        let world = &mut self.world;
        let can_turn = {
            let clock = world.resource::<GameClock>();
            clock.started && !clock.game_over && !clock.level_complete
        };
        if !can_turn {
            return;
        }

        let mut players =
            world.query::<(&PlayerPosition, &mut PlayerDirection, &Grounded, &mut Trail)>();
        let Ok((position, mut direction, grounded, mut trail)) = players.single_mut(world) else {
            return;
        };
        if !grounded.0 {
            return;
        }

        const MIN_DELTA: f32 = 0.001;
        if let Some(segment) = trail.segments.last_mut() {
            let duplicate = segment.last().is_some_and(|last| {
                (last[0] - position.0[0]).abs() < MIN_DELTA
                    && (last[1] - position.0[1]).abs() < MIN_DELTA
                    && (last[2] - position.0[2]).abs() < MIN_DELTA
            });
            if !duplicate {
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

    pub fn advance_to(&mut self, target_time_seconds: f32) {
        puffin::profile_scope!("TimelineAdvanceTo");
        let target_time = target_time_seconds.max(0.0);
        if target_time <= self.elapsed_seconds {
            return;
        }

        const TIME_EPSILON_SECONDS: f32 = 1e-6;
        let simulation_dt = self.simulation_dt.max(TIME_EPSILON_SECONDS);

        while self.elapsed_seconds < target_time {
            self.apply_pending_taps(self.elapsed_seconds + TIME_EPSILON_SECONDS);

            let mut step_target = (self.elapsed_seconds + simulation_dt).min(target_time);
            if let Some(next_tap_time) = self.tap_times.get(self.tap_index).copied() {
                if next_tap_time <= self.elapsed_seconds + TIME_EPSILON_SECONDS {
                    self.apply_pending_taps(self.elapsed_seconds + TIME_EPSILON_SECONDS);
                    continue;
                }
                if next_tap_time < step_target {
                    step_target = next_tap_time;
                }
            }

            let step_dt = step_target - self.elapsed_seconds;
            if step_dt <= TIME_EPSILON_SECONDS {
                self.elapsed_seconds = step_target;
                self.apply_pending_taps(self.elapsed_seconds + TIME_EPSILON_SECONDS);
                continue;
            }

            self.world.resource_mut::<FrameDelta>().0 = step_dt;
            self.schedule.run(&mut self.world);
            self.elapsed_seconds = step_target;
            self.state.elapsed_seconds = step_target;
            self.apply_pending_taps(self.elapsed_seconds + TIME_EPSILON_SECONDS);
            // A tap on the exact end of this step changes the player
            // direction after the gameplay schedule has run. Snapshot after
            // applying it so scrubbing at the boundary observes that turn.
            self.sync_snapshot();

            if self.game_over {
                return;
            }
        }
    }

    pub fn elapsed_seconds(&self) -> f32 {
        self.elapsed_seconds
    }

    pub fn position(&self) -> [f32; 3] {
        self.state.position
    }

    pub fn direction(&self) -> Direction {
        self.state.direction
    }

    pub fn is_grounded(&self) -> bool {
        self.state.is_grounded
    }

    pub fn game_over(&self) -> bool {
        self.game_over
    }

    pub fn take_consumed_object_events(&mut self) -> Vec<ConsumedObjectEvent> {
        let world = &mut self.world;
        world
            .resource_mut::<bevy_ecs::message::Messages<ConsumedObjectEvent>>()
            .drain()
            .collect()
    }

    pub fn trail_segments(&self) -> &[Vec<[f32; 3]>] {
        &self.trail_segments
    }

    pub fn objects(&self) -> &[LevelObject] {
        &self.objects
    }

    pub fn snapshot(&self) -> TimelineSimulationState {
        self.state
    }

    fn sync_snapshot(&mut self) {
        let (clock, player, objects) = {
            let world = &mut self.world;
            let clock = {
                let clock = world.resource::<GameClock>();
                (clock.elapsed_seconds, clock.game_over, clock.level_complete)
            };
            let player = {
                let mut query = world.query::<(
                    &PlayerPosition,
                    &PlayerDirection,
                    &PlayerSpeed,
                    &VerticalVelocity,
                    &Grounded,
                    &Trail,
                )>();
                query.iter(world).next().map(
                    |(position, direction, speed, vertical_velocity, grounded, trail)| {
                        (
                            position.0,
                            direction.0,
                            speed.0,
                            vertical_velocity.0,
                            grounded.0,
                            trail.segments.clone(),
                        )
                    },
                )
            };
            let mut query = world.query::<(&ObjectId, &ObjectData)>();
            let mut objects = query
                .iter(world)
                .map(|(id, data)| (id.0.clone(), data.0.clone()))
                .collect::<Vec<_>>();
            objects.sort_by(|(left, _), (right, _)| {
                object_order_key(left).cmp(&object_order_key(right))
            });
            (clock, player, objects)
        };

        self.game_over = clock.1;
        self.level_complete = clock.2;
        if let Some((position, direction, speed, vertical_velocity, grounded, trail)) = player {
            self.state.position = position;
            self.state.direction = direction;
            self.state.speed = speed;
            self.state.vertical_velocity = vertical_velocity;
            self.state.is_grounded = grounded;
            self.trail_segments = trail;
        }
        self.objects = objects.into_iter().map(|(_, object)| object).collect();
    }
}
pub fn simulate_timeline_state(
    spawn_position: [f32; 3],
    spawn_direction: Direction,
    objects: &[LevelObject],
    tap_times: &[f32],
    timeline_time_seconds: f32,
) -> TimelineSimulationState {
    puffin::profile_scope!("TimelineSimulateState");
    let mut runtime =
        TimelineSimulationRuntime::new(spawn_position, spawn_direction, objects, tap_times);
    runtime.advance_to(timeline_time_seconds);
    runtime.snapshot()
}

pub fn advance_simulation_time<F>(
    elapsed_seconds: &mut f32,
    target_time_seconds: f32,
    simulation_dt: f32,
    mut step: F,
) where
    F: FnMut(f32, f32) -> bool,
{
    puffin::profile_scope!("AdvanceSimulationTime");
    let target_time = target_time_seconds.max(0.0);
    if target_time <= *elapsed_seconds {
        return;
    }

    let simulation_dt = simulation_dt.max(1e-6);

    while *elapsed_seconds + simulation_dt <= target_time {
        let step_target = *elapsed_seconds + simulation_dt;
        let should_continue = step(step_target, simulation_dt);
        *elapsed_seconds = step_target;
        if !should_continue {
            return;
        }
    }

    let remaining = target_time - *elapsed_seconds;
    if remaining > 1e-6 {
        let step_target = *elapsed_seconds + remaining;
        let _ = step(step_target, remaining);
        *elapsed_seconds = step_target;
    }
}

pub fn trigger_transformed_objects_at_time(
    base_objects: &[LevelObject],
    trigger_objects: &[LevelObject],
    time_seconds: f32,
) -> Vec<LevelObject> {
    puffin::profile_scope!("TriggerTransformObjects");
    apply_timed_triggers_to_objects(base_objects, trigger_objects, time_seconds)
}
