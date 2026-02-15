use super::physics::{aabb_overlaps_object_xy, object_xy_contains, BASE_PLAYER_SPEED};
use crate::block_repository::{resolve_block_definition, BlockCollision};
use crate::types::{Direction, LevelObject, SpawnDirection};

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
