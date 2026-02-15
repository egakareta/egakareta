use super::state::GameState;
use crate::types::{Direction, LevelObject, SpawnDirection};

pub(crate) struct TimelineSimulationState {
    pub(crate) position: [f32; 3],
    pub(crate) direction: SpawnDirection,
    pub(crate) elapsed_seconds: f32,
}

pub(crate) struct TimelineSimulationRuntime {
    game: GameState,
    tap_times: Vec<f32>,
    tap_index: usize,
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

    pub(crate) fn new_with_dt(
        spawn_position: [f32; 3],
        spawn_direction: SpawnDirection,
        objects: &[LevelObject],
        tap_times: &[f32],
        simulation_dt: f32,
    ) -> Self {
        let mut game = GameState::new();
        game.objects = objects.to_vec();
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
        while self.tap_index < self.tap_times.len()
            && self.tap_times[self.tap_index] <= up_to_time + TIMELINE_TAP_EPSILON_SECONDS
        {
            self.game.turn_right();
            self.tap_index += 1;
        }
    }

    pub(crate) fn advance_to(&mut self, target_time_seconds: f32) {
        let target_time = target_time_seconds.max(0.0);
        if target_time <= self.elapsed_seconds {
            return;
        }

        while self.elapsed_seconds + self.simulation_dt <= target_time {
            let step_target = self.elapsed_seconds + self.simulation_dt;
            self.apply_pending_taps(step_target);
            self.game.update(self.simulation_dt);
            self.elapsed_seconds = step_target;
            if self.game.game_over {
                return;
            }
        }

        let remaining = target_time - self.elapsed_seconds;
        if remaining > 1e-6 {
            let step_target = self.elapsed_seconds + remaining;
            self.apply_pending_taps(step_target);
            self.game.update(remaining);
            self.elapsed_seconds = step_target;
        }
    }

    pub(crate) fn elapsed_seconds(&self) -> f32 {
        self.elapsed_seconds
    }

    pub(crate) fn snapshot(&self) -> TimelineSimulationState {
        TimelineSimulationState {
            position: self.game.position,
            direction: match self.game.direction {
                Direction::Forward => SpawnDirection::Forward,
                Direction::Right => SpawnDirection::Right,
            },
            elapsed_seconds: self.elapsed_seconds,
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
