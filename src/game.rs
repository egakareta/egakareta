use crate::types::{Direction, LevelObject};

pub(crate) struct GameState {
    pub(crate) position: [f32; 2],
    pub(crate) direction: Direction,
    pub(crate) speed: f32,
    pub(crate) trail: Vec<[f32; 2]>,
    pub(crate) objects: Vec<LevelObject>,
    pub(crate) game_over: bool,
}

impl GameState {
    pub(crate) fn new() -> Self {
        Self {
            position: [0.0, 0.0],
            direction: Direction::Forward,
            speed: 8.0,
            trail: vec![[0.0, 0.0]],
            objects: Vec::new(),
            game_over: false,
        }
    }

    pub(crate) fn turn_right(&mut self) {
        if self.game_over {
            return;
        }
        self.trail.push(self.position);
        self.direction = match self.direction {
            Direction::Forward => Direction::Right,
            Direction::Right => Direction::Forward,
        };
    }

    pub(crate) fn update(&mut self, dt: f32) {
        if self.game_over {
            return;
        }
        let delta = match self.direction {
            Direction::Forward => [0.0, 1.0],
            Direction::Right => [1.0, 0.0],
        };

        self.position[0] += delta[0] * self.speed * dt;
        self.position[1] += delta[1] * self.speed * dt;

        let col_size = 0.4;

        for obj in &self.objects {
            let p_min = [self.position[0] - col_size, self.position[1] - col_size];
            let p_max = [self.position[0] + col_size, self.position[1] + col_size];
            let o_min = [obj.position[0], obj.position[1]];
            let o_max = [obj.position[0] + obj.size[0], obj.position[1] + obj.size[1]];

            if p_max[0] >= o_min[0]
                && p_min[0] <= o_max[0]
                && p_max[1] >= o_min[1]
                && p_min[1] <= o_max[1]
            {
                self.game_over = true;
            }
        }
    }
}
