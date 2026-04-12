/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::iter;

use egui_wgpu::{Renderer as EguiRenderer, ScreenDescriptor};
use wgpu::{SurfaceError, TextureViewDescriptor};

use super::super::State;
use crate::types::{AppPhase, EditorMode};

fn linear_to_srgb(linear: f32) -> f32 {
    if linear <= 0.0031308 {
        linear * 12.92
    } else {
        linear.powf(1.0 / 2.4) * 1.055 - 0.055
    }
}

fn clear_color_for_phase(phase: AppPhase, game_over: bool) -> wgpu::Color {
    match phase {
        AppPhase::Playing if game_over => wgpu::Color {
            r: 0.15,
            g: 0.05,
            b: 0.05,
            a: 1.0,
        },
        AppPhase::Editor => wgpu::Color {
            r: 0.04,
            g: 0.07,
            b: 0.09,
            a: 1.0,
        },
        _ => wgpu::Color {
            r: 0.05,
            g: 0.05,
            b: 0.08,
            a: 1.0,
        },
    }
}

fn apply_gamma_correction_if_enabled(
    color: wgpu::Color,
    apply_gamma_correction: bool,
) -> wgpu::Color {
    if apply_gamma_correction {
        wgpu::Color {
            r: linear_to_srgb(color.r as f32) as f64,
            g: linear_to_srgb(color.g as f32) as f64,
            b: linear_to_srgb(color.b as f32) as f64,
            a: color.a,
        }
    } else {
        color
    }
}

fn should_draw_floor_and_grid(phase: AppPhase, mode: EditorMode) -> bool {
    phase != AppPhase::Menu && mode != EditorMode::Timing
}

fn should_skip_world(phase: AppPhase, mode: EditorMode) -> bool {
    phase == AppPhase::Editor && mode == EditorMode::Timing
}

fn should_draw_editor_overlays(phase: AppPhase, skip_world: bool) -> bool {
    phase == AppPhase::Editor && !skip_world
}

fn should_draw_editor_cursor(mode: EditorMode) -> bool {
    mode == EditorMode::Place
}

impl State {
    /// Renders the `egui` user interface over the current frame.
    ///
    /// This should be called after the main scene rendering to ensure the UI
    /// appears on top. It handles updating `egui` buffers and executing the
    /// render pass.
    pub fn render_egui(
        &mut self,
        renderer: &mut EguiRenderer,
        paint_jobs: &[egui::ClippedPrimitive],
        screen_descriptor: &ScreenDescriptor,
    ) -> Result<(), SurfaceError> {
        self.render_with_overlay(|device, queue, view, encoder| {
            renderer.update_buffers(device, queue, encoder, paint_jobs, screen_descriptor);

            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("egui_render_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                })
                .forget_lifetime();

            renderer.render(&mut pass, paint_jobs, screen_descriptor);
        })
    }

    /// Creates a new `EguiRenderer` configured for the current GPU device and surface format.
    pub fn create_egui_renderer(&self) -> EguiRenderer {
        EguiRenderer::new(
            &self.render.gpu.device,
            self.render.gpu.config.format,
            egui_wgpu::RendererOptions::default(),
        )
    }

    /// Performs a full render of the current application state.
    ///
    /// This method clears the surface and draws the active scene (Menu, Editor, or Gameplay).
    pub fn render(&mut self) -> Result<(), SurfaceError> {
        self.render_with_overlay(|_, _, _, _| {})
    }

    /// Renders the current scene with an additional custom overlay pass.
    ///
    /// The `overlay` closure is provided with the GPU device, queue, current texture view,
    /// and a command encoder to perform additional drawing operations.
    pub fn render_with_overlay<F>(&mut self, overlay: F) -> Result<(), SurfaceError>
    where
        F: FnOnce(&wgpu::Device, &wgpu::Queue, &wgpu::TextureView, &mut wgpu::CommandEncoder),
    {
        let surface = match &self.render.gpu.surface {
            Some(s) => s,
            None => return Ok(()),
        };
        let output = surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&TextureViewDescriptor::default());

        let mut encoder =
            self.render
                .gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Encoder"),
                });

        self.render_to_view(&view, &mut encoder, overlay);

        self.render.gpu.queue.submit(iter::once(encoder.finish()));
        output.present();
        Ok(())
    }

    pub(crate) fn render_to_view<F>(
        &mut self,
        view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
        overlay: F,
    ) where
        F: FnOnce(&wgpu::Device, &wgpu::Queue, &wgpu::TextureView, &mut wgpu::CommandEncoder),
    {
        let editor_mode = self.editor.ui.mode;
        let clear_color = apply_gamma_correction_if_enabled(
            clear_color_for_phase(self.phase, self.gameplay.state.game_over),
            self.render.gpu.apply_gamma_correction,
        );

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear_color),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.render.gpu.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        render_pass.set_pipeline(&self.render.gpu.render_pipeline);
        render_pass.set_bind_group(0, &self.render.gpu.camera_bind_group, &[]);
        render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
        render_pass.set_bind_group(2, &self.render.gpu.color_space_bind_group, &[]);
        render_pass.set_bind_group(3, &self.render.gpu.block_texture_bind_group, &[]);

        if should_draw_floor_and_grid(self.phase, editor_mode) {
            if let Some((buffer, count)) = self.render.meshes.floor.draw_data() {
                render_pass.set_vertex_buffer(0, buffer.slice(..));
                render_pass.draw(0..count, 0..1);
            }

            if let Some((buffer, count)) = self.render.meshes.grid.draw_data() {
                render_pass.set_vertex_buffer(0, buffer.slice(..));
                render_pass.draw(0..count, 0..1);
            }
        }

        if self.phase == AppPhase::Playing
            || self.phase == AppPhase::GameOver
            || self.phase == AppPhase::Editor
            || self.phase == AppPhase::Menu
        {
            let skip_world = should_skip_world(self.phase, editor_mode);

            if !skip_world {
                if self.phase == AppPhase::Editor {
                    if let Some((buffer, count)) = self.render.meshes.blocks_static.draw_data() {
                        render_pass.set_vertex_buffer(0, buffer.slice(..));
                        render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                        render_pass.draw(0..count, 0..1);
                    }

                    if let Some((buffer, count)) = self.render.meshes.blocks_selected.draw_data() {
                        render_pass.set_vertex_buffer(0, buffer.slice(..));
                        render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                        render_pass.draw(0..count, 0..1);
                    }
                } else if let Some((buffer, count)) = self.render.meshes.blocks.draw_data() {
                    render_pass.set_vertex_buffer(0, buffer.slice(..));
                    render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                    render_pass.draw(0..count, 0..1);
                }

                if let Some((buffer, count)) = self.render.meshes.trail.draw_data() {
                    if self.phase == AppPhase::Editor {
                        render_pass.set_pipeline(&self.render.gpu.editor_ghost_trail_pipeline);
                        render_pass.set_bind_group(0, &self.render.gpu.camera_bind_group, &[]);
                        render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                        render_pass.set_bind_group(2, &self.render.gpu.color_space_bind_group, &[]);
                        render_pass.set_bind_group(
                            3,
                            &self.render.gpu.block_texture_bind_group,
                            &[],
                        );
                    }

                    render_pass.set_vertex_buffer(0, buffer.slice(..));
                    render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                    render_pass.draw(0..count, 0..1);

                    if self.phase == AppPhase::Editor {
                        render_pass.set_pipeline(&self.render.gpu.render_pipeline);
                        render_pass.set_bind_group(0, &self.render.gpu.camera_bind_group, &[]);
                        render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                        render_pass.set_bind_group(2, &self.render.gpu.color_space_bind_group, &[]);
                        render_pass.set_bind_group(
                            3,
                            &self.render.gpu.block_texture_bind_group,
                            &[],
                        );
                    }
                }
            }

            if should_draw_editor_overlays(self.phase, skip_world) {
                if let Some((buffer, count)) = self.render.meshes.spawn_marker.draw_data() {
                    render_pass.set_vertex_buffer(0, buffer.slice(..));
                    render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                    render_pass.draw(0..count, 0..1);
                }

                if let Some((buffer, count)) = self.render.meshes.camera_trigger_markers.draw_data()
                {
                    render_pass.set_vertex_buffer(0, buffer.slice(..));
                    render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                    render_pass.draw(0..count, 0..1);
                }

                if let Some((buffer, count)) = self.render.meshes.tap_indicators.draw_data() {
                    render_pass.set_vertex_buffer(0, buffer.slice(..));
                    render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                    render_pass.draw(0..count, 0..1);
                }

                if let Some((buffer, count)) =
                    self.render.meshes.editor_selection_outline.draw_data()
                {
                    render_pass.set_vertex_buffer(0, buffer.slice(..));
                    render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                    render_pass.draw(0..count, 0..1);
                }

                if let Some((buffer, count)) = self.render.meshes.editor_hover_outline.draw_data() {
                    render_pass.set_vertex_buffer(0, buffer.slice(..));
                    render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                    render_pass.draw(0..count, 0..1);
                }

                if let Some((buffer, count)) = self.render.meshes.editor_gizmo.draw_data() {
                    render_pass.set_pipeline(&self.render.gpu.gizmo_overlay_pipeline);
                    render_pass.set_bind_group(0, &self.render.gpu.camera_bind_group, &[]);
                    render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                    render_pass.set_bind_group(2, &self.render.gpu.color_space_bind_group, &[]);
                    render_pass.set_bind_group(3, &self.render.gpu.block_texture_bind_group, &[]);
                    render_pass.set_vertex_buffer(0, buffer.slice(..));
                    render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                    render_pass.draw(0..count, 0..1);

                    render_pass.set_pipeline(&self.render.gpu.render_pipeline);
                    render_pass.set_bind_group(0, &self.render.gpu.camera_bind_group, &[]);
                    render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                    render_pass.set_bind_group(2, &self.render.gpu.color_space_bind_group, &[]);
                    render_pass.set_bind_group(3, &self.render.gpu.block_texture_bind_group, &[]);
                }

                if should_draw_editor_cursor(editor_mode) {
                    if let Some((buffer, count)) = self.render.meshes.editor_cursor.draw_data() {
                        render_pass.set_vertex_buffer(0, buffer.slice(..));
                        render_pass.set_bind_group(1, &self.render.gpu.zero_line_bind_group, &[]);
                        render_pass.draw(0..count, 0..1);
                    }
                }
            }
        }

        drop(render_pass);

        overlay(
            &self.render.gpu.device,
            &self.render.gpu.queue,
            view,
            encoder,
        );
    }

    /// Recreates the window surface following a resize or other configuration change.
    pub fn recreate_surface(&mut self) {
        let size = self.render.gpu.current_size();
        self.resize_surface(size);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use egui_wgpu::ScreenDescriptor;
    use wgpu::Color;

    use super::super::MeshSlot;
    use super::{
        apply_gamma_correction_if_enabled, clear_color_for_phase, linear_to_srgb,
        should_draw_editor_cursor, should_draw_editor_overlays, should_draw_floor_and_grid,
        should_skip_world,
    };
    use crate::state::State;
    use crate::types::{AppPhase, EditorMode, Vertex};

    fn approx_eq(a: f32, b: f32, eps: f32) {
        assert!(
            (a - b).abs() <= eps,
            "expected {a} to be within {eps} of {b}"
        );
    }

    #[test]
    fn linear_to_srgb_uses_linear_segment_at_or_below_threshold() {
        let below = 0.002;
        let at_threshold = 0.0031308;

        approx_eq(linear_to_srgb(below), below * 12.92, 1e-7);
        approx_eq(linear_to_srgb(at_threshold), at_threshold * 12.92, 1e-7);
    }

    #[test]
    fn linear_to_srgb_uses_gamma_segment_above_threshold() {
        let mid_gray_linear = 0.18;
        let expected = 0.461_356_1;

        approx_eq(linear_to_srgb(mid_gray_linear), expected, 1e-6);
    }

    #[test]
    fn clear_color_for_phase_matches_expected_palette() {
        assert_eq!(
            clear_color_for_phase(AppPhase::Playing, true),
            Color {
                r: 0.15,
                g: 0.05,
                b: 0.05,
                a: 1.0,
            }
        );

        assert_eq!(
            clear_color_for_phase(AppPhase::Editor, false),
            Color {
                r: 0.04,
                g: 0.07,
                b: 0.09,
                a: 1.0,
            }
        );

        assert_eq!(
            clear_color_for_phase(AppPhase::Playing, false),
            Color {
                r: 0.05,
                g: 0.05,
                b: 0.08,
                a: 1.0,
            }
        );

        assert_eq!(
            clear_color_for_phase(AppPhase::Menu, false),
            Color {
                r: 0.05,
                g: 0.05,
                b: 0.08,
                a: 1.0,
            }
        );

        assert_eq!(
            clear_color_for_phase(AppPhase::GameOver, false),
            Color {
                r: 0.05,
                g: 0.05,
                b: 0.08,
                a: 1.0,
            }
        );
    }

    #[test]
    fn apply_gamma_correction_if_enabled_only_changes_rgb() {
        let linear = Color {
            r: 0.18,
            g: 0.04,
            b: 0.01,
            a: 0.5,
        };

        let unchanged = apply_gamma_correction_if_enabled(linear, false);
        assert_eq!(unchanged, linear);

        let corrected = apply_gamma_correction_if_enabled(linear, true);
        approx_eq(corrected.r as f32, linear_to_srgb(linear.r as f32), 1e-7);
        approx_eq(corrected.g as f32, linear_to_srgb(linear.g as f32), 1e-7);
        approx_eq(corrected.b as f32, linear_to_srgb(linear.b as f32), 1e-7);
        approx_eq(corrected.a as f32, linear.a as f32, 1e-7);
    }

    #[test]
    fn render_gate_helpers_match_phase_and_mode_rules() {
        assert!(!should_draw_floor_and_grid(
            AppPhase::Menu,
            EditorMode::Place
        ));
        assert!(!should_draw_floor_and_grid(
            AppPhase::Editor,
            EditorMode::Timing
        ));
        assert!(should_draw_floor_and_grid(
            AppPhase::Playing,
            EditorMode::Place
        ));
        assert!(should_draw_floor_and_grid(
            AppPhase::GameOver,
            EditorMode::Trigger
        ));

        assert!(should_skip_world(AppPhase::Editor, EditorMode::Timing));
        assert!(!should_skip_world(AppPhase::Editor, EditorMode::Place));
        assert!(!should_skip_world(AppPhase::Playing, EditorMode::Timing));

        assert!(should_draw_editor_overlays(AppPhase::Editor, false));
        assert!(!should_draw_editor_overlays(AppPhase::Editor, true));
        assert!(!should_draw_editor_overlays(AppPhase::Menu, false));

        assert!(should_draw_editor_cursor(EditorMode::Place));
        assert!(!should_draw_editor_cursor(EditorMode::Select));
        assert!(!should_draw_editor_cursor(EditorMode::Trigger));
        assert!(!should_draw_editor_cursor(EditorMode::Timing));
    }

    #[test]
    fn render_paths_are_safe_when_surface_is_absent() {
        pollster::block_on(async {
            let mut state = State::new_test().await;

            assert!(state.render().is_ok());

            let overlay_called = Cell::new(false);
            let render_result = state.render_with_overlay(|_, _, _, _| {
                overlay_called.set(true);
            });

            assert!(render_result.is_ok());
            assert!(
                !overlay_called.get(),
                "overlay should not run when there is no surface"
            );

            let before = (state.surface_width(), state.surface_height());
            state.recreate_surface();
            let after = (state.surface_width(), state.surface_height());
            assert_eq!(before, after);
        });
    }

    #[test]
    fn render_egui_returns_ok_when_surface_is_absent() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            let mut renderer = state.create_egui_renderer();
            let paint_jobs: Vec<egui::ClippedPrimitive> = Vec::new();
            let screen_descriptor = ScreenDescriptor {
                size_in_pixels: [state.surface_width(), state.surface_height()],
                pixels_per_point: 1.0,
            };

            let result = state.render_egui(&mut renderer, &paint_jobs, &screen_descriptor);
            assert!(result.is_ok());
        });
    }

    #[test]
    fn test_render_to_view_exercises_full_pipeline() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            let size = 64;
            state.resize_surface(crate::types::PhysicalSize {
                width: size,
                height: size,
            });

            let texture = state.device().create_texture(&wgpu::TextureDescriptor {
                label: Some("Test Render Texture"),
                size: wgpu::Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: state.render.gpu.config.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let mut encoder =
                state
                    .device()
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("Test Encoder"),
                    });

            // 1. Test Editor Phase + Place Mode
            state.phase = AppPhase::Editor;
            state.editor.ui.mode = EditorMode::Place;

            // Populate some meshes
            state.editor.objects.push(crate::types::LevelObject {
                block_id: "core/stone".to_string(),
                ..Default::default()
            });
            state.editor.ui.selected_block_indices = vec![0];
            state.sync_editor_objects();
            state.rebuild_editor_cursor_vertices();
            state.rebuild_spawn_marker_vertices();

            state.render_to_view(&view, &mut encoder, |_, _, _, _| {});

            // 2. Test Playing Phase + GameOver
            state.phase = AppPhase::Playing;
            state.gameplay.state.game_over = true;
            state.editor.timeline.playback.playing = true; // For trail
            state.render_to_view(&view, &mut encoder, |_, _, _, _| {});

            // 3. Test Timing Mode (skip world)
            state.phase = AppPhase::Editor;
            state.editor.ui.mode = EditorMode::Timing;
            state.render_to_view(&view, &mut encoder, |_, _, _, _| {});

            // 4. Test with gamma correction enabled
            state.render.gpu.apply_gamma_correction = true;
            state.render_to_view(&view, &mut encoder, |_, _, _, _| {});

            state.queue().submit(std::iter::once(encoder.finish()));
        });
    }

    #[test]
    fn render_to_view_draws_populated_mesh_slots_across_phases() {
        pollster::block_on(async {
            let mut state = State::new_test().await;
            let size = 96;
            state.resize_surface(crate::types::PhysicalSize {
                width: size,
                height: size,
            });

            let texture = state.device().create_texture(&wgpu::TextureDescriptor {
                label: Some("Branch Coverage Render Texture"),
                size: wgpu::Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: state.render.gpu.config.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            let mut encoder =
                state
                    .device()
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("Branch Coverage Encoder"),
                    });

            let tri = vec![
                Vertex::untextured([0.0, 0.0, 0.0], [1.0, 0.2, 0.2, 1.0]),
                Vertex::untextured([0.2, 0.0, 0.0], [0.2, 1.0, 0.2, 1.0]),
                Vertex::untextured([0.0, 0.2, 0.0], [0.2, 0.2, 1.0, 1.0]),
            ];

            state.render.meshes.floor = MeshSlot::from_vertices(state.device(), "floor", &tri);
            state.render.meshes.grid = MeshSlot::from_vertices(state.device(), "grid", &tri);
            state.render.meshes.trail = MeshSlot::from_vertices(state.device(), "trail", &tri);
            state.render.meshes.blocks = MeshSlot::from_vertices(state.device(), "blocks", &tri);
            state.render.meshes.blocks_static =
                MeshSlot::from_vertices(state.device(), "blocks_static", &tri);
            state.render.meshes.blocks_selected =
                MeshSlot::from_vertices(state.device(), "blocks_selected", &tri);
            state.render.meshes.editor_cursor =
                MeshSlot::from_vertices(state.device(), "cursor", &tri);
            state.render.meshes.editor_hover_outline =
                MeshSlot::from_vertices(state.device(), "hover", &tri);
            state.render.meshes.editor_selection_outline =
                MeshSlot::from_vertices(state.device(), "selection", &tri);
            state.render.meshes.editor_gizmo =
                MeshSlot::from_vertices(state.device(), "gizmo", &tri);
            state.render.meshes.tap_indicators =
                MeshSlot::from_vertices(state.device(), "taps", &tri);
            state.render.meshes.spawn_marker =
                MeshSlot::from_vertices(state.device(), "spawn", &tri);
            state.render.meshes.camera_trigger_markers =
                MeshSlot::from_vertices(state.device(), "camera_markers", &tri);

            let overlay_calls = Cell::new(0_u32);

            state.phase = AppPhase::Editor;
            state.editor.ui.mode = EditorMode::Place;
            state.render_to_view(&view, &mut encoder, |_, _, _, _| {
                overlay_calls.set(overlay_calls.get() + 1);
            });

            state.phase = AppPhase::Editor;
            state.editor.ui.mode = EditorMode::Timing;
            state.render_to_view(&view, &mut encoder, |_, _, _, _| {
                overlay_calls.set(overlay_calls.get() + 1);
            });

            state.phase = AppPhase::Playing;
            state.gameplay.state.game_over = false;
            state.render_to_view(&view, &mut encoder, |_, _, _, _| {
                overlay_calls.set(overlay_calls.get() + 1);
            });

            state.phase = AppPhase::GameOver;
            state.gameplay.state.game_over = true;
            state.render_to_view(&view, &mut encoder, |_, _, _, _| {
                overlay_calls.set(overlay_calls.get() + 1);
            });

            state.phase = AppPhase::Menu;
            state.editor.ui.mode = EditorMode::Select;
            state.render_to_view(&view, &mut encoder, |_, _, _, _| {
                overlay_calls.set(overlay_calls.get() + 1);
            });

            assert_eq!(overlay_calls.get(), 5);
            state.queue().submit(std::iter::once(encoder.finish()));
        });
    }
}
