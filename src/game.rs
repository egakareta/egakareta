use crate::block_repository::{resolve_block_definition, BlockCollision};
use crate::types::{Direction, LevelObject, SpawnDirection};

pub(crate) const BASE_PLAYER_SPEED: f32 = 8.0;

fn rotate_point_around_center_2d(point: [f32; 2], center: [f32; 2], radians: f32) -> [f32; 2] {
    let sin = radians.sin();
    let cos = radians.cos();
    let dx = point[0] - center[0];
    let dy = point[1] - center[1];
    [
        center[0] + (dx * cos - dy * sin),
        center[1] + (dx * sin + dy * cos),
    ]
}

fn object_xy_contains(obj: &LevelObject, x: f32, y: f32) -> bool {
    let center = [
        obj.position[0] + obj.size[0] * 0.5,
        obj.position[1] + obj.size[1] * 0.5,
    ];
    let local = rotate_point_around_center_2d([x, y], center, -obj.rotation_degrees.to_radians());
    local[0] >= obj.position[0]
        && local[0] < obj.position[0] + obj.size[0]
        && local[1] >= obj.position[1]
        && local[1] < obj.position[1] + obj.size[1]
}

fn aabb_overlaps_object_xy(
    min_x: f32,
    max_x: f32,
    min_y: f32,
    max_y: f32,
    obj: &LevelObject,
) -> bool {
    let aabb_center_x = (min_x + max_x) * 0.5;
    let aabb_center_y = (min_y + max_y) * 0.5;
    let aabb_half_x = (max_x - min_x) * 0.5;
    let aabb_half_y = (max_y - min_y) * 0.5;

    let rect_center_x = obj.position[0] + obj.size[0] * 0.5;
    let rect_center_y = obj.position[1] + obj.size[1] * 0.5;
    let rect_half_x = obj.size[0] * 0.5;
    let rect_half_y = obj.size[1] * 0.5;

    let theta = obj.rotation_degrees.to_radians();
    let axis_u = [theta.cos(), theta.sin()];
    let axis_v = [-theta.sin(), theta.cos()];

    let axes = [[1.0, 0.0], [0.0, 1.0], axis_u, axis_v];
    for axis in axes {
        let aabb_proj_center = aabb_center_x * axis[0] + aabb_center_y * axis[1];
        let aabb_proj_radius = aabb_half_x * axis[0].abs() + aabb_half_y * axis[1].abs();

        let rect_proj_center = rect_center_x * axis[0] + rect_center_y * axis[1];
        let rect_proj_radius = rect_half_x * (axis_u[0] * axis[0] + axis_u[1] * axis[1]).abs()
            + rect_half_y * (axis_v[0] * axis[0] + axis_v[1] * axis[1]).abs();

        if (aabb_proj_center - rect_proj_center).abs() > aabb_proj_radius + rect_proj_radius {
            return false;
        }
    }

    true
}

pub(crate) struct GameState {
    pub(crate) position: [f32; 3],
    pub(crate) direction: Direction,
    pub(crate) speed: f32,
    pub(crate) trail_segments: Vec<Vec<[f32; 3]>>,
    pub(crate) objects: Vec<LevelObject>,
    pub(crate) vertical_velocity: f32,
    pub(crate) is_grounded: bool,
    pub(crate) game_over: bool,
    pub(crate) started: bool,
}

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

pub(crate) fn center_spawn_position(position: [f32; 3]) -> [f32; 3] {
    [
        position[0].floor() + 0.5,
        position[1].floor() + 0.5,
        position[2],
    ]
}

impl GameState {
    pub(crate) fn new() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            direction: Direction::Forward,
            speed: BASE_PLAYER_SPEED,
            trail_segments: vec![vec![[0.0, 0.0, 0.0]]],
            objects: Vec::new(),
            vertical_velocity: 0.0,
            is_grounded: true,
            game_over: false,
            started: false,
        }
    }

    pub(crate) fn apply_spawn(&mut self, position: [f32; 3], direction: SpawnDirection) {
        let centered_position = center_spawn_position(position);
        self.position = centered_position;
        self.direction = direction.into();
        self.speed = BASE_PLAYER_SPEED;
        self.vertical_velocity = 0.0;
        self.is_grounded = true;
        self.game_over = false;
        self.trail_segments = vec![vec![centered_position]];
    }

    pub(crate) fn turn_right(&mut self) {
        if self.game_over || !self.started || !self.is_grounded {
            return;
        }
        self.push_to_active_trail(self.position);
        self.direction = match self.direction {
            Direction::Forward => Direction::Right,
            Direction::Right => Direction::Forward,
        };
    }

    pub(crate) fn update(&mut self, dt: f32) {
        if self.game_over || !self.started {
            return;
        }

        const GRAVITY: f32 = 26.0;
        const MAX_FALL_SPEED: f32 = 40.0;
        const SNAP_DISTANCE: f32 = 0.3;
        const DEATH_Z: f32 = -6.0;

        let delta = match self.direction {
            Direction::Forward => [0.0, 1.0],
            Direction::Right => [1.0, 0.0],
        };

        self.position[0] += delta[0] * self.speed * dt;
        self.position[1] += delta[1] * self.speed * dt;

        // Collision detection
        let mut hit_death = false;
        let mut hit_portals = Vec::new();

        const SNAKE_WIDTH: f32 = 0.8;
        const SNAKE_HEIGHT: f32 = 0.8;
        const TOLERANCE: f32 = SNAKE_WIDTH * 0.05;

        let x = self.position[0];
        let y = self.position[1];
        let z = self.position[2];

        let s_min_x = x - SNAKE_WIDTH / 2.0 + TOLERANCE;
        let s_max_x = x + SNAKE_WIDTH / 2.0 - TOLERANCE;
        let s_min_y = y - SNAKE_WIDTH / 2.0 + TOLERANCE;
        let s_max_y = y + SNAKE_WIDTH / 2.0 - TOLERANCE;
        let s_min_z = z + TOLERANCE;
        let s_max_z = z + SNAKE_HEIGHT - TOLERANCE;

        for (i, obj) in self.objects.iter().enumerate() {
            let o_min_z = obj.position[2];
            let o_max_z = obj.position[2] + obj.size[2];
            let behavior = &resolve_block_definition(&obj.block_id).behavior;

            if aabb_overlaps_object_xy(s_min_x, s_max_x, s_min_y, s_max_y, obj)
                && s_max_z > o_min_z
                && s_min_z < o_max_z
            {
                match behavior.collision {
                    BlockCollision::Portal => {
                        hit_portals.push(i);
                    }
                    BlockCollision::Hazard => {
                        hit_death = true;
                    }
                    BlockCollision::Solid => {
                        hit_death = true;
                    }
                    BlockCollision::PassThrough => {}
                }
            }
        }

        if hit_death {
            self.game_over = true;
            return;
        }

        if !hit_portals.is_empty() {
            for i in hit_portals.into_iter().rev() {
                if let Some(portal) = self.objects.get(i) {
                    let behavior = &resolve_block_definition(&portal.block_id).behavior;
                    self.speed *= behavior.speed_multiplier.max(0.1);
                    if behavior.consumed_on_overlap {
                        self.objects.remove(i);
                    }
                }
            }
        }

        let was_grounded = self.is_grounded;
        let mut is_grounded = false;

        let support_height = self.top_surface_height_at(
            self.position[0],
            self.position[1],
            self.position[2] + SNAP_DISTANCE,
        );

        if let Some(top) = support_height {
            let close_enough =
                self.position[2] <= top + SNAP_DISTANCE && self.position[2] >= top - SNAP_DISTANCE;
            if self.vertical_velocity <= 0.0 && close_enough {
                self.position[2] = top;
                self.vertical_velocity = 0.0;
                is_grounded = true;
            } else {
                self.vertical_velocity =
                    (self.vertical_velocity - GRAVITY * dt).max(-MAX_FALL_SPEED);
                self.position[2] += self.vertical_velocity * dt;
            }
        } else {
            self.vertical_velocity = (self.vertical_velocity - GRAVITY * dt).max(-MAX_FALL_SPEED);
            self.position[2] += self.vertical_velocity * dt;
        }

        if was_grounded && !is_grounded {
            self.push_to_active_trail(self.position);
        } else if !was_grounded && is_grounded {
            self.start_new_trail_segment(self.position);
        }

        self.is_grounded = is_grounded;

        if self.position[2] < DEATH_Z {
            self.game_over = true;
        }
    }

    fn start_new_trail_segment(&mut self, point: [f32; 3]) {
        self.trail_segments.push(vec![point]);
    }

    fn push_to_active_trail(&mut self, point: [f32; 3]) {
        const MIN_DELTA: f32 = 0.001;
        if let Some(segment) = self.trail_segments.last_mut() {
            if let Some(last) = segment.last() {
                if (last[0] - point[0]).abs() < MIN_DELTA
                    && (last[1] - point[1]).abs() < MIN_DELTA
                    && (last[2] - point[2]).abs() < MIN_DELTA
                {
                    return;
                }
            }
            segment.push(point);
        } else {
            self.trail_segments.push(vec![point]);
        }
    }

    pub(crate) fn top_surface_height_at(&self, x: f32, y: f32, max_z: f32) -> Option<f32> {
        let mut top_surface: Option<f32> = Some(0.0);
        for obj in &self.objects {
            if !resolve_block_definition(&obj.block_id)
                .behavior
                .support_surface
            {
                continue;
            }
            if object_xy_contains(obj, x, y) {
                let top = obj.position[2] + obj.size[2];
                if top <= max_z {
                    top_surface = match top_surface {
                        Some(existing) if existing > top => Some(existing),
                        _ => Some(top),
                    };
                }
            }
        }

        top_surface
    }
}

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

pub(crate) fn create_menu_scene() -> Vec<LevelObject> {
    let mut objects = Vec::new();

    // Create a base platform
    for x in -5..6 {
        for y in -5..6 {
            let height = if (x * x + y * y) < 8 {
                0.0
            } else if (x + y) % 2 == 0 {
                -1.0
            } else {
                -2.0
            };

            objects.push(LevelObject {
                position: [x as f32 * 2.0, y as f32 * 2.0, height],
                size: [2.0, 2.0, 2.0],
                rotation_degrees: 0.0,
                roundness: 0.18,
                block_id: "core/grass".to_string(),
            });
        }
    }

    objects
}

#[cfg(test)]
mod tests {
    use super::{
        aabb_overlaps_object_xy, object_xy_contains, simulate_timeline_state, GameState,
        TimelineSimulationRuntime, BASE_PLAYER_SPEED,
    };
    use crate::types::{LevelObject, SpawnDirection};

    fn approx_eq(a: f32, b: f32, eps: f32) {
        assert!((a - b).abs() <= eps, "expected {a} ~= {b}");
    }

    #[test]
    fn test_ground_detection_normal() {
        let mut game = GameState::new();
        game.objects.push(LevelObject {
            position: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            rotation_degrees: 0.0,
            roundness: 0.18,
            block_id: "core/standard".to_string(),
        });

        // Player at 0.5, 0.5 (center of block), check ground at 0.5, 0.5
        // Max Z should be > 1.0 to detect the block top
        let height = game.top_surface_height_at(0.5, 0.5, 2.0);
        assert_eq!(height, Some(1.0));
    }

    #[test]
    fn test_ground_detection_under_overhang() {
        let mut game = GameState::new();
        // Ground block
        game.objects.push(LevelObject {
            position: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            rotation_degrees: 0.0,
            roundness: 0.18,
            block_id: "core/standard".to_string(),
        });
        // Overhang block at height 3
        game.objects.push(LevelObject {
            position: [0.0, 0.0, 3.0],
            size: [1.0, 1.0, 1.0],
            rotation_degrees: 0.0,
            roundness: 0.18,
            block_id: "core/standard".to_string(),
        });

        // Player is walking on the ground block (z=1).
        // We check ground height with max_z slightly above player head (e.g. 1.0 + SNAP)
        // It should ignore the block at z=3.
        let height = game.top_surface_height_at(0.5, 0.5, 1.5);
        assert_eq!(height, Some(1.0));
    }

    #[test]
    fn test_cant_turn_while_falling() {
        let mut game = GameState::new();
        game.started = true;
        game.is_grounded = false;
        let initial_direction = game.direction;
        game.turn_right();
        assert_eq!(game.direction, initial_direction);
    }

    #[test]
    fn test_can_turn_while_grounded() {
        let mut game = GameState::new();
        game.started = true;
        game.is_grounded = true;
        let initial_direction = game.direction;
        game.turn_right();
        assert_ne!(game.direction, initial_direction);
    }

    #[test]
    fn rotated_object_contains_expected_points() {
        let obj = LevelObject {
            position: [0.0, 0.0, 0.0],
            size: [2.0, 1.0, 1.0],
            rotation_degrees: 90.0,
            roundness: 0.18,
            block_id: "core/standard".to_string(),
        };

        assert!(object_xy_contains(&obj, 1.0, 0.5));
        assert!(!object_xy_contains(&obj, 2.1, 0.5));
    }

    #[test]
    fn rotated_overlap_uses_oriented_bounds() {
        let obj = LevelObject {
            position: [0.0, 0.0, 0.0],
            size: [2.0, 1.0, 1.0],
            rotation_degrees: 45.0,
            roundness: 0.18,
            block_id: "core/standard".to_string(),
        };

        assert!(aabb_overlaps_object_xy(0.9, 1.1, 0.3, 0.5, &obj));
        assert!(!aabb_overlaps_object_xy(3.0, 3.4, 3.0, 3.4, &obj));
    }

    #[test]
    fn rotated_ground_detection_works() {
        let mut game = GameState::new();
        game.objects.push(LevelObject {
            position: [0.0, 0.0, 0.0],
            size: [2.0, 1.0, 2.0],
            rotation_degrees: 90.0,
            roundness: 0.18,
            block_id: "core/standard".to_string(),
        });

        let inside = game.top_surface_height_at(1.0, 0.5, 3.0);
        let outside = game.top_surface_height_at(2.2, 0.5, 3.0);
        assert_eq!(inside, Some(2.0));
        assert_eq!(outside, Some(0.0));
    }

    #[test]
    fn speed_portal_overlap_removes_portal_and_boosts_speed() {
        let mut game = GameState::new();
        game.started = true;
        game.position = [0.5, 0.2, 0.0];
        game.speed = 1.0;
        game.objects.push(LevelObject {
            position: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            rotation_degrees: 30.0,
            roundness: 0.18,
            block_id: "core/speedportal".to_string(),
        });

        game.update(0.0);

        approx_eq(game.speed, 1.5, 1e-6);
        assert!(game.objects.is_empty());
        assert!(!game.game_over);
    }

    #[test]
    fn timeline_negative_time_clamps_to_zero() {
        let snapshot =
            simulate_timeline_state([0.0, 0.0, 0.0], SpawnDirection::Forward, &[], &[], -2.0);

        approx_eq(snapshot.position[0], 0.5, 1e-6);
        approx_eq(snapshot.position[1], 0.5, 1e-6);
        approx_eq(snapshot.position[2], 0.0, 1e-6);
        assert!(matches!(snapshot.direction, SpawnDirection::Forward));
        approx_eq(snapshot.elapsed_seconds, 0.0, 1e-6);
    }

    #[test]
    fn timeline_tap_at_zero_turns_before_movement() {
        let dt = 1.0 / BASE_PLAYER_SPEED;
        let snapshot =
            simulate_timeline_state([0.0, 0.0, 0.0], SpawnDirection::Forward, &[], &[0.0], dt);

        assert!(matches!(snapshot.direction, SpawnDirection::Right));
        approx_eq(snapshot.position[0], 1.5, 0.05);
        approx_eq(snapshot.position[1], 0.5, 0.05);
        approx_eq(snapshot.elapsed_seconds, dt, 1e-6);
    }

    #[test]
    fn timeline_incremental_runtime_matches_direct_simulation() {
        let mut runtime = TimelineSimulationRuntime::new(
            [0.0, 0.0, 0.0],
            SpawnDirection::Forward,
            &[],
            &[0.375, 0.125, 0.25],
        );

        for target in [0.05_f32, 0.2, 0.31, 0.6] {
            runtime.advance_to(target);
        }

        let incremental = runtime.snapshot();
        let direct = simulate_timeline_state(
            [0.0, 0.0, 0.0],
            SpawnDirection::Forward,
            &[],
            &[0.375, 0.125, 0.25],
            0.6,
        );

        approx_eq(incremental.position[0], direct.position[0], 0.02);
        approx_eq(incremental.position[1], direct.position[1], 0.02);
        approx_eq(incremental.position[2], direct.position[2], 0.02);
        assert!(matches!(
            (incremental.direction, direct.direction),
            (SpawnDirection::Forward, SpawnDirection::Forward)
                | (SpawnDirection::Right, SpawnDirection::Right)
        ));
        approx_eq(incremental.elapsed_seconds, direct.elapsed_seconds, 1e-6);
    }
}
