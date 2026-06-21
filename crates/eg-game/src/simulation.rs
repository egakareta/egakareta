/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::state::{ConsumedObjectEvent, GameState};
use eg_types::{
    any_simulate_hitboxes_trigger_interpolating, apply_simulate_hitboxes_triggers_to_objects_into,
    apply_timed_triggers_to_objects, has_simulate_hitboxes_triggers,
    simulate_hitboxes_trigger_pose_signature, TimedTrigger,
};
use eg_types::{Direction, LevelObject, SpawnDirection};

pub struct TimelineSimulationState {
    pub position: [f32; 3],
    pub direction: SpawnDirection,
    pub elapsed_seconds: f32,
    pub speed: f32,
    pub vertical_velocity: f32,
    pub is_grounded: bool,
}

pub struct TimelineSimulationRuntime {
    game: GameState,
    tap_times: Vec<f32>,
    tap_index: usize,
    triggers: Vec<TimedTrigger>,
    trigger_base_objects: Option<Vec<LevelObject>>,
    trigger_transformed_objects: Vec<LevelObject>,
    has_simulate_hitboxes_triggers: bool,
    /// Signature of the last hitbox pose applied to `game.objects`, used to skip
    /// redundant object replacements + spatial-grid rebuilds while the
    /// transformed pose is stable. `None` means "pose unknown / force rebuild".
    last_hitbox_pose_signature: Option<u64>,
    elapsed_seconds: f32,
    simulation_dt: f32,
}

const TIMELINE_SIMULATION_DT: f32 = 1.0 / 240.0;
const TIMELINE_TAP_EPSILON_SECONDS: f32 = 1.0 / 480.0;

impl TimelineSimulationRuntime {
    pub fn new(
        spawn_position: [f32; 3],
        spawn_direction: SpawnDirection,
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

    pub fn new_with_triggers(
        spawn_position: [f32; 3],
        spawn_direction: SpawnDirection,
        objects: &[LevelObject],
        tap_times: &[f32],
        triggers: &[TimedTrigger],
    ) -> Self {
        Self::new_with_dt_and_triggers(
            spawn_position,
            spawn_direction,
            objects,
            tap_times,
            TIMELINE_SIMULATION_DT,
            triggers,
        )
    }

    pub fn new_with_dt(
        spawn_position: [f32; 3],
        spawn_direction: SpawnDirection,
        objects: &[LevelObject],
        tap_times: &[f32],
        simulation_dt: f32,
    ) -> Self {
        Self::new_with_dt_and_triggers(
            spawn_position,
            spawn_direction,
            objects,
            tap_times,
            simulation_dt,
            &[],
        )
    }

    pub fn new_with_dt_and_triggers(
        spawn_position: [f32; 3],
        spawn_direction: SpawnDirection,
        objects: &[LevelObject],
        tap_times: &[f32],
        simulation_dt: f32,
        triggers: &[TimedTrigger],
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

        let mut sorted_triggers: Vec<TimedTrigger> = triggers
            .iter()
            .filter(|trigger| trigger.time_seconds.is_finite())
            .cloned()
            .collect();
        sorted_triggers.sort_by(|a, b| f32::total_cmp(&a.time_seconds, &b.time_seconds));

        let has_simulate_hitboxes_triggers = has_simulate_hitboxes_triggers(&sorted_triggers);
        let mut runtime = Self {
            game,
            tap_times: sorted_taps,
            tap_index: 0,
            triggers: sorted_triggers,
            trigger_base_objects: has_simulate_hitboxes_triggers.then(|| objects.to_vec()),
            trigger_transformed_objects: Vec::new(),
            has_simulate_hitboxes_triggers,
            last_hitbox_pose_signature: None,
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

    fn apply_trigger_hitboxes(&mut self, up_to_time: f32) {
        puffin::profile_scope!("TimelineTriggerHitboxes");
        if !self.has_simulate_hitboxes_triggers || self.triggers.is_empty() {
            return;
        }

        let Some(base_objects) = self.trigger_base_objects.as_ref() else {
            return;
        };

        let time_seconds = up_to_time.max(0.0);

        // The transformed hitbox pose only changes while a simulate-hitboxes
        // trigger is interpolating, or when a trigger enters/leaves its window.
        // During the long stable stretches between trigger windows the pose is
        // identical to the previous step, and `game.objects` is already kept in
        // sync with the (consumed-pruned) base set. Skipping the object
        // replacement + spatial-grid rebuild in that case avoids thousands of
        // redundant full grid rebuilds when simulating a level with sparse
        // transform triggers (e.g. toggling "Move Hitboxes" in the editor).
        if any_simulate_hitboxes_trigger_interpolating(&self.triggers, time_seconds) {
            apply_simulate_hitboxes_triggers_to_objects_into(
                base_objects,
                &self.triggers,
                time_seconds,
                &mut self.trigger_transformed_objects,
            );
            let objects = std::mem::take(&mut self.trigger_transformed_objects);
            self.trigger_transformed_objects =
                self.game.replace_objects_for_transform_hitboxes(objects);
            // Force a rebuild on the first stable step after interpolation ends.
            self.last_hitbox_pose_signature = None;
            return;
        }

        let signature = simulate_hitboxes_trigger_pose_signature(&self.triggers, time_seconds);
        if self.last_hitbox_pose_signature == Some(signature) {
            return;
        }

        apply_simulate_hitboxes_triggers_to_objects_into(
            base_objects,
            &self.triggers,
            time_seconds,
            &mut self.trigger_transformed_objects,
        );
        let objects = std::mem::take(&mut self.trigger_transformed_objects);
        self.trigger_transformed_objects =
            self.game.replace_objects_for_transform_hitboxes(objects);
        self.last_hitbox_pose_signature = Some(signature);
    }

    fn sync_trigger_base_with_consumed(&mut self) {
        if !self.has_simulate_hitboxes_triggers {
            return;
        }

        let consumed_indices = self.game.take_consumed_object_indices();
        if consumed_indices.is_empty() {
            return;
        }

        let Some(base_objects) = self.trigger_base_objects.as_mut() else {
            return;
        };

        GameState::prune_consumed_indices_from_objects(base_objects, consumed_indices);
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

            self.apply_trigger_hitboxes(step_target);
            self.game.update(step_dt);
            self.sync_trigger_base_with_consumed();
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

    /// Returns the consumed-pruned base object set used as the source for hitbox
    /// simulation, when any transform trigger moves hitboxes. Callers can apply
    /// the full set of transform triggers to this base to render the visual
    /// object poses while still reflecting consumed blocks.
    pub fn trigger_base_objects(&self) -> Option<&[LevelObject]> {
        self.trigger_base_objects.as_deref()
    }

    /// Returns `true` when at least one transform trigger moves gameplay
    /// hitboxes during this simulation.
    pub fn has_simulate_hitboxes_triggers(&self) -> bool {
        self.has_simulate_hitboxes_triggers
    }

    pub fn snapshot(&self) -> TimelineSimulationState {
        TimelineSimulationState {
            position: self.game.position,
            direction: match self.game.direction {
                Direction::Forward => SpawnDirection::Forward,
                Direction::Right => SpawnDirection::Right,
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
    spawn_direction: SpawnDirection,
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

pub fn simulate_timeline_state_with_triggers(
    spawn_position: [f32; 3],
    spawn_direction: SpawnDirection,
    objects: &[LevelObject],
    tap_times: &[f32],
    triggers: &[TimedTrigger],
    timeline_time_seconds: f32,
) -> TimelineSimulationState {
    puffin::profile_scope!("TimelineSimulateStateWithTriggers");
    let mut runtime = TimelineSimulationRuntime::new_with_triggers(
        spawn_position,
        spawn_direction,
        objects,
        tap_times,
        triggers,
    );
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
    triggers: &[TimedTrigger],
    time_seconds: f32,
) -> Vec<LevelObject> {
    puffin::profile_scope!("TriggerTransformObjects");
    apply_timed_triggers_to_objects(base_objects, triggers, time_seconds)
}
