use super::*;

impl State {
    pub fn update(&mut self) {
        self.update_audio_imports();
        const FIXED_DT: f32 = 1.0 / 120.0;

        let now = PlatformInstant::now();
        let frame_dt = (now - self.last_frame).as_secs_f32();
        self.last_frame = now;
        self.accumulator = (self.accumulator + frame_dt).min(0.25);

        if self.phase == AppPhase::Menu {
            self.accumulator = 0.0;
            self.update_menu_camera();
            return;
        }

        if self.phase == AppPhase::Editor {
            self.accumulator = 0.0;
            self.trail_mesh.clear();

            if self.editor_timeline_playing {
                self.editor_timeline_playback_accumulator += frame_dt;

                while self.editor_timeline_playback_accumulator >= EDITOR_TIMELINE_STEP_SECONDS {
                    self.editor_timeline_playback_accumulator -= EDITOR_TIMELINE_STEP_SECONDS;
                    let max_step = self.editor_timeline_length.saturating_sub(1);
                    if self.editor_timeline_step >= max_step {
                        self.editor_timeline_playing = false;
                        self.editor_timeline_playback_accumulator = 0.0;
                        self.stop_audio();
                        break;
                    }

                    self.editor_timeline_step += 1;
                    self.refresh_editor_timeline_position();
                }
            }

            self.update_editor_pan_from_keys(frame_dt);
            if self.editor_gizmo_drag.is_some() || self.editor_block_drag.is_some() {
                if let Some(pointer) = self.editor_pointer_screen {
                    self.drag_editor_selection_from_screen(pointer[0], pointer[1]);
                }
            }
            self.rebuild_editor_gizmo_vertices();
            self.update_editor_camera();
            return;
        }

        while self.accumulator >= FIXED_DT {
            self.game.update(FIXED_DT);
            self.accumulator -= FIXED_DT;
        }

        if self.game.game_over {
            self.stop_audio();
        }

        let mut trail_vertices = Vec::new();
        let player_pos = self.game.position;
        // Cull segments that are too far from the player to save on vertices.
        const CULL_DISTANCE_SQ: f32 = 120.0 * 120.0;

        for (segment_index, segment) in self.game.trail_segments.iter().enumerate() {
            if segment.is_empty() {
                continue;
            }

            let is_last_segment = segment_index + 1 == self.game.trail_segments.len();

            // For older segments, check if they are still potentially visible.
            if !is_last_segment {
                let last_point = segment.last().unwrap();
                let dx = last_point[0] - player_pos[0];
                let dy = last_point[1] - player_pos[1];
                let dz = last_point[2] - player_pos[2];
                if dx * dx + dy * dy + dz * dz > CULL_DISTANCE_SQ {
                    continue;
                }
            }

            if is_last_segment && self.game.is_grounded {
                let mut points = segment.clone();
                points.push(self.game.position);
                trail_vertices.extend(build_trail_vertices(&points, self.game.game_over));
            } else {
                trail_vertices.extend(build_trail_vertices(segment, self.game.game_over));
            }
        }

        if !self.game.is_grounded {
            let head_length = 0.22;
            let dir = match self.game.direction {
                Direction::Forward => [0.0, 1.0],
                Direction::Right => [1.0, 0.0],
            };
            let head_start = [
                self.game.position[0] - dir[0] * head_length,
                self.game.position[1] - dir[1] * head_length,
                self.game.position[2],
            ];
            let head_points = [head_start, self.game.position];
            trail_vertices.extend(build_trail_vertices(&head_points, self.game.game_over));
        }

        self.trail_mesh
            .write_streaming_vertices(&self.queue, &trail_vertices);

        self.line_uniform.offset = [
            (self.game.position[0] * 100.0).round() / 100.0,
            (self.game.position[1] * 100.0).round() / 100.0,
        ];
        self.line_uniform.rotation = match self.game.direction {
            Direction::Forward => 0.0,
            Direction::Right => -std::f32::consts::FRAC_PI_2,
        };

        self.queue.write_buffer(
            &self.line_uniform_buffer,
            0,
            bytemuck::bytes_of(&self.line_uniform),
        );

        let aspect = self.config.width as f32 / self.config.height as f32;
        let pos_3d = Vec3::new(
            self.game.position[0],
            self.game.position[1],
            self.game.position[2],
        );
        let target = pos_3d;
        let offset = self.playing_camera_offset();
        let eye = pos_3d + offset;
        let up = Vec3::new(0.0, 0.0, 1.0);
        let view = Mat4::look_at_rh(eye, target, up);
        let proj = Mat4::perspective_rh_gl(45f32.to_radians(), aspect, 0.1, 1000.0);
        let view_proj = proj * view;
        let camera_uniform = CameraUniform {
            view_proj: view_proj.to_cols_array_2d(),
        };

        self.queue.write_buffer(
            &self.camera_uniform_buffer,
            0,
            bytemuck::bytes_of(&camera_uniform),
        );
    }

    fn update_menu_camera(&mut self) {
        let aspect = self.config.width as f32 / self.config.height as f32;
        let radius = 25.0;
        let angle = -25.0f32.to_radians();
        let eye = Vec3::new(radius * angle.cos(), radius * angle.sin(), 15.0);
        let target = Vec3::new(0.0, 0.0, 0.0);
        let up = Vec3::new(0.0, 0.0, 1.0);
        let view = Mat4::look_at_rh(eye, target, up);
        let proj = Mat4::perspective_rh_gl(45f32.to_radians(), aspect, 0.1, 1000.0);
        let view_proj = proj * view;
        let camera_uniform = CameraUniform {
            view_proj: view_proj.to_cols_array_2d(),
        };

        self.queue.write_buffer(
            &self.camera_uniform_buffer,
            0,
            bytemuck::bytes_of(&camera_uniform),
        );
    }

    fn update_editor_camera(&mut self) {
        let aspect = self.config.width as f32 / self.config.height as f32;
        let target = Vec3::new(self.editor_camera_pan[0], self.editor_camera_pan[1], 0.0);
        let offset = self.editor_camera_offset();
        let eye = target + offset;
        let up = Vec3::new(0.0, 0.0, 1.0);
        let view = Mat4::look_at_rh(eye, target, up);
        let proj = Mat4::perspective_rh_gl(45f32.to_radians(), aspect, 0.1, 1000.0);
        let view_proj = proj * view;
        let camera_uniform = CameraUniform {
            view_proj: view_proj.to_cols_array_2d(),
        };

        self.queue.write_buffer(
            &self.camera_uniform_buffer,
            0,
            bytemuck::bytes_of(&camera_uniform),
        );
    }
}
