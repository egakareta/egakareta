use super::state::GameState;
use crate::types::{
    apply_timed_triggers_to_objects, Direction, LevelObject, SpawnDirection, TimedTrigger,
};

pub(crate) struct TimelineSimulationState {
    pub(crate) position: [f32; 3],
    pub(crate) direction: SpawnDirection,
    pub(crate) elapsed_seconds: f32,
    pub(crate) speed: f32,
}

pub(crate) struct TimelineSimulationRuntime {
    game: GameState,
    tap_times: Vec<f32>,
    tap_index: usize,
    triggers: Vec<TimedTrigger>,
    trigger_base_objects: Option<Vec<LevelObject>>,
    simulate_trigger_hitboxes: bool,
    elapsed_seconds: f32,
    simulation_dt: f32,
}

const TIMELINE_SIMULATION_DT: f32 = 1.0 / 240.0;
const TIMELINE_TAP_EPSILON_SECONDS: f32 = 1.0 / 480.0;

impl TimelineSimulationRuntime {
    pub(crate) fn new(
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

    pub(crate) fn new_with_triggers(
        spawn_position: [f32; 3],
        spawn_direction: SpawnDirection,
        objects: &[LevelObject],
        tap_times: &[f32],
        triggers: &[TimedTrigger],
        simulate_trigger_hitboxes: bool,
    ) -> Self {
        Self::new_with_dt_and_triggers(
            spawn_position,
            spawn_direction,
            objects,
            tap_times,
            TIMELINE_SIMULATION_DT,
            triggers,
            simulate_trigger_hitboxes,
        )
    }

    pub(crate) fn new_with_dt(
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
            false,
        )
    }

    pub(crate) fn new_with_dt_and_triggers(
        spawn_position: [f32; 3],
        spawn_direction: SpawnDirection,
        objects: &[LevelObject],
        tap_times: &[f32],
        simulation_dt: f32,
        triggers: &[TimedTrigger],
        simulate_trigger_hitboxes: bool,
    ) -> Self {
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

        let mut runtime = Self {
            game,
            tap_times: sorted_taps,
            tap_index: 0,
            triggers: sorted_triggers,
            trigger_base_objects: simulate_trigger_hitboxes.then(|| objects.to_vec()),
            simulate_trigger_hitboxes,
            elapsed_seconds: 0.0,
            simulation_dt: simulation_dt.clamp(1.0 / 240.0, 1.0 / 30.0),
        };

        runtime.apply_pending_taps(TIMELINE_TAP_EPSILON_SECONDS);
        runtime
    }

    fn apply_pending_taps(&mut self, up_to_time: f32) {
        while self.tap_index < self.tap_times.len()
            && self.tap_times[self.tap_index] <= up_to_time + TIMELINE_TAP_EPSILON_SECONDS
        {
            self.game.turn_right();
            self.tap_index += 1;
        }
    }

    fn apply_trigger_hitboxes(&mut self, up_to_time: f32) {
        if !self.simulate_trigger_hitboxes || self.triggers.is_empty() {
            return;
        }

        let Some(base_objects) = self.trigger_base_objects.as_ref() else {
            return;
        };

        self.game.objects =
            trigger_transformed_objects_at_time(base_objects, &self.triggers, up_to_time.max(0.0));
        self.game.rebuild_behavior_cache();
    }

    fn sync_trigger_base_with_consumed(&mut self) {
        if !self.simulate_trigger_hitboxes {
            return;
        }

        let consumed_indices = self.game.take_consumed_object_indices();
        if consumed_indices.is_empty() {
            return;
        }

        let Some(base_objects) = self.trigger_base_objects.as_mut() else {
            return;
        };

        let mut indices = consumed_indices;
        indices.sort_unstable();
        indices.dedup();
        for index in indices.into_iter().rev() {
            if index < base_objects.len() {
                base_objects.remove(index);
            }
        }
    }

    pub(crate) fn advance_to(&mut self, target_time_seconds: f32) {
        let mut elapsed_seconds = self.elapsed_seconds;
        advance_simulation_time(
            &mut elapsed_seconds,
            target_time_seconds,
            self.simulation_dt,
            |step_target, step_dt| {
                self.apply_pending_taps(step_target);
                self.apply_trigger_hitboxes(step_target);
                self.game.update(step_dt);
                self.sync_trigger_base_with_consumed();
                !self.game.game_over
            },
        );
        self.elapsed_seconds = elapsed_seconds;
    }

    pub(crate) fn elapsed_seconds(&self) -> f32 {
        self.elapsed_seconds
    }

    pub(crate) fn position(&self) -> [f32; 3] {
        self.game.position
    }

    pub(crate) fn direction(&self) -> Direction {
        self.game.direction
    }

    pub(crate) fn is_grounded(&self) -> bool {
        self.game.is_grounded
    }

    pub(crate) fn game_over(&self) -> bool {
        self.game.game_over
    }

    pub(crate) fn trail_segments(&self) -> &[Vec<[f32; 3]>] {
        &self.game.trail_segments
    }

    pub(crate) fn snapshot(&self) -> TimelineSimulationState {
        TimelineSimulationState {
            position: self.game.position,
            direction: match self.game.direction {
                Direction::Forward => SpawnDirection::Forward,
                Direction::Right => SpawnDirection::Right,
            },
            elapsed_seconds: self.elapsed_seconds,
            speed: self.game.speed,
        }
    }
}

pub(crate) fn simulate_timeline_state(
    spawn_position: [f32; 3],
    spawn_direction: SpawnDirection,
    objects: &[LevelObject],
    tap_times: &[f32],
    timeline_time_seconds: f32,
) -> TimelineSimulationState {
    let mut runtime =
        TimelineSimulationRuntime::new(spawn_position, spawn_direction, objects, tap_times);
    runtime.advance_to(timeline_time_seconds);
    runtime.snapshot()
}

pub(crate) fn simulate_timeline_state_with_triggers(
    spawn_position: [f32; 3],
    spawn_direction: SpawnDirection,
    objects: &[LevelObject],
    tap_times: &[f32],
    triggers: &[TimedTrigger],
    simulate_trigger_hitboxes: bool,
    timeline_time_seconds: f32,
) -> TimelineSimulationState {
    let mut runtime = TimelineSimulationRuntime::new_with_triggers(
        spawn_position,
        spawn_direction,
        objects,
        tap_times,
        triggers,
        simulate_trigger_hitboxes,
    );
    runtime.advance_to(timeline_time_seconds);
    runtime.snapshot()
}

pub(crate) fn advance_simulation_time<F>(
    elapsed_seconds: &mut f32,
    target_time_seconds: f32,
    simulation_dt: f32,
    mut step: F,
) where
    F: FnMut(f32, f32) -> bool,
{
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

pub(crate) fn trigger_transformed_objects_at_time(
    base_objects: &[LevelObject],
    triggers: &[TimedTrigger],
    time_seconds: f32,
) -> Vec<LevelObject> {
    apply_timed_triggers_to_objects(base_objects, triggers, time_seconds)
}
