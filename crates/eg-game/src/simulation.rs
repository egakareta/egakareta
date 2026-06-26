/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::events::ConsumedObjectEvent;
use super::state::GameState;
use eg_types::{Direction, LevelObject, apply_timed_triggers_to_objects};

pub struct TimelineSimulationState {
    pub position: [f32; 3],
    pub direction: Direction,
    pub elapsed_seconds: f32,
    pub speed: f32,
    pub vertical_velocity: f32,
    pub is_grounded: bool,
}

/// REFACTOR POINT(full-bevy): currently uses its own private [`GameState`]
/// (which itself is a wrapper). Will be replaced by a `bevy_ecs::World` +
/// `Schedule` shared with the editor and play views.
pub struct TimelineSimulationRuntime {
    game: GameState,
    tap_times: Vec<f32>,
    tap_index: usize,
    elapsed_seconds: f32,
    simulation_dt: f32,
}

const TIMELINE_SIMULATION_DT: f32 = 1.0 / 240.0;
const TIMELINE_TAP_EPSILON_SECONDS: f32 = 1.0 / 480.0;

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
        let mut game = GameState::new();
        game.objects = objects.to_vec();
        game.rebuild_behavior_cache();
        game.apply_spawn(spawn_position, spawn_direction);
        game.started = true;

        let mut sorted_taps: Vec<f32> = tap_times
            .iter()
            .copied()
            .filter(|tap| tap.is_finite() && *tap >= 0.0)
            .collect();
        sorted_taps.sort_by(f32::total_cmp);

        let mut runtime = Self {
            game,
            tap_times: sorted_taps,
            tap_index: 0,
            elapsed_seconds: 0.0,
            simulation_dt: simulation_dt.clamp(1.0 / 240.0, 1.0 / 30.0),
        };

        runtime.apply_pending_taps(TIMELINE_TAP_EPSILON_SECONDS);
        runtime
    }

    fn apply_pending_taps(&mut self, up_to_time: f32) {
        while self.tap_index < self.tap_times.len() && self.tap_times[self.tap_index] <= up_to_time
        {
            self.game.turn_right();
            self.tap_index += 1;
        }
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

            self.game.update(step_dt);
            self.elapsed_seconds = step_target;
            self.apply_pending_taps(self.elapsed_seconds + TIME_EPSILON_SECONDS);

            if self.game.game_over {
                return;
            }
        }
    }

    pub fn elapsed_seconds(&self) -> f32 {
        self.elapsed_seconds
    }

    pub fn position(&self) -> [f32; 3] {
        self.game.position
    }

    pub fn direction(&self) -> Direction {
        self.game.direction
    }

    pub fn is_grounded(&self) -> bool {
        self.game.is_grounded
    }

    pub fn game_over(&self) -> bool {
        self.game.game_over
    }

    pub fn take_consumed_object_events(&mut self) -> Vec<ConsumedObjectEvent> {
        self.game.take_consumed_object_events()
    }

    pub fn trail_segments(&self) -> &[Vec<[f32; 3]>] {
        &self.game.trail_segments
    }

    pub fn objects(&self) -> &[LevelObject] {
        &self.game.objects
    }

    pub fn snapshot(&self) -> TimelineSimulationState {
        TimelineSimulationState {
            position: self.game.position,
            direction: match self.game.direction {
                Direction::Forward => Direction::Forward,
                Direction::Right => Direction::Right,
            },
            elapsed_seconds: self.elapsed_seconds,
            speed: self.game.speed,
            vertical_velocity: self.game.vertical_velocity,
            is_grounded: self.game.is_grounded,
        }
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
