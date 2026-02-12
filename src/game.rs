use crate::types::{Direction, LevelObject};

pub(crate) struct GameState {
    pub(crate) position: [f32; 3],
    pub(crate) direction: Direction,
    pub(crate) speed: f32,
    pub(crate) trail_segments: Vec<Vec<[f32; 3]>>,
    pub(crate) objects: Vec<LevelObject>,
    pub(crate) vertical_velocity: f32,
    pub(crate) is_grounded: bool,
    pub(crate) game_over: bool,
}

impl GameState {
    pub(crate) fn new() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            direction: Direction::Forward,
            speed: 8.0,
            trail_segments: vec![vec![[0.0, 0.0, 0.0]]],
            objects: Vec::new(),
            vertical_velocity: 0.0,
            is_grounded: true,
            game_over: false,
        }
    }

    pub(crate) fn turn_right(&mut self) {
        if self.game_over {
            return;
        }
        if self.is_grounded {
            self.push_to_active_trail(self.position);
        }
        self.direction = match self.direction {
            Direction::Forward => Direction::Right,
            Direction::Right => Direction::Forward,
        };
    }

    pub(crate) fn update(&mut self, dt: f32) {
        if self.game_over {
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

        if self.collides_with_block_body(self.position[0], self.position[1], self.position[2]) {
            self.game_over = true;
            return;
        }

        let was_grounded = self.is_grounded;
        let mut is_grounded = false;

        let support_height = self.top_surface_height_at(
            self.position[0],
            self.position[1],
            self.position[2] + SNAP_DISTANCE,
        );

        if let Some(top) = support_height {
            let close_enough = self.position[2] <= top + SNAP_DISTANCE
                && self.position[2] >= top - SNAP_DISTANCE;
            if self.vertical_velocity <= 0.0 && close_enough {
                self.position[2] = top;
                self.vertical_velocity = 0.0;
                is_grounded = true;
            } else {
                self.vertical_velocity = (self.vertical_velocity - GRAVITY * dt).max(-MAX_FALL_SPEED);
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

    fn collides_with_block_body(&self, x: f32, y: f32, z: f32) -> bool {
        const SNAKE_WIDTH: f32 = 0.8;
        const SNAKE_HEIGHT: f32 = 0.8;
        const TOLERANCE: f32 = SNAKE_WIDTH * 0.05;

        let s_min_x = x - SNAKE_WIDTH / 2.0 + TOLERANCE;
        let s_max_x = x + SNAKE_WIDTH / 2.0 - TOLERANCE;
        let s_min_y = y - SNAKE_WIDTH / 2.0 + TOLERANCE;
        let s_max_y = y + SNAKE_WIDTH / 2.0 - TOLERANCE;
        let s_min_z = z + TOLERANCE;
        let s_max_z = z + SNAKE_HEIGHT - TOLERANCE;

        for obj in &self.objects {
            let o_min_x = obj.position[0];
            let o_max_x = obj.position[0] + obj.size[0];
            let o_min_y = obj.position[1];
            let o_max_y = obj.position[1] + obj.size[1];
            let o_min_z = obj.position[2];
            let o_max_z = obj.position[2] + obj.size[2];

            if s_max_x > o_min_x
                && s_min_x < o_max_x
                && s_max_y > o_min_y
                && s_min_y < o_max_y
                && s_max_z > o_min_z
                && s_min_z < o_max_z
            {
                return true;
            }
        }

        false
    }

    pub(crate) fn top_surface_height_at(&self, x: f32, y: f32, max_z: f32) -> Option<f32> {
        let mut top_surface: Option<f32> = Some(0.0);
        for obj in &self.objects {
            let o_min_x = obj.position[0];
            let o_max_x = obj.position[0] + obj.size[0];
            let o_min_y = obj.position[1];
            let o_max_y = obj.position[1] + obj.size[1];

            if x >= o_min_x && x <= o_max_x && y >= o_min_y && y <= o_max_y {
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
