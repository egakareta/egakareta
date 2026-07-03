/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::load_menu_textures;
use crate::platform::pipeline::FramePipeline;
use bevy_app::App;
use bevy_ecs::world::World;
use eg_state::events::PendingCommands;
use eg_state::state::AppPhaseRes;
use eg_state::state::AuthOpenSignupRequest;
use eg_state::state::AuthSignOutRequest;
use eg_state::state::AuthSubmitSignInRequest;
use eg_state::state::EditorConfigRes;
use eg_state::state::EditorRuntimeRes;
use eg_state::state::EditorTimelineRes;
use eg_state::state::EditorTimingRes;
use eg_state::state::EscapeRequest;
use eg_state::state::MenuStateRes;
use eg_state::state::PerfFrameCopyRequest;
use eg_state::state::RenderResizeRequest;
use eg_state::state::SessionStateRes;
use eg_state::state::State;
use eg_state::state::enqueue_platform_input;
use eg_state::state::register_state_command_pipeline;
use eg_state::state::run_state_schedule_in_world;
use eg_state::state::setup_state_app;
use eg_state::state::{EditorHistoryRes, EditorInputRes, EditorObjectsRes};
use eg_ui::{
    drain_ui_command_queue, enqueue_ui_commands, register_ui_resources_world, sync_egui_context,
};

const SORA_TTF: &[u8] = include_bytes!(concat!(
    env!("EGAKARETA_WORKSPACE_ROOT"),
    "/assets/Sora.ttf"
));

/// Runtime owns the winit-facing frame loop.
///
/// Phase 7: alongside the legacy `state: State` and
/// `pipeline: FramePipeline` fields, the runtime owns a
/// `bevy_app::App` wired with the eg-state plugin graph (see
/// [`setup_state_app`]). The winit redraw path now calls
/// [`Runtime::run_app_frame`], which owns the App update and command
/// drain before handing the frame back to the compatibility renderer.
///
/// The App world is the production command/schedule driver during the
/// compatibility window. Legacy `State` fields still own render/input data
/// until those resources move fully into the App.
///
/// REFACTOR POINT(full-bevy): when full Bevy lands, the `state`
/// and `pipeline` fields disappear; the `App` owns the egui
/// context + render resources and the winit loop calls
/// `runtime.app.update()` directly.
pub struct Runtime {
    pub state: State,
    pub pipeline: FramePipeline,
    pub app: App,
}

impl Runtime {
    pub fn new(state: State) -> Self {
        let egui_ctx = egui::Context::default();

        let mut fonts = egui::FontDefinitions::default();
        fonts.font_data.insert(
            "sora".to_owned(),
            std::sync::Arc::new(egui::FontData::from_static(SORA_TTF)),
        );
        fonts.font_data.insert(
            "sora_thin".to_owned(),
            std::sync::Arc::new(
                egui::FontData::from_static(SORA_TTF).tweak(egui::FontTweak {
                    coords: egui::epaint::text::VariationCoords::new([(b"wght", 100.0)]),
                    ..Default::default()
                }),
            ),
        );
        fonts
            .families
            .entry(egui::FontFamily::Name("sora_thin".into()))
            .or_default()
            .insert(0, "sora_thin".to_owned());
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "sora".to_owned());
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .insert(0, "sora".to_owned());

        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        egui_ctx.set_fonts(fonts);

        let egui_renderer = state.create_egui_renderer();
        let menu_textures = load_menu_textures(&egui_ctx);
        let pipeline = FramePipeline::new(egui_ctx, egui_renderer, menu_textures);

        // Build the Bevy `App` with the eg-state plugin graph. The
        // App owns the single ECS `World`; legacy `State` fields are
        // bridged into it each frame during the compatibility window.
        let mut app = App::new();
        setup_state_app(&mut app);
        register_state_command_pipeline(app.world_mut());
        register_ui_resources_world(app.world_mut());

        // Seed `AppPhaseRes` from the legacy `State.phase`.
        // `StatePlugins` default this resource to Menu, but tests and
        // transition paths can construct Runtime from an already-Editor
        // or already-Playing legacy state. Handlers that are phase
        // guarded (for example menu-preview camera capture) need the
        // App-world phase to match on the first frame.
        app.world_mut().insert_resource(state.app_phase_resource());
        app.world_mut().insert_resource(state.menu_state_resource());

        // Seed `EditorConfigRes` from the legacy `State` so the
        // resource starts in sync with user-loaded `app_settings`
        // (snap config, selected block id, recent blocks). Without
        // this, `EditorPlugin` Default-initializes the resource and
        // the change-guarded `sync_app_editor_config_state` below
        // would clobber user settings on the first frame: the
        // resource default differs from the legacy user value, the
        // setter fires, and the user value is overwritten and
        // re-persisted. Seeding once at construction makes the first
        // frame a true no-op for unchanged fields.
        app.world_mut()
            .insert_resource(state.editor_config_resource());

        // Seed `EditorInputRes` from the legacy `State` so direct
        // input handlers (including the Phase 3.27 editor
        // next/previous cursor movement carve-out) start from the same
        // cursor/mode/modifier state as the compatibility path.
        app.world_mut()
            .insert_resource(state.editor_input_resource());
        app.world_mut()
            .insert_resource(state.editor_objects_resource());

        // Seed `EditorTimingRes` from the legacy `State` for the
        // bpm-tap carve-out (see `events::bpm`). The resource's
        // bpm_tap_times/bpm_tap_result would otherwise be
        // Default-initialized (empty list, no result) on the first
        // frame; the change-guarded `sync_app_editor_timing_state`
        // would then see a diff vs the legacy state, fire the
        // setter, and clobber any in-flight tap history. Seeding
        // once at construction makes the first frame a no-op for
        // unchanged fields.
        app.world_mut()
            .insert_resource(state.editor_timing_resource());

        // Seed `EditorTimelineRes` from the legacy `State` for the
        // timeline carve-outs (see `events::timeline`). Without
        // seeding, the change-guarded `sync_app_editor_timeline_state`
        // would see a default-vs-legacy diff on the first frame and
        // clobber user timeline position/duration or tap selection.
        app.world_mut()
            .insert_resource(state.editor_timeline_resource());
        app.world_mut()
            .insert_resource(state.editor_runtime_resource());
        app.world_mut()
            .insert_resource(state.editor_history_resource());

        // Seed the carved-out `SessionStateRes` fields from the
        // legacy `State.session` (see `events::session`). The
        // carved-out fields (`editor_show_metadata`,
        // `editor_show_settings`, `editor_settings_section`,
        // `editor_keybind_capture_action`) are written to the
        // resource directly by their Bevy handlers; the
        // change-guarded `sync_app_session_state` would otherwise
        // see a default-vs-legacy diff on the first frame and
        // clobber any user-driven UI-window state. Seeding once
        // at construction makes the first frame a no-op for
        // unchanged fields.
        app.world_mut()
            .insert_resource(state.session_state_resource());

        // Seed `EditorSpawnRes` from the legacy `State.editor.spawn`
        // for the spawn carve-out (see `events::spawn`). Without
        // seeding, the change-guarded
        // `sync_app_editor_spawn_state` would see a
        // `SpawnMetadata::default()`-vs-legacy diff on the first
        // frame and clobber any user-set spawn position/direction.
        // Seeding once at construction makes the first frame a
        // no-op for unchanged fields.
        app.world_mut()
            .insert_resource(state.editor_spawn_resource());

        // Seed `EditorCameraRes` from the legacy `State.editor.camera`
        // for the camera carve-out (see `events::camera`). Without
        // seeding, the change-guarded
        // `sync_app_editor_camera_state` would see a default-vs-legacy
        // diff on the first frame and clobber any user-set camera
        // position / yaw / pitch. Seeding once at construction makes
        // the first frame a no-op for unchanged fields.
        app.world_mut()
            .insert_resource(state.editor_camera_resource());

        // Seed `EditorTriggerRes` from the legacy
        // `State.editor.triggers` for the trigger-selected carve-out
        // (see `events::camera::handle_set_trigger_selected`).
        app.world_mut()
            .insert_resource(state.editor_trigger_resource());

        // Seed `PerfRes` from the legacy `State.editor.perf` for the
        // perf-overlay carve-out (see `events::misc`). Without
        // seeding, the change-guarded `sync_app_perf_state` would
        // see a default-vs-legacy diff on the first frame and
        // re-invoke the legacy `set_perf_overlay_enabled` setter
        // (which calls `puffin::set_scopes_on(...)` — a global side
        // effect). Seeding once at construction makes the first
        // frame a no-op for unchanged fields.
        app.world_mut().insert_resource(state.perf_resource());

        Self {
            state,
            pipeline,
            app,
        }
    }

    /// Drive one frame through the compatibility `FramePipeline` while
    /// routing UI-originated messages through Bevy's `App` world.
    pub fn run_app_frame(&mut self, raw_input: egui::RawInput) -> egui::FullOutput {
        let Runtime {
            state,
            pipeline,
            app,
        } = self;
        sync_egui_context(app.world_mut(), pipeline.ctx());

        let output = pipeline.run_frame(state, raw_input, |state, ui_commands| {
            enqueue_ui_commands(app.world_mut(), ui_commands);
            drain_ui_command_queue(app.world_mut());
            app.update();
            state.prepare_app_world_for_state_schedule(app.world_mut());
            run_state_schedule_in_world(app.world_mut());
            Self::sync_app_editor_history_state(state, app.world());
            Self::sync_app_editor_input_state(state, app.world());
            Self::sync_app_editor_config_state(state, app.world());
            Self::sync_app_editor_timeline_state(state, app.world_mut());
            Self::sync_app_editor_runtime_state(state, app.world());
            Self::sync_app_editor_timing_state(state, app.world());
            Self::sync_app_editor_spawn_state(state, app.world());
            Self::sync_app_editor_camera_state(state, app.world());
            Self::sync_app_editor_trigger_state(state, app.world());
            Self::sync_app_editor_objects_state(state, app.world_mut());
            Self::sync_app_menu_state(state, app.world());
            Self::sync_app_session_state(state, app.world());
            Self::sync_app_perf_state(state, app.world());
            Self::sync_app_perf_frame_copy_request(state, app.world_mut());
            Self::sync_app_render_resize_request(state, app.world_mut());
            Self::sync_app_escape_request(state, app.world_mut());
            Self::sync_app_auth_submit_sign_in_request(state, app.world_mut());
            Self::sync_app_auth_sign_out_request(state, app.world_mut());
            Self::sync_app_auth_open_signup_request(state, app.world_mut());
            Self::dispatch_app_pending_commands(state, app.world_mut());
            state.drain_pending_async_results();
        });
        Self::refresh_editor_playback_mirrors_from_legacy(state, app.world_mut());
        output
    }

    fn refresh_editor_playback_mirrors_from_legacy(state: &State, world: &mut World) {
        world.insert_resource(state.editor_timeline_resource());
        world.insert_resource(state.editor_runtime_resource());
        world.insert_resource(state.editor_input_resource());
    }

    fn sync_app_editor_history_state(state: &mut State, world: &World) {
        state.set_editor_history_state(world.resource::<EditorHistoryRes>().0.clone());
    }

    fn sync_app_editor_runtime_state(state: &mut State, world: &World) {
        state.set_editor_clipboard_from_app(
            world
                .resource::<EditorRuntimeRes>()
                .0
                .interaction
                .clipboard
                .clone(),
        );
    }

    fn sync_app_editor_objects_state(state: &mut State, world: &mut World) {
        let (
            objects,
            selected_block_index,
            selected_block_indices,
            hovered_block_index,
            history_restore_pending,
            object_mutation_pending,
        ) = {
            let objects = world.resource::<EditorObjectsRes>();
            let input = &world.resource::<EditorInputRes>().0;
            (
                objects.objects.clone(),
                input.selected_block_index,
                input.selected_block_indices.clone(),
                input.hovered_block_index,
                objects.history_restore_pending,
                objects.object_mutation_pending,
            )
        };
        if state.editor_objects() != objects.as_slice() {
            state.set_editor_objects_from_app(objects);
        }
        if history_restore_pending || object_mutation_pending {
            state.set_editor_block_selection_from_app(
                selected_block_index,
                selected_block_indices,
                hovered_block_index,
            );
        }
        if history_restore_pending {
            state.sync_editor_scene_after_history_restore();
        } else if object_mutation_pending {
            state.sync_editor_scene_after_app_object_mutation();
        }
        if history_restore_pending || object_mutation_pending {
            let mut objects = world.resource_mut::<EditorObjectsRes>();
            if objects.objects.as_slice() != state.editor_objects() {
                objects.objects = state.editor_objects().to_vec();
            }
            objects.history_restore_pending = false;
            objects.object_mutation_pending = false;
        }
    }

    fn sync_app_editor_input_state(state: &mut State, world: &World) {
        let input = &world.resource::<EditorInputRes>().0;
        let shift_held = input.shift_held;
        let ctrl_held = input.ctrl_held;
        let alt_held = input.alt_held;
        let pan_up_held = input.pan_up_held;
        let pan_down_held = input.pan_down_held;
        let pan_left_held = input.pan_left_held;
        let pan_right_held = input.pan_right_held;
        let show_hitbox_visualization = input.show_hitbox_visualization;
        let mode = input.mode;
        let cursor = input.cursor;

        state.set_editor_shift_held(shift_held);
        state.set_editor_ctrl_held(ctrl_held);
        state.set_editor_alt_held(alt_held);
        state.set_editor_pan_up_held(pan_up_held);
        state.set_editor_pan_down_held(pan_down_held);
        state.set_editor_pan_left_held(pan_left_held);
        state.set_editor_pan_right_held(pan_right_held);
        // Phase 3.12: the `ToggleHitboxVisualization` handler in
        // `events::misc` mutates `EditorInputRes.0.show_hitbox_visualization`
        // directly. The legacy setter
        // (`set_editor_hitbox_visualization`) marks
        // `EditorDirtyFlags::hitbox_visualization_changed()`, so the
        // change guard here keeps the dirty-bit churn to one
        // notification per real user-driven change. The legacy
        // toggle also gates on `phase == AppPhase::Editor`; the
        // App-world handler does not gate (it follows the carve-out
        // pattern of writing the resource regardless of phase and
        // letting the sync no-op if the value matches).
        if state.editor_hitbox_visualization() != show_hitbox_visualization {
            state.set_editor_hitbox_visualization(show_hitbox_visualization);
        }
        // Phase 3.13: the `SetEditorMode` handler in
        // `events::editor_mode` mutates `EditorInputRes.0.mode`
        // directly. The legacy `set_editor_mode` has heavy side
        // effects (rebuilds gizmo/hover/selection outline
        // vertices, marks `EditorDirtyFlags::tap_timeline_changed()`,
        // and on Tapping→other or other→Tapping transitions, the
        // `dispatch_editor` path additionally clears the tap
        // selection / refreshes the tap preview). The change guard
        // here invokes the legacy setter only on real user-driven
        // changes; for the simple no-op case (e.g. message
        // resources seeded identically at construction time) the
        // sync is a true no-op. The Tapping-auxiliary side effects
        // (tap selection / preview refresh) live in
        // `dispatch_editor`, not in `set_editor_mode`, so a future
        // refactor that ports those to a separate sync helper is
        // still straightforward.
        if state.editor_mode() != mode {
            state.set_editor_mode(mode);
        }
        // Phase 3.27: the `NextLevel` / `PrevLevel` navigation
        // handlers mutate `EditorInputRes.0.cursor` directly while
        // legacy `State` still owns the cursor mesh rebuild side
        // effect. Keep the bridge one-way for actual editor frames:
        // the resource is seeded from legacy state at construction,
        // and legacy command dispatch refreshes it after any remaining
        // command path mutates editor input.
        if world.resource::<AppPhaseRes>().current == eg_types::AppPhase::Editor
            && state.editor_cursor_position() != cursor
        {
            state.set_editor_cursor_position(cursor);
        }
    }

    /// Sync the legacy `State.editor.config` from the App-world
    /// `EditorConfigRes`. The `snap_*` fields (carved out in
    /// `events::snap`), `selected_block_id` (carved out in
    /// `events::editor_mode::handle_set_block_id`), and
    /// `selected_block_rotation_degrees` (carved out in
    /// `events::spawn::handle_rotate_placement_preview`) are written
    /// to the resource directly by their Bevy handlers, so the
    /// resource is the source of truth at this point in the frame.
    ///
    /// Unlike the modifier-held sync above, the legacy setters for
    /// these fields have side effects (persist `app_settings` to disk,
    /// re-snap the selected block, rebuild cursor vertices, remember
    /// the block in the recent-blocks list). To avoid repeating those
    /// side effects every frame, the sync only invokes the setter when
    /// the value actually differs from the legacy state — once the
    /// legacy field is brought back in sync, no further setter calls
    /// fire until the user changes the setting again. The resource is
    /// seeded from the legacy state in `Runtime::new`, so the first
    /// frame after construction is a no-op for unchanged fields.
    fn sync_app_editor_config_state(state: &mut State, world: &World) {
        let config = &world.resource::<EditorConfigRes>().0;
        if state.editor_selected_block_id() != &config.selected_block_id {
            state.set_editor_block_id(config.selected_block_id.clone());
        }
        if state.editor_snap_to_grid() != config.snap_to_grid {
            state.set_editor_snap_to_grid(config.snap_to_grid);
        }
        if state.editor_snap_step() != config.snap_step {
            state.set_editor_snap_step(config.snap_step);
        }
        if state.editor_snap_rotation() != config.snap_rotation {
            state.set_editor_snap_rotation(config.snap_rotation);
        }
        if state.editor_snap_rotation_step_degrees() != config.snap_rotation_step_degrees {
            state.set_editor_snap_rotation_step(config.snap_rotation_step_degrees);
        }
        if state.editor_selected_block_rotation_degrees() != config.selected_block_rotation_degrees
        {
            state
                .set_editor_selected_block_rotation_degrees(config.selected_block_rotation_degrees);
        }
    }

    /// Sync the legacy `State.editor.timeline` clock + tap state from
    /// the App-world `EditorTimelineRes`. The carved-out fields are
    /// `clock.time_seconds` (`ShiftTimeline`, `SetTimelineTime`),
    /// `clock.duration_seconds` (`SetTimelineDuration`), and
    /// `taps.selected_index` (`SetSelectedTap`) plus the tap arrays for
    /// direct tap-array handlers such as `ClearTaps` and
    /// `SetSelectedTapTime` from `events::timeline`.
    ///
    /// The sync is change-guarded so a no-op frame does not re-invoke
    /// the legacy setters. Tap-array sync copies legacy's final
    /// derived tap indicators and cursor back into App resources
    /// because editor objects/spawn-derived preview side effects still
    /// live on `State` during this compatibility slice. The resource
    /// is seeded at `Runtime::new` from the legacy state, so the first
    /// frame after construction is a no-op for unchanged fields.
    fn sync_app_editor_timeline_state(state: &mut State, world: &mut World) {
        let (
            duration_seconds,
            time_seconds,
            tap_times,
            tap_indicator_positions,
            selected_index,
            tap_cursor_override,
            playback_playing,
            playback_last_mode,
        ) = {
            let timeline = &world.resource::<EditorTimelineRes>().0;
            let runtime = world.resource::<EditorRuntimeRes>();
            (
                timeline.clock.duration_seconds,
                timeline.clock.time_seconds,
                timeline.taps.tap_times.clone(),
                timeline.taps.tap_indicator_positions.clone(),
                timeline.taps.selected_index,
                timeline.tap_cursor_override,
                timeline.playback.playing,
                runtime.0.interaction.last_mode,
            )
        };
        if (state.editor_timeline_duration_seconds() - duration_seconds).abs() > 1e-6 {
            state.set_editor_timeline_duration_seconds(duration_seconds);
        }
        if (state.editor_timeline_time_seconds() - time_seconds).abs() > 1e-6 {
            state.set_editor_timeline_time_seconds(time_seconds);
        }
        let mut tap_state_changed = false;
        let mut selection_changed = false;
        if state.editor_tap_times() != tap_times.as_slice()
            || state.editor_tap_indicator_positions() != tap_indicator_positions.as_slice()
        {
            state.set_editor_tap_state_with_cursor_override(
                tap_times,
                tap_indicator_positions,
                selected_index,
                tap_cursor_override,
            );
            tap_state_changed = true;
        }
        if state.editor_selected_tap_index_value() != selected_index {
            state.set_editor_selected_tap_index(selected_index);
            selection_changed = true;
        }
        let playback_changed = state.editor_is_playing() != playback_playing;
        if playback_changed {
            state.set_editor_timeline_playback_active(playback_playing, playback_last_mode);
        }
        if tap_state_changed {
            let mut timeline = world.resource_mut::<EditorTimelineRes>();
            timeline.0.taps.tap_times = state.editor_tap_times().to_vec();
            timeline.0.taps.tap_indicator_positions =
                state.editor_tap_indicator_positions().to_vec();
            timeline.0.taps.selected_index = state.editor_selected_tap_index_value();
            timeline.0.tap_cursor_override = None;
        }
        if state.editor_is_playing() == playback_playing {
            let mut runtime = world.resource_mut::<EditorRuntimeRes>();
            runtime.0.interaction.last_mode = state.editor_playback_last_mode();
        }
        if playback_changed {
            world.resource_mut::<EditorInputRes>().0.mode = state.editor_mode();
        }
        if tap_state_changed || selection_changed {
            world.resource_mut::<EditorInputRes>().0.cursor = state.editor_cursor_position();
        }
    }

    /// Sync the legacy `State.editor.timing.{bpm_tap_*,timing_points,
    /// timing_selected_index,playback_speed,waveform_*}` from the
    /// App-world `EditorTimingRes`.
    /// The carved-out fields are:
    /// - `bpm_tap_times` / `bpm_tap_result` (`events::bpm`)
    /// - `timing_points` / `timing_selected_index` (`events::timing`)
    /// - `playback_speed`, `waveform_zoom`, `waveform_scroll`
    ///   (`events::timeline`)
    ///   The handlers compute the new state in the App world and write
    ///   the resource directly.
    ///
    /// The sync is change-guarded so a no-op frame does not re-invoke
    /// the (relatively cheap, but still mutating) legacy setters. The
    /// resource is seeded at `Runtime::new` from the legacy state, so
    /// the first frame after construction is a no-op for unchanged
    /// fields.
    fn sync_app_editor_timing_state(state: &mut State, world: &World) {
        let timing = &world.resource::<EditorTimingRes>().0;
        if state.editor_bpm_tap_times() != timing.bpm_tap_times.as_slice()
            || state.editor_bpm_tap_result() != timing.bpm_tap_result
        {
            state.set_editor_bpm_tap_state(timing.bpm_tap_times.clone(), timing.bpm_tap_result);
        }
        if state.editor_timing_points() != timing.timing_points.as_slice()
            || state.editor_timing_selected_index() != timing.timing_selected_index
        {
            state.set_editor_timing_state(
                timing.timing_points.clone(),
                timing.timing_selected_index,
            );
        }
        if (state.editor_playback_speed() - timing.playback_speed).abs() > 1e-6 {
            state.set_editor_playback_speed(timing.playback_speed);
        }
        if (state.editor_waveform_zoom() - timing.waveform_zoom).abs() > 1e-6 {
            state.set_editor_waveform_zoom(timing.waveform_zoom);
        }
        if (state.editor_waveform_scroll() - timing.waveform_scroll).abs() > 1e-6 {
            state.set_editor_waveform_scroll(timing.waveform_scroll);
        }
    }

    /// Sync the legacy `State.editor.spawn` from the App-world
    /// `EditorSpawnRes`. The carved-out fields are
    /// `position` + `direction` (`events::spawn`): the
    /// `SetSpawnHere` and `RotateSpawnDirection` handlers write
    /// the resource directly. The `RotatePlacementPreview`
    /// handler writes to `EditorConfigRes` (handled by
    /// `sync_app_editor_config_state`).
    ///
    /// The sync is change-guarded so a no-op frame does not
    /// re-invoke the legacy setter (which records history and
    /// rebuilds spawn-marker vertices — both heavy). The
    /// resource is seeded at `Runtime::new` from the legacy
    /// state, so the first frame after construction is a no-op
    /// for unchanged fields.
    fn sync_app_editor_spawn_state(state: &mut State, world: &World) {
        use eg_state::state::EditorSpawnRes;
        let spawn = &world.resource::<EditorSpawnRes>().0;
        if state.editor_spawn_position() != spawn.position
            || state.editor_spawn_direction() != spawn.direction
        {
            state.set_editor_spawn_state(spawn.clone());
        }
    }

    /// Sync the legacy `State.editor.camera` from the App-world
    /// `EditorCameraRes`. The carved-out fields are
    /// `editor_pan` + `editor_target_z` + `editor_yaw` +
    /// `editor_pitch` + `transition` (`events::camera`): the
    /// `AdjustZoom` and `SetCameraOrientation` handlers write
    /// the resource directly. The `AddCameraTrigger` and
    /// `AddCameraFollowTrigger` messages keep the legacy
    /// `PendingCommands` path because they push to
    /// `editor.objects` (a `Vec<LevelObject>`), which is out
    /// of scope for the Phase 3.11 carve-out.
    ///
    /// The sync is change-guarded so a no-op frame does not
    /// re-invoke the legacy setter (which records history and
    /// marks `EditorDirtyFlags::camera_changed()` — both
    /// undesirable per-frame). The resource is seeded at
    /// `Runtime::new` from the legacy state, so the first
    /// frame after construction is a no-op for unchanged
    /// fields.
    fn sync_app_editor_camera_state(state: &mut State, world: &World) {
        use eg_state::state::EditorCameraRes;
        let camera = &world.resource::<EditorCameraRes>().0;
        if state.editor_camera_pan() != camera.editor_pan
            || state.editor_camera_target_z() != camera.editor_target_z
            || state.editor_camera_yaw() != camera.editor_yaw
            || state.editor_camera_pitch() != camera.editor_pitch
        {
            state.set_editor_camera_state(camera.clone());
        }
    }

    /// Sync the legacy `State.editor.triggers.selected_index`
    /// from the App-world `EditorTriggerRes`. The carved-out
    /// field is `selected_index`
    /// (`events::camera::handle_set_trigger_selected`).
    ///
    /// The sync is change-guarded so a no-op frame does not
    /// re-invoke the legacy setter (which has side effects:
    /// block-selection replacement, hover index, dirty flags).
    /// The resource is seeded at `Runtime::new` from the
    /// legacy state, so the first frame after construction is
    /// a no-op for unchanged fields.
    fn sync_app_editor_trigger_state(state: &mut State, world: &World) {
        use eg_state::state::EditorTriggerRes;
        let trigger = &world.resource::<EditorTriggerRes>().0;
        if state.editor_selected_trigger_index_value() != trigger.selected_index {
            state.set_editor_trigger_selected_index(trigger.selected_index);
        }
    }

    fn sync_app_menu_state(state: &mut State, world: &World) {
        let menu = &world.resource::<MenuStateRes>().0;
        if state.menu_selected_level_index() != menu.selected_level {
            state.set_menu_selected_level_index(menu.selected_level);
        }
    }

    /// Sync the legacy `State.session` simple UI-window fields from
    /// the App-world `SessionStateRes`. The carved-out fields
    /// (`editor_level_name`, `editor_music_metadata`,
    /// `editor_menu_preview_camera`,
    /// `editor_show_metadata`, `editor_show_settings`,
    /// `editor_settings_section`, `editor_keybind_capture_action`,
    /// `editor_show_place_window`, `game_paused`,
    /// `app_settings.graphics_backend`,
    /// `app_settings.keybinds`,
    /// `app_settings.ui_scale_multiplier`,
    /// `editor_creator_metadata`, `editor_sky_color`) are written to
    /// the resource directly by their Bevy handlers in
    /// `events::session` (and by `events::misc` /
    /// `events::navigation`). The sync is
    /// change-guarded so a no-op frame does not re-invoke the legacy
    /// setters. The resource is seeded at `Runtime::new` from the
    /// legacy state, so the first frame after construction is a no-op
    /// for unchanged fields.
    fn sync_app_session_state(state: &mut State, world: &World) {
        let session = &world.resource::<SessionStateRes>();
        // Phase 3.22: `RenameLevel` mutates
        // `SessionStateRes.editor_level_name` directly. The legacy
        // setter only stores `Some(name)`, matching the message
        // payload shape; loading/import paths still own clearing or
        // replacing the whole editor session.
        if state.editor_level_name().as_ref() != session.editor_level_name.as_ref()
            && let Some(level_name) = &session.editor_level_name
        {
            state.set_editor_level_name(level_name.clone());
        }
        // Phase 3.22: `UpdateMusic` mutates
        // `SessionStateRes.editor_music_metadata` directly. The
        // legacy setter is a pure session-field write; audio import
        // and waveform side effects stay on their existing legacy
        // commands for now.
        if state.editor_music_metadata() != &session.editor_music_metadata {
            state.set_editor_music_metadata(session.editor_music_metadata.clone());
        }
        // Phase 3.25: menu-preview camera messages mutate
        // `SessionStateRes.editor_menu_preview_camera` directly.
        // The capture handler computes the camera pose from
        // `EditorCameraRes`; the legacy side is now a pure metadata
        // copy for UI/export compatibility.
        if state.editor_menu_preview_camera() != session.editor_menu_preview_camera.as_ref() {
            state.set_editor_menu_preview_camera(session.editor_menu_preview_camera.clone());
        }
        if state.editor_show_metadata() != session.editor_show_metadata {
            state.set_editor_show_metadata(session.editor_show_metadata);
        }
        if state.editor_show_settings() != session.editor_show_settings {
            state.set_editor_show_settings(session.editor_show_settings);
        }
        if state.editor_settings_section() != session.editor_settings_section {
            state.set_editor_settings_section(session.editor_settings_section);
        }
        if state.editor_keybind_capture_action() != session.editor_keybind_capture_action.as_ref() {
            state.set_editor_keybind_capture_action(session.editor_keybind_capture_action.clone());
        }
        // Phase 3.12: the `TogglePlaceWindow` handler in
        // `events::misc` mutates
        // `SessionStateRes.editor_show_place_window` directly. The
        // legacy setter has no side effects, so this is the simplest
        // case in the carve-out family — pure field write with a
        // change guard.
        if state.editor_show_place_window() != session.editor_show_place_window {
            state.set_editor_show_place_window(session.editor_show_place_window);
        }
        // Phase 3.23: `SetGraphicsBackend` mutates
        // `SessionStateRes.app_settings.graphics_backend` directly
        // when backend selection is not locked. Use the legacy setter
        // as the compatibility point so restart-required bookkeeping
        // and settings persistence remain unchanged while the setting
        // is still read from legacy `State.session` by UI snapshots.
        if state.app_settings().graphics_backend != session.app_settings.graphics_backend {
            state.set_preferred_graphics_backend(session.app_settings.graphics_backend.clone());
        }
        // Phase 3.24: keybind messages mutate
        // `SessionStateRes.app_settings.keybinds` directly using the
        // shared `AppSettings` helpers. The runtime no longer sees the
        // original semantic operation after the schedule, so sync the
        // final vector through a compatibility setter that preserves
        // settings persistence while legacy UI/settings reads remain.
        if state.app_settings().keybinds != session.app_settings.keybinds {
            state.set_app_settings_keybinds(session.app_settings.keybinds.clone());
        }
        // Phase 3.18: `GameResume` mutates
        // `SessionStateRes.game_paused` directly. Only the resume
        // direction is carved out for now: pausing still routes through
        // legacy Escape/toggle paths, and
        // `State::prepare_app_world_for_state_schedule` mirrors that
        // legacy pause flag before the schedule runs.
        if state.game_paused_value() && !session.game_paused {
            state.resume_game_from_pause();
        }
        // Phase 3.19: `SetUiScaleMultiplier` mutates
        // `SessionStateRes.app_settings.ui_scale_multiplier`
        // directly. Keep the legacy setter as the compatibility point
        // so clamping + persistence continue to use the established
        // path, and only sync this one AppSettings field until the
        // broader settings/keybind ownership pass moves the rest.
        if (state.app_settings().ui_scale_multiplier - session.app_settings.ui_scale_multiplier)
            .abs()
            > 1e-6
        {
            state.set_ui_scale_multiplier(session.app_settings.ui_scale_multiplier);
        }
        // Phase 3.21: `UpdateCreatorMetadata` mutates
        // `SessionStateRes.editor_creator_metadata` directly. The
        // legacy setter is a pure session-field write, so the sync is
        // a simple change-guarded compatibility copy until export/UI
        // metadata reads move fully into the App world.
        if state.editor_creator_metadata() != &session.editor_creator_metadata {
            state.set_editor_creator_metadata(session.editor_creator_metadata.clone());
        }
        // Phase 3.20: `UpdateSkyColor` mutates
        // `SessionStateRes.editor_sky_color` directly. Use the
        // legacy setter as the compatibility point so clamping stays
        // identical while legacy render/UI paths still read
        // `State.session.editor_sky_color`.
        if state.editor_sky_color() != session.editor_sky_color {
            state.set_editor_sky_color(session.editor_sky_color);
        }
        // Phase 3.15: the `GameSetPracticeMode` handler in
        // `events::navigation` mutates
        // `SessionStateRes.practice_mode_enabled` directly. The
        // legacy setter
        // (`set_practice_mode_enabled` → `set_practice_mode_enabled_internal`)
        // has heavy side effects (clears `practice_checkpoints`,
        // calls `rebuild_practice_checkpoint_vertices`,
        // `restart_level`, and `resume_game`); the change guard here
        // invokes the legacy setter only on real user-driven changes.
        // The resource is seeded at `Runtime::new` from the legacy
        // state, so the first frame after construction is a no-op
        // for unchanged fields.
        if state.practice_mode_enabled_value() != session.practice_mode_enabled {
            state.set_practice_mode_enabled(session.practice_mode_enabled);
        }
    }

    /// Sync the legacy `State.editor.perf.overlay_enabled` from the
    /// App-world `PerfRes`. The carved-out field is
    /// `overlay_enabled` (`events::misc::handle_toggle_perf_overlay`):
    /// the handler toggles the resource directly.
    ///
    /// The sync is change-guarded so a no-op frame does not
    /// re-invoke the legacy setter (which calls
    /// `puffin::set_scopes_on(...)` — a global side effect that
    /// does not need to fire every frame). The resource is seeded
    /// at `Runtime::new` from the legacy state via
    /// `State::perf_resource()`, so the first frame after
    /// construction is a no-op for unchanged fields.
    fn sync_app_perf_state(state: &mut State, world: &World) {
        use eg_state::state::PerfRes;
        let overlay_enabled = world.resource::<PerfRes>().0.overlay_enabled;
        if state.perf_overlay_enabled() != overlay_enabled {
            state.set_perf_overlay_enabled(overlay_enabled);
        }
    }

    /// Consume App-world profiler frame copy requests.
    ///
    /// The copy intent is now App-world authoritative, but the latest
    /// `puffin` frame view is still produced by legacy `State.editor.perf`.
    /// Keep the formatter/clipboard side effect here until perf-frame
    /// ownership moves into scheduled systems.
    fn sync_app_perf_frame_copy_request(state: &State, world: &mut World) {
        if !world.resource_mut::<PerfFrameCopyRequest>().take() {
            return;
        }

        state.copy_latest_perf_frame_to_clipboard();
    }

    /// Apply the App-world render resize request through the legacy
    /// `State::resize_surface` compatibility method until presenting
    /// moves fully into `RenderSet::Present`.
    fn sync_app_render_resize_request(state: &mut State, world: &mut World) {
        let Some(size) = world.resource_mut::<RenderResizeRequest>().take() else {
            return;
        };
        state.resize_surface(size);
    }

    /// Consume the App-world `EscapeRequest` and apply the multi-step
    /// context-sensitive escape state machine through the legacy
    /// `State::apply_editor_escape` compatibility method.
    ///
    /// Phase 3.46: `events::misc::handle_escape` writes
    /// `EscapeRequest.pending = true` instead of staging
    /// `EditorCommand::Escape` in `PendingCommands`, so the App
    /// world owns the "user pressed Escape" intent while the legacy
    /// state still owns the state machine. The sync runs after
    /// `sync_app_perf_frame_copy_request` / `sync_app_render_resize_request`
    /// and before `dispatch_app_pending_commands` so the request is
    /// consumed in the same frame and any state changes (phase,
    /// timeline, session) propagate to the App world via the
    /// downstream sync helpers.
    fn sync_app_escape_request(state: &mut State, world: &mut World) {
        if !world.resource_mut::<EscapeRequest>().take() {
            return;
        }
        state.apply_editor_escape();
    }

    /// Consume the App-world `AuthSubmitSignInRequest` and apply the
    /// legacy `State::submit_auth_sign_in` compatibility method.
    ///
    /// Phase 3.47: `events::auth::handle_auth_submit_sign_in` writes
    /// `AuthSubmitSignInRequest.pending = true` instead of staging
    /// `AppCommand::AuthSubmitSignIn` in `PendingCommands`. The
    /// sign-in side effect (network IO via `trigger_auth_sign_in`
    /// on the auth channel) stays legacy-owned because the auth
    /// channel `Sender` lives on the legacy `State`. The sync runs
    /// before `dispatch_app_pending_commands` so the request is
    /// consumed in the same frame and any auth state changes
    /// (pending flag, message) propagate to legacy readers via the
    /// existing `State::update_auth_results` polling path.
    fn sync_app_auth_submit_sign_in_request(state: &mut State, world: &mut World) {
        if !world.resource_mut::<AuthSubmitSignInRequest>().take() {
            return;
        }
        state.submit_auth_sign_in();
    }

    /// Consume the App-world `AuthSignOutRequest` and apply the
    /// legacy `State::sign_out_auth_session` compatibility method.
    ///
    /// Phase 3.47: same request pattern as
    /// `sync_app_auth_submit_sign_in_request`. The sign-out side
    /// effect (network IO via `trigger_auth_sign_out` on the auth
    /// channel) stays legacy-owned because the auth channel
    /// `Sender` lives on the legacy `State`.
    fn sync_app_auth_sign_out_request(state: &mut State, world: &mut World) {
        if !world.resource_mut::<AuthSignOutRequest>().take() {
            return;
        }
        state.sign_out_auth_session();
    }

    /// Consume the App-world `AuthOpenSignupRequest` and apply the
    /// legacy `State::open_auth_signup_page` compatibility method.
    ///
    /// Phase 3.47: same request pattern as
    /// `sync_app_auth_submit_sign_in_request`. The signup-page-open
    /// side effect (platform code via
    /// `eg_platform::auth::open_signup_page`) stays legacy-owned
    /// because the platform API is only available in non-wasm
    /// targets.
    #[cfg(not(target_arch = "wasm32"))]
    fn sync_app_auth_open_signup_request(state: &mut State, world: &mut World) {
        if !world.resource_mut::<AuthOpenSignupRequest>().take() {
            return;
        }
        state.open_auth_signup_page();
    }

    /// WASM stub: `open_auth_signup_page` is not callable in wasm
    /// targets (it would invoke `window.open` in a non-async
    /// browser context). Phase 3.47 still keeps the request
    /// resource so the App world owns the "user requested signup
    /// page" intent in the unified schedule; runtime drops it on
    /// wasm where the platform API is unavailable.
    #[cfg(target_arch = "wasm32")]
    fn sync_app_auth_open_signup_request(_state: &mut State, world: &mut World) {
        let _ = world.resource_mut::<AuthOpenSignupRequest>().take();
    }

    fn dispatch_app_pending_commands(state: &mut State, world: &mut World) {
        let mut pending = std::mem::take(&mut *world.resource_mut::<PendingCommands>());
        let had_commands = !pending.is_empty();
        for command in pending.app_commands.drain(..) {
            state.dispatch(command);
        }
        for command in pending.editor_commands.drain(..) {
            state.dispatch_editor(command);
        }
        if had_commands {
            world.insert_resource(state.app_phase_resource());
            world.insert_resource(state.editor_input_resource());
            world.insert_resource(state.menu_state_resource());
        }
    }

    /// Queue platform input in the App world, then apply it through the
    /// legacy State input path during the compatibility window.
    pub fn process_input_event(&mut self, event: eg_state::commands::InputEvent) {
        enqueue_platform_input(self.app.world_mut(), event.clone());
        self.state.process_input_event(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eg_state::state::State;
    use eg_state::state::{
        AppPhaseRes, AuthOpenSignupRequest, AuthSignOutRequest, AuthSubmitSignInRequest,
        EditorCameraRes, EditorConfigRes, EditorHistoryRes, EditorInputRes, EditorObjectsRes,
        EditorRuntimeRes, EditorTimelineRes, EditorTimingRes, EditorTriggerRes, EscapeRequest,
        MenuStateRes, PerfFrameCopyRequest, PerfRes, PlatformInputQueue, RenderResizeRequest,
        SessionStateRes,
    };
    use eg_ui::EguiContextRes;

    #[tokio::test]
    async fn test_runtime_new() {
        let state = State::new_test().await;
        let runtime = Runtime::new(state);

        // Verify the egui context was initialized
        let ctx = runtime.pipeline.ctx();
        assert!(ctx.pixels_per_point() > 0.0);

        // Verify the state was correctly moved into the runtime
        assert!(runtime.state.is_menu());
    }

    // ---- Phase 7 tests ----
    //
    // These pin the Phase 7-scaffold contract: `Runtime::new`
    // builds a `bevy_app::App` with the eg-state plugin graph,
    // and `run_app_frame` drives `app.update()` without
    // panicking. Future sub-phases (7-1: schedule migration,
    // 7-2: frame driver, 7-3: pipeline split) extend these
    // tests with system-level assertions.

    /// `Runtime::new` builds an `App` whose `World` contains every
    /// resource that `StatePlugins` is responsible for registering.
    /// `AppPhaseRes` defaults to `AppPhase::Menu`; gameplay, editor,
    /// session, and render resources are installed by their plugins.
    #[tokio::test]
    async fn runtime_app_has_state_plugin_resources() {
        let state = State::new_test().await;
        let runtime = Runtime::new(state);

        let world = runtime.app.world();
        assert!(
            world.contains_resource::<eg_game::GameClock>(),
            "GameClock registered by GameplayPlugin"
        );
        assert!(
            world.contains_resource::<AppPhaseRes>(),
            "AppPhaseRes registered by MenuPlugin"
        );
        assert!(
            world.contains_resource::<SessionStateRes>(),
            "SessionStateRes registered by SessionPlugin"
        );
        assert!(
            world.contains_resource::<MenuStateRes>(),
            "MenuStateRes seeded from legacy State"
        );
        assert!(
            world.contains_resource::<EditorInputRes>(),
            "EditorInputRes registered by EditorPlugin"
        );
        assert!(
            world.contains_resource::<EditorObjectsRes>(),
            "EditorObjectsRes registered by EditorPlugin"
        );
        assert!(
            world.contains_resource::<EditorConfigRes>(),
            "EditorConfigRes registered by EditorPlugin"
        );
        assert!(
            world.contains_resource::<EditorCameraRes>(),
            "EditorCameraRes registered by EditorPlugin"
        );
        assert!(
            world.contains_resource::<EditorTriggerRes>(),
            "EditorTriggerRes registered by EditorPlugin"
        );
        assert!(
            world.contains_resource::<EditorTimelineRes>(),
            "EditorTimelineRes registered by EditorPlugin"
        );
        assert!(
            world.contains_resource::<EditorTimingRes>(),
            "EditorTimingRes registered by EditorPlugin"
        );
        assert!(
            world.contains_resource::<EditorRuntimeRes>(),
            "EditorRuntimeRes registered by EditorPlugin"
        );
        assert!(
            world.contains_resource::<EditorHistoryRes>(),
            "EditorHistoryRes registered by EditorPlugin"
        );
        assert!(
            world.contains_resource::<PerfRes>(),
            "PerfRes registered by EditorPlugin"
        );
        assert!(
            world.contains_resource::<PerfFrameCopyRequest>(),
            "PerfFrameCopyRequest registered by EditorPlugin"
        );
        assert!(
            world.contains_resource::<RenderResizeRequest>(),
            "RenderResizeRequest registered by RenderPlugin"
        );
        assert!(
            world.contains_resource::<PlatformInputQueue>(),
            "PlatformInputQueue registered by EditorPlugin"
        );
        assert!(
            world.contains_resource::<EguiContextRes>(),
            "EguiContextRes registered by Runtime UI bridge"
        );
    }

    /// `register_state_command_pipeline` adds the per-frame
    /// `StateSchedule` to the App's World via Bevy's
    /// `World::add_schedule`. Pin the schedule is reachable from
    /// the App so the future 7-1 sub-phase can move the
    /// `State::tick_state` invocation here.
    #[tokio::test]
    async fn runtime_app_registers_state_schedule() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        // The schedule is named `StateSchedule` (defined in
        // `eg_state::state::sets`). Reaching it via the
        // `try_run_schedule` API is the cheapest way to assert
        // registration without exposing a Bevy-typed getter.
        let world = runtime.app.world_mut();
        let result = world.try_run_schedule(eg_state::state::StateSchedule);
        assert!(
            result.is_ok(),
            "StateSchedule should be registered on the App's World"
        );
    }

    /// `GameplayPlugin` installs the real eg-game ECS schedule into
    /// the App's World. This is the first single-source cut: gameplay
    /// resources and systems exist on the App, not only in the legacy
    /// `GameState` wrapper path.
    #[tokio::test]
    async fn runtime_app_registers_gameplay_schedule() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        let result = runtime
            .app
            .world_mut()
            .try_run_schedule(eg_game::GameplaySchedule);
        assert!(
            result.is_ok(),
            "GameplaySchedule should be registered on the App's World"
        );
    }

    /// `Runtime::run_app_frame` is safe to call and produces normal
    /// egui output while driving the App first.
    #[tokio::test]
    async fn runtime_run_app_frame_returns_output() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        let output = runtime.run_app_frame(egui::RawInput::default());
        assert!(output.pixels_per_point > 0.0);
    }

    /// `Runtime::run_app_frame` mirrors the live egui context into the
    /// App's World before the frame update. This gives Bevy UI systems
    /// an App-world anchor while the compatibility renderer still owns
    /// actual egui painting.
    #[tokio::test]
    async fn runtime_run_app_frame_syncs_egui_context_to_app_world() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        assert!(
            runtime.app.world().resource::<EguiContextRes>().0.is_none(),
            "egui context starts empty before the first frame"
        );

        let output = runtime.run_app_frame(egui::RawInput::default());
        let app_ctx = runtime
            .app
            .world()
            .resource::<EguiContextRes>()
            .0
            .as_ref()
            .expect("egui context mirrored into App World");

        assert_eq!(app_ctx.pixels_per_point(), output.pixels_per_point);
    }

    /// Platform input now enters the App world before the legacy
    /// `State::process_input_event` compatibility path consumes it.
    #[tokio::test]
    async fn runtime_process_input_event_queues_app_world_input() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        runtime.process_input_event(eg_state::commands::InputEvent::Resize {
            width: 640,
            height: 360,
        });

        let queue = runtime.app.world().resource::<PlatformInputQueue>();
        assert_eq!(queue.len(), 1);
        assert!(matches!(
            queue.events()[0],
            eg_state::commands::InputEvent::Resize {
                width: 640,
                height: 360
            }
        ));
        assert_eq!(runtime.state.surface_width(), 640);
        assert_eq!(runtime.state.surface_height(), 360);
    }

    /// `Runtime::run_app_frame` drives the App's `Update` schedule,
    /// then explicitly runs the eg-state `StateSchedule` on the App's
    /// World before draining pending commands.
    #[tokio::test]
    async fn runtime_run_app_frame_updates_app_state_schedule() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .resource_mut::<AppPhaseRes>()
            .set(eg_types::AppPhase::Playing);

        let _ = runtime.run_app_frame(egui::RawInput::default());

        let app_phase = runtime.app.world().resource::<AppPhaseRes>();
        assert_eq!(app_phase.current, eg_types::AppPhase::Playing);
        assert_eq!(app_phase.previous_app_phase, eg_types::AppPhase::Playing);
    }

    /// Plain `app.update()` no longer drives the mirror `StateSchedule`.
    /// Runtime owns that order so command draining stays visible in one
    /// frame-driver method.
    #[tokio::test]
    async fn runtime_app_update_does_not_run_state_schedule_directly() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .resource_mut::<AppPhaseRes>()
            .set(eg_types::AppPhase::Playing);

        runtime.app.update();

        let app_phase = runtime.app.world().resource::<AppPhaseRes>();
        assert_eq!(app_phase.current, eg_types::AppPhase::Playing);
        assert_eq!(app_phase.previous_app_phase, eg_types::AppPhase::Menu);
    }

    /// `Runtime::run_app_frame` drains App-world messages and applies
    /// the recovered legacy command to `State` during the compatibility
    /// window. UI commands enter through this path before legacy dispatch.
    #[tokio::test]
    async fn runtime_run_app_frame_dispatches_app_world_commands() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .write_message(eg_state::events::navigation::ToggleEditor);

        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(runtime.state.is_editor());
    }

    #[tokio::test]
    async fn runtime_carve_out_resize_surface_applies_render_resize_request() {
        use eg_state::events::auth::ResizeSurface;

        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);
        let initial_size = runtime.state.current_render_size();
        let new_size = eg_types::PhysicalSize::new(
            initial_size.width.saturating_add(17).max(1),
            initial_size.height.saturating_add(11).max(1),
        );

        runtime.app.world_mut().write_message(ResizeSurface {
            width: new_size.width,
            height: new_size.height,
        });
        let _ = runtime.run_app_frame(egui::RawInput::default());

        let applied_size = runtime.state.current_render_size();
        assert_eq!(
            applied_size.width, new_size.width,
            "sync_app_render_resize_request must apply App-world resize width"
        );
        assert_eq!(
            applied_size.height, new_size.height,
            "sync_app_render_resize_request must apply App-world resize height"
        );
        let resize = runtime.app.world().resource::<RenderResizeRequest>();
        assert!(
            !resize.pending,
            "runtime must consume RenderResizeRequest after applying it"
        );
        let pending = runtime.app.world().resource::<PendingCommands>();
        assert!(
            pending.app_commands.is_empty() && pending.editor_commands.is_empty(),
            "ResizeSurface must not stage a legacy command"
        );
    }

    #[tokio::test]
    async fn runtime_carve_out_copy_latest_perf_frame_consumes_copy_request() {
        use eg_state::events::auth::CopyLatestPerfFrame;

        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        runtime.app.world_mut().write_message(CopyLatestPerfFrame);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        let request = runtime.app.world().resource::<PerfFrameCopyRequest>();
        assert!(
            !request.pending,
            "runtime must consume PerfFrameCopyRequest after handling it"
        );
        let pending = runtime.app.world().resource::<PendingCommands>();
        assert!(
            pending.app_commands.is_empty() && pending.editor_commands.is_empty(),
            "CopyLatestPerfFrame must not stage a legacy command"
        );
    }

    /// `AuthSubmitSignIn` is a Phase 3.47 carve-out: the App-world
    /// handler writes `AuthSubmitSignInRequest.pending = true`, the
    /// runtime sync consumes the request and calls the legacy
    /// `State::submit_auth_sign_in` compatibility method, and no
    /// legacy `AppCommand::AuthSubmitSignIn` is staged in
    /// `PendingCommands`. The legacy `auth.pending` flag is also
    /// set by the legacy side effect.
    #[tokio::test]
    async fn runtime_carve_out_auth_submit_sign_in_consumes_request() {
        use eg_state::events::auth::AuthSubmitSignIn;

        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        runtime.app.world_mut().write_message(AuthSubmitSignIn);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        let request = runtime.app.world().resource::<AuthSubmitSignInRequest>();
        assert!(
            !request.pending,
            "runtime must consume AuthSubmitSignInRequest after applying it"
        );
        let pending = runtime.app.world().resource::<PendingCommands>();
        assert!(
            pending.app_commands.is_empty() && pending.editor_commands.is_empty(),
            "AuthSubmitSignIn must not stage a legacy command"
        );
    }

    /// `AuthSignOut` is a Phase 3.47 carve-out: same request pattern
    /// as `AuthSubmitSignIn`.
    #[tokio::test]
    async fn runtime_carve_out_auth_sign_out_consumes_request() {
        use eg_state::events::auth::AuthSignOut;

        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        runtime.app.world_mut().write_message(AuthSignOut);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        let request = runtime.app.world().resource::<AuthSignOutRequest>();
        assert!(
            !request.pending,
            "runtime must consume AuthSignOutRequest after applying it"
        );
        let pending = runtime.app.world().resource::<PendingCommands>();
        assert!(
            pending.app_commands.is_empty() && pending.editor_commands.is_empty(),
            "AuthSignOut must not stage a legacy command"
        );
    }

    /// `AuthOpenSignup` is a Phase 3.47 carve-out: same request
    /// pattern as `AuthSubmitSignIn`. The legacy
    /// `State::open_auth_signup_page` is platform-only and may
    /// no-op in test builds; the carve-out assertion pins the
    /// request was consumed and no legacy command was staged.
    #[tokio::test]
    async fn runtime_carve_out_auth_open_signup_consumes_request() {
        use eg_state::events::auth::AuthOpenSignup;

        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        runtime.app.world_mut().write_message(AuthOpenSignup);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        let request = runtime.app.world().resource::<AuthOpenSignupRequest>();
        assert!(
            !request.pending,
            "runtime must consume AuthOpenSignupRequest after applying it"
        );
        let pending = runtime.app.world().resource::<PendingCommands>();
        assert!(
            pending.app_commands.is_empty() && pending.editor_commands.is_empty(),
            "AuthOpenSignup must not stage a legacy command"
        );
    }

    /// `Runtime` keeps App-world setup from mutating legacy State before
    /// the frame driver explicitly applies queued commands.
    #[tokio::test]
    async fn runtime_app_world_setup_does_not_mutate_state() {
        let state = State::new_test().await;
        let runtime = Runtime::new(state);

        // The App's World has the plugin resources.
        let app_phase = runtime.app.world().resource::<AppPhaseRes>();
        assert_eq!(app_phase.current, eg_types::AppPhase::Menu);

        // Pin that the State is still in the Menu phase after
        // the App was built (no setup cross-talk).
        assert!(runtime.state.is_menu());
    }

    // --- Phase 7 carve-out sync tests (snap domain) ---
    //
    // The `snap` event handlers in `eg_state::events::snap` write to
    // `EditorConfigRes` directly. `Runtime::run_app_frame` then calls
    // `sync_app_editor_config_state` to copy the resource to the legacy
    // `State.editor.config` fields before any dispatch. The tests below
    // pin the end-to-end path: message → App-world resource → legacy
    // State field. They are intentionally written against the public
    // Runtime API (not the internal `sync_app_editor_config_state`
    // helper) so they survive internal refactors.

    use eg_state::events::snap::{
        SetSnapRotation, SetSnapRotationStep, SetSnapStep, SetSnapToGrid,
    };

    /// Writing a `SetSnapToGrid` message reaches the App world, mutates
    /// `EditorConfigRes.0.snap_to_grid`, and the per-frame sync propagates
    /// the value to the legacy `State.editor.config.snap_to_grid` so the
    /// existing snap-aware rendering / persistence code paths still work.
    #[tokio::test]
    async fn runtime_carve_out_set_snap_to_grid_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        // Seed an opposite value on both sides to prove the message
        // actually flows through.
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorConfigRes>()
            .0
            .snap_to_grid = true;
        runtime.state.set_editor_snap_to_grid(true);

        // Flip both the message resource and the runtime's pending message
        // so the next frame's schedule runs the carve-out handler.
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorConfigRes>()
            .0
            .snap_to_grid = true;
        runtime.app.world_mut().write_message(SetSnapToGrid(false));

        let _ = runtime.run_app_frame(egui::RawInput::default());

        // App world is the source of truth.
        assert!(
            !runtime
                .app
                .world()
                .resource::<EditorConfigRes>()
                .0
                .snap_to_grid,
            "SetSnapToGrid(false) handler must mutate EditorConfigRes"
        );
        // Legacy state was synced to match the resource.
        assert!(
            !runtime.state.editor_snap_to_grid(),
            "sync_app_editor_config_state must propagate EditorConfigRes \
             into the legacy State.editor.config.snap_to_grid field"
        );
    }

    /// `SetSnapStep` follows the same carve-out: the resource value
    /// is the source of truth, and the legacy state field reflects it
    /// after the per-frame sync.
    #[tokio::test]
    async fn runtime_carve_out_set_snap_step_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        let new_step = 0.5_f32;
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorConfigRes>()
            .0
            .snap_step = 1.0;
        runtime.state.set_editor_snap_step(1.0);

        runtime.app.world_mut().write_message(SetSnapStep(new_step));

        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(
            (runtime
                .app
                .world()
                .resource::<EditorConfigRes>()
                .0
                .snap_step
                - new_step)
                .abs()
                < 1e-6,
            "SetSnapStep handler must mutate EditorConfigRes.snap_step"
        );
        assert!(
            (runtime.state.editor_snap_step() - new_step).abs() < 1e-6,
            "sync_app_editor_config_state must propagate snap_step to legacy state"
        );
    }

    /// Rotation-snap fields follow the carve-out too.
    #[tokio::test]
    async fn runtime_carve_out_set_snap_rotation_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        runtime.state.set_editor_snap_rotation(true);
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorConfigRes>()
            .0
            .snap_rotation = true;

        runtime
            .app
            .world_mut()
            .write_message(SetSnapRotation(false));

        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(!runtime.state.editor_snap_rotation());
        assert!(
            !runtime
                .app
                .world()
                .resource::<EditorConfigRes>()
                .0
                .snap_rotation
        );
    }

    /// `SetSnapRotationStep` propagates as a f32.
    #[tokio::test]
    async fn runtime_carve_out_set_snap_rotation_step_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        runtime.state.set_editor_snap_rotation_step(15.0);
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorConfigRes>()
            .0
            .snap_rotation_step_degrees = 15.0;

        runtime
            .app
            .world_mut()
            .write_message(SetSnapRotationStep(45.0));

        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!((runtime.state.editor_snap_rotation_step_degrees() - 45.0).abs() < 1e-6);
        assert!(
            (runtime
                .app
                .world()
                .resource::<EditorConfigRes>()
                .0
                .snap_rotation_step_degrees
                - 45.0)
                .abs()
                < 1e-6
        );
    }

    /// When the message and the resource are unchanged, the sync must
    /// not re-invoke the legacy setter (which has side effects: it
    /// persists `app_settings` to storage). Pin the setter side-effect
    /// guard by checking the State field matches the resource after a
    /// no-op frame.
    #[tokio::test]
    async fn runtime_carve_out_no_op_frame_does_not_resync_snap() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        // Seed both sides to a known state, then run a frame with no
        // snap message. The sync guard must not re-invoke the legacy
        // setter, so the State field stays at the value we set.
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorConfigRes>()
            .0
            .snap_to_grid = true;
        runtime.state.set_editor_snap_to_grid(true);

        let _ = runtime.run_app_frame(egui::RawInput::default());

        // No message written -> handler does not run -> resource
        // unchanged. State value must remain `true` (the sync guard
        // sees no diff and does not touch the State field).
        assert!(runtime.state.editor_snap_to_grid());
        assert!(
            runtime
                .app
                .world()
                .resource::<EditorConfigRes>()
                .0
                .snap_to_grid
        );
    }

    /// Regression: `EditorConfigRes` must be seeded from the legacy
    /// `State` at `Runtime::new` so the change-guarded config sync
    /// does not clobber user-loaded `app_settings` on the first
    /// frame. Before the seeding fix, the resource was
    /// Default-initialized (`snap_to_grid = true`) while the legacy
    /// state held a user value (`false`); the first frame's sync saw
    /// a diff and overwrote the user value back to the default (and
    /// re-persisted it). This test pins the seeding + the no-clobber
    /// property for the snap config.
    #[tokio::test]
    async fn runtime_seeds_editor_config_res_from_legacy_state_no_first_frame_clobber() {
        let mut state = State::new_test().await;
        // User-loaded settings: snap-to-grid disabled and a non-default
        // step. The resource defaults are `snap_to_grid = true` and
        // `snap_step = 1.0`, so these differ from the defaults and
        // would be clobbered without the seeding fix.
        state.set_editor_snap_to_grid(false);
        state.set_editor_snap_step(0.25);
        let mut runtime = Runtime::new(state);

        // Seeding: the resource must reflect the legacy user value,
        // not the `EditorConfigState` default.
        assert!(
            !runtime
                .app
                .world()
                .resource::<EditorConfigRes>()
                .0
                .snap_to_grid,
            "EditorConfigRes must be seeded from legacy State at Runtime::new"
        );
        assert!(
            (runtime
                .app
                .world()
                .resource::<EditorConfigRes>()
                .0
                .snap_step
                - 0.25)
                .abs()
                < 1e-6,
            "EditorConfigRes.snap_step must be seeded from legacy State"
        );

        // Run a frame with no snap message. The change-guarded sync
        // must see no diff and must NOT invoke the legacy setter, so
        // the user value survives the first frame.
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(
            !runtime.state.editor_snap_to_grid(),
            "first-frame sync must not clobber user snap_to_grid setting"
        );
        assert!(
            (runtime.state.editor_snap_step() - 0.25).abs() < 1e-6,
            "first-frame sync must not clobber user snap_step setting"
        );
    }

    /// `SetBlockId` carve-out end-to-end: writing the message reaches
    /// the App world, mutates `EditorConfigRes.0.selected_block_id`,
    /// and the per-frame sync propagates the value to the legacy
    /// `State.editor.config.selected_block_id` (which also remembers
    /// the block in `recent_block_ids` and persists). The resource is
    /// the source of truth; the legacy field mirrors it after the sync.
    #[tokio::test]
    async fn runtime_carve_out_set_block_id_propagates_to_legacy_state() {
        use eg_blocks::BlockId;
        use eg_state::events::editor_mode::SetBlockId;

        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        let new_id = BlockId::new("core/lava");
        // Precondition: the new id differs from the seeded value.
        assert_ne!(
            runtime.state.editor_selected_block_id(),
            &new_id,
            "test precondition: new block id must differ from current"
        );

        runtime
            .app
            .world_mut()
            .write_message(SetBlockId(new_id.clone()));

        let _ = runtime.run_app_frame(egui::RawInput::default());

        // App world is the source of truth.
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorConfigRes>()
                .0
                .selected_block_id,
            new_id,
            "SetBlockId handler must mutate EditorConfigRes.selected_block_id"
        );
        // Legacy state was synced to match the resource.
        assert_eq!(
            runtime.state.editor_selected_block_id(),
            &new_id,
            "sync_app_editor_config_state must propagate selected_block_id to legacy state"
        );
        // The legacy setter also remembers the block in recent_block_ids,
        // proving the change-guarded sync invoked the full setter (not
        // just a field copy).
        assert_eq!(
            runtime.state.editor_recent_block_ids().first(),
            Some(&new_id),
            "set_editor_block_id side effect must remember the block in recent_block_ids"
        );
    }

    /// `PickSelectedBlock` carve-out end-to-end: the App-world handler
    /// reads the selected object, writes `EditorConfigRes.selected_block_id`,
    /// and runtime sync propagates the picked id to legacy State without
    /// staging `EditorCommand::PickSelectedBlock`.
    #[tokio::test]
    async fn runtime_carve_out_pick_selected_block_propagates_to_legacy_state() {
        use eg_state::events::editor_mode::PickSelectedBlock;

        let picked = eg_blocks::BlockId::new("core/lava");
        let mut state = State::new_test().await;
        state.enter_editor_phase_for_test("PickSelectedBlockRuntime");
        state.set_editor_objects_from_app(vec![eg_types::LevelObject {
            block_id: picked.clone(),
            ..eg_types::LevelObject::default()
        }]);
        state.set_editor_block_selection_from_app(Some(0), vec![0], Some(0));
        assert_ne!(state.editor_selected_block_id(), &picked);

        let mut runtime = Runtime::new(state);
        runtime.app.world_mut().write_message(PickSelectedBlock);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorConfigRes>()
                .0
                .selected_block_id,
            picked
        );
        assert_eq!(runtime.state.editor_selected_block_id(), &picked);
        assert_eq!(runtime.state.editor_mode(), eg_types::EditorMode::Place);
        let pending = runtime.app.world().resource::<PendingCommands>();
        assert!(
            pending.app_commands.is_empty() && pending.editor_commands.is_empty(),
            "PickSelectedBlock must not stage a legacy command"
        );
    }

    /// `SetBlockId` carve-out no-op frame: when no `SetBlockId` message
    /// is written and the resource matches the legacy state (the
    /// seeded invariant), the sync must not re-invoke
    /// `set_editor_block_id`. Pin the side-effect guard by confirming
    /// the recent-blocks list is not reordered when nothing changed.
    #[tokio::test]
    async fn runtime_carve_out_no_op_frame_does_not_resync_block_id() {
        use eg_blocks::BlockId;

        let mut state = State::new_test().await;
        // Seed a known block id + recent list via the legacy setter so
        // both the resource (seeded at Runtime::new) and the legacy
        // state start in sync.
        state.set_editor_block_id(BlockId::new("core/lava"));
        let recent_before = Vec::from(state.editor_recent_block_ids());
        let mut runtime = Runtime::new(state);

        let _ = runtime.run_app_frame(egui::RawInput::default());

        // No message -> handler does not run -> resource unchanged.
        // Sync sees no diff -> setter not invoked -> recent list stable.
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorConfigRes>()
                .0
                .selected_block_id,
            BlockId::new("core/lava")
        );
        assert_eq!(
            runtime.state.editor_selected_block_id(),
            &BlockId::new("core/lava")
        );
        assert_eq!(
            runtime.state.editor_recent_block_ids(),
            recent_before.as_slice(),
            "no-op frame must not reorder recent_block_ids"
        );
    }

    #[tokio::test]
    async fn runtime_carve_out_block_copy_and_remove_propagate_to_legacy_state() {
        use eg_state::events::block::{CopyBlock, RemoveBlock};

        let mut state = State::new_test().await;
        state.enter_editor_phase_for_test("BlockRuntimeCarveOut");
        state.set_editor_objects_from_app(vec![
            eg_types::LevelObject {
                id: "removed".to_string(),
                position: [0.0, 0.0, 0.0],
                ..eg_types::LevelObject::default()
            },
            eg_types::LevelObject {
                id: "kept".to_string(),
                position: [2.0, 0.0, 0.0],
                ..eg_types::LevelObject::default()
            },
        ]);
        state.set_editor_block_selection_from_app(Some(0), vec![0], Some(0));
        let mut runtime = Runtime::new(state);

        runtime.app.world_mut().write_message(CopyBlock);
        let _ = runtime.run_app_frame(egui::RawInput::default());
        assert!(
            runtime.state.editor_clipboard().is_some(),
            "copy must sync the App-world clipboard back to legacy State"
        );

        runtime.app.world_mut().write_message(RemoveBlock);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(runtime.state.editor_objects().len(), 1);
        assert_eq!(runtime.state.editor_objects()[0].id, "kept");
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorObjectsRes>()
                .objects
                .len(),
            1,
            "App object resource must converge with legacy State after presentation sync"
        );
        assert!(
            !runtime
                .app
                .world()
                .resource::<EditorObjectsRes>()
                .object_mutation_pending,
            "object mutation flag must be consumed by Runtime sync"
        );
        let pending = runtime.app.world().resource::<PendingCommands>();
        assert!(
            pending.app_commands.is_empty() && pending.editor_commands.is_empty(),
            "block carve-out messages must not stage legacy commands"
        );
    }

    #[tokio::test]
    async fn runtime_carve_out_add_transform_trigger_propagates_to_legacy_state() {
        use eg_state::events::selection::AddTransformTrigger;

        let mut state = State::new_test().await;
        state.enter_editor_phase_for_test("SelectionRuntimeCarveOut");
        state.set_editor_show_place_window(true);
        state.set_editor_timeline_time_seconds(2.5);
        state.set_editor_objects_from_app(vec![eg_types::LevelObject {
            id: "object_0".to_string(),
            position: [0.0, 0.0, 0.0],
            ..eg_types::LevelObject::default()
        }]);
        state.set_editor_block_selection_from_app(Some(0), vec![0], Some(0));
        let mut runtime = Runtime::new(state);

        runtime.app.world_mut().write_message(AddTransformTrigger);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(runtime.state.editor_objects().len(), 2);
        let trigger_object = &runtime.state.editor_objects()[1];
        assert_eq!(
            trigger_object.block_id,
            eg_types::TRANSFORM_TRIGGER_BLOCK_ID
        );
        let trigger = trigger_object
            .trigger
            .as_ref()
            .expect("new transform trigger");
        approx::assert_abs_diff_eq!(trigger.time_seconds, 2.5, epsilon = 1e-6);
        assert_eq!(
            trigger.target,
            eg_types::TriggerTarget::Objects {
                object_ids: vec!["object_0".to_string()],
            }
        );
        assert!(
            !runtime.state.editor_show_place_window(),
            "selection handler must sync the App-world place-window close to legacy State"
        );
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorObjectsRes>()
                .objects
                .len(),
            2,
            "App object resource must converge with legacy State after presentation sync"
        );
        assert!(
            !runtime
                .app
                .world()
                .resource::<EditorObjectsRes>()
                .object_mutation_pending,
            "object mutation flag must be consumed by Runtime sync"
        );
        let pending = runtime.app.world().resource::<PendingCommands>();
        assert!(
            pending.app_commands.is_empty() && pending.editor_commands.is_empty(),
            "AddTransformTrigger must not stage a legacy command"
        );
    }

    #[tokio::test]
    async fn runtime_carve_out_add_camera_trigger_propagates_to_legacy_state() {
        use eg_state::events::camera::AddCameraTrigger;

        let mut state = State::new_test().await;
        state.enter_editor_phase_for_test("CameraTriggerRuntimeCarveOut");
        state.set_editor_timeline_time_seconds(2.5);
        state.set_editor_camera_pose_for_test([12.0, -7.0], 4.5, 0.9, 0.3);
        let mut runtime = Runtime::new(state);

        runtime.app.world_mut().write_message(AddCameraTrigger);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(runtime.state.editor_objects().len(), 1);
        let trigger_object = &runtime.state.editor_objects()[0];
        assert_eq!(trigger_object.block_id, eg_types::CAMERA_TRIGGER_BLOCK_ID);
        let trigger = trigger_object.trigger.as_ref().expect("camera trigger");
        approx::assert_abs_diff_eq!(trigger.time_seconds, 2.5, epsilon = 1e-6);
        assert_eq!(trigger.target, eg_types::TriggerTarget::Camera);
        let camera = trigger.camera.as_ref().expect("camera payload");
        assert_eq!(camera.mode, eg_types::CameraMode::Static);
        assert_eq!(camera.target_position, [12.0, 4.5, -7.0]);
        approx::assert_abs_diff_eq!(camera.yaw, 0.9, epsilon = 1e-6);
        approx::assert_abs_diff_eq!(camera.pitch, 0.3, epsilon = 1e-6);
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorObjectsRes>()
                .objects
                .len(),
            1,
            "App object resource must converge with legacy State after presentation sync"
        );
        assert!(
            !runtime
                .app
                .world()
                .resource::<EditorObjectsRes>()
                .object_mutation_pending,
            "object mutation flag must be consumed by Runtime sync"
        );
        let pending = runtime.app.world().resource::<PendingCommands>();
        assert!(
            pending.app_commands.is_empty() && pending.editor_commands.is_empty(),
            "AddCameraTrigger must not stage a legacy command"
        );
    }

    #[tokio::test]
    async fn runtime_carve_out_snap_selection_to_grid_propagates_block_snap() {
        use eg_state::events::selection::SnapSelectionToGrid;

        let mut state = State::new_test().await;
        state.enter_editor_phase_for_test("SelectionSnapBlocks");
        state.set_editor_snap_step(1.0);
        state.set_editor_objects_from_app(vec![
            eg_types::LevelObject {
                id: "first".to_string(),
                position: [0.2, 0.0, 0.7],
                ..eg_types::LevelObject::default()
            },
            eg_types::LevelObject {
                id: "second".to_string(),
                position: [3.1, 0.0, 3.6],
                ..eg_types::LevelObject::default()
            },
        ]);
        state.set_editor_block_selection_from_app(Some(0), vec![0, 1], Some(0));
        let mut runtime = Runtime::new(state);

        runtime.app.world_mut().write_message(SnapSelectionToGrid);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(runtime.state.editor_objects()[0].position, [0.0, 0.0, 1.0]);
        assert_eq!(runtime.state.editor_objects()[1].position, [3.0, 0.0, 4.0]);
        assert_eq!(
            runtime.app.world().resource::<EditorInputRes>().0.cursor,
            runtime.state.editor_cursor_position(),
            "runtime sync must converge the App cursor with legacy State"
        );
        assert!(
            !runtime
                .app
                .world()
                .resource::<EditorObjectsRes>()
                .object_mutation_pending,
            "object mutation flag must be consumed by Runtime sync"
        );
        let pending = runtime.app.world().resource::<PendingCommands>();
        assert!(
            pending.app_commands.is_empty() && pending.editor_commands.is_empty(),
            "SnapSelectionToGrid block branch must not stage a legacy command"
        );
    }

    #[tokio::test]
    async fn runtime_carve_out_snap_selection_to_grid_propagates_tap_snap() {
        use eg_state::events::selection::SnapSelectionToGrid;

        let mut state = State::new_test().await;
        state.enter_editor_phase_for_test("SelectionSnapTap");
        state.set_editor_mode(eg_types::EditorMode::Tapping);
        state.set_editor_snap_step(1.0);
        state.set_editor_timeline_duration_seconds(8.0);
        state.set_editor_timeline_time_seconds(1.5);
        state.set_editor_tap_state_for_test(
            vec![0.5, 1.5],
            vec![[0.0, 0.0, 0.0], [1.2, 0.0, 1.7]],
            Some(1),
        );
        let mut runtime = Runtime::new(state);

        runtime.app.world_mut().write_message(SnapSelectionToGrid);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        let selected_index = runtime
            .state
            .editor_selected_tap_index_value()
            .expect("selected tap after snap");
        assert_eq!(
            runtime.state.editor_tap_indicator_positions()[selected_index],
            [1.0, 0.0, 2.0],
            "legacy refresh must preserve the snapped tap indicator"
        );
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimelineRes>()
                .0
                .taps
                .tap_indicator_positions[selected_index],
            [1.0, 0.0, 2.0],
            "runtime sync must copy the snapped indicator back into the App world"
        );
        assert_eq!(
            runtime.app.world().resource::<EditorInputRes>().0.cursor,
            [1.0, 0.0, 2.0],
            "runtime sync must keep the App cursor on the snapped selected tap"
        );
        let pending = runtime.app.world().resource::<PendingCommands>();
        assert!(
            pending.app_commands.is_empty() && pending.editor_commands.is_empty(),
            "SnapSelectionToGrid tap branch must not stage a legacy command"
        );
    }

    #[tokio::test]
    async fn runtime_carve_out_focus_camera_target_propagates_to_legacy_state() {
        use eg_state::events::selection::FocusCameraTarget;

        let mut state = State::new_test().await;
        state.enter_editor_phase_for_test("SelectionFocusCamera");
        state.set_editor_objects_from_app(vec![eg_types::LevelObject {
            position: [10.0, 2.0, 30.0],
            size: [2.0, 4.0, 6.0],
            ..eg_types::LevelObject::default()
        }]);
        state.set_editor_block_selection_from_app(Some(0), vec![0], Some(0));
        let mut runtime = Runtime::new(state);

        runtime.app.world_mut().write_message(FocusCameraTarget);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(runtime.state.editor_camera_pan(), [11.0, 33.0]);
        approx::assert_abs_diff_eq!(runtime.state.editor_camera_target_z(), 4.0, epsilon = 1e-6);
        let camera = &runtime.app.world().resource::<EditorCameraRes>().0;
        assert_eq!(camera.editor_pan, [11.0, 33.0]);
        approx::assert_abs_diff_eq!(camera.editor_target_z, 4.0, epsilon = 1e-6);
        let pending = runtime.app.world().resource::<PendingCommands>();
        assert!(
            pending.app_commands.is_empty() && pending.editor_commands.is_empty(),
            "FocusCameraTarget must not stage a legacy command"
        );
    }

    // --- Phase 3.7 / BPM carve-out tests ---
    //
    // The `bpm` event handlers in `eg_state::events::bpm` write
    // directly to `EditorTimingRes.0.bpm_tap_times` and
    // `bpm_tap_result`. `Runtime::run_app_frame` then calls
    // `sync_app_editor_timing_state` to copy the resource to the
    // legacy `State.editor.timing` fields before any dispatch.

    use eg_state::events::bpm::{BpmTap, BpmTapReset};

    /// Writing a `BpmTap` message reaches the App world, mutates
    /// `EditorTimingRes.0.bpm_tap_times`, and the per-frame sync
    /// propagates the value to the legacy `State.editor.timing`
    /// fields.
    #[tokio::test]
    async fn runtime_carve_out_bpm_tap_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        // Reset to a known empty state for the assertion.
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorTimingRes>()
            .0
            .bpm_tap_times
            .clear();
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorTimingRes>()
            .0
            .bpm_tap_result = None;
        runtime.state.set_editor_bpm_tap_state(Vec::new(), None);

        // Write the message; the next frame's schedule will run the
        // handler and write wall-clock time into the resource.
        runtime.app.world_mut().write_message(BpmTap);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        // App world is the source of truth: one tap recorded.
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .bpm_tap_times
                .len(),
            1,
            "BpmTap handler must append to EditorTimingRes.bpm_tap_times"
        );
        // Legacy state was synced to match the resource.
        assert_eq!(
            runtime.state.editor_bpm_tap_times().len(),
            1,
            "sync_app_editor_timing_state must propagate bpm_tap_times to legacy state"
        );
    }

    /// `BpmTapReset` is a carve-out: writing the message clears the
    /// resource's tap history and the sync propagates the cleared
    /// list to the legacy state.
    #[tokio::test]
    async fn runtime_carve_out_bpm_tap_reset_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        // Pre-populate both sides with a fake tap history.
        let mut res = runtime.app.world_mut().resource_mut::<EditorTimingRes>();
        res.0.bpm_tap_times = vec![1.0, 2.0, 3.0];
        res.0.bpm_tap_result = Some(120.0);
        drop(res);
        runtime
            .state
            .set_editor_bpm_tap_state(vec![1.0, 2.0, 3.0], Some(120.0));

        runtime.app.world_mut().write_message(BpmTapReset);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .bpm_tap_times
                .is_empty(),
            "BpmTapReset handler must clear EditorTimingRes.bpm_tap_times"
        );
        assert!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .bpm_tap_result
                .is_none(),
            "BpmTapReset handler must clear EditorTimingRes.bpm_tap_result"
        );
        assert!(
            runtime.state.editor_bpm_tap_times().is_empty(),
            "sync_app_editor_timing_state must propagate cleared bpm_tap_times to legacy state"
        );
        assert!(
            runtime.state.editor_bpm_tap_result().is_none(),
            "sync_app_editor_timing_state must propagate cleared bpm_tap_result to legacy state"
        );
    }

    /// No-op frame: when no `BpmTap`/`BpmTapReset` message is
    /// written and the resource matches the legacy state, the
    /// sync must not re-invoke the legacy setter.
    #[tokio::test]
    async fn runtime_carve_out_no_op_frame_does_not_resync_bpm_tap() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        // Seed both sides with a known tap history.
        let times = vec![1.0_f64, 2.0, 3.0];
        let result = Some(60.0_f32);
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorTimingRes>()
            .0
            .bpm_tap_times = times.clone();
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorTimingRes>()
            .0
            .bpm_tap_result = result;
        runtime
            .state
            .set_editor_bpm_tap_state(times.clone(), result);

        // Run a frame with no message: handler doesn't run, sync
        // sees no diff, setter not invoked.
        let _ = runtime.run_app_frame(egui::RawInput::default());

        // The resource and legacy state should still hold the seeded
        // values (the sync guard ensures no spurious write occurs).
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .bpm_tap_times,
            times
        );
        assert_eq!(runtime.state.editor_bpm_tap_times(), times.as_slice());
        assert_eq!(runtime.state.editor_bpm_tap_result(), result);
    }

    /// Regression: `EditorTimingRes` must be seeded from the legacy
    /// `State` at `Runtime::new` so the change-guarded bpm-tap sync
    /// does not clobber an in-flight tap history on the first
    /// frame. Same pattern as the snap / block-id first-frame
    /// clobber regressions.
    #[tokio::test]
    async fn runtime_seeds_editor_timing_res_from_legacy_state_no_first_frame_clobber() {
        let mut state = State::new_test().await;
        // User-driven tap history + computed result. The default
        // resource is empty (`bpm_tap_times = []`,
        // `bpm_tap_result = None`); without seeding, the change-
        // guarded sync would see a diff and clobber the user value
        // back to defaults on the first frame.
        let times = vec![10.0_f64, 11.0, 12.0];
        let result = Some(60.0_f32);
        state.set_editor_bpm_tap_state(times.clone(), result);

        let mut runtime = Runtime::new(state);

        // Seeding: the resource must reflect the legacy user value,
        // not the `EditorTimingState` default.
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .bpm_tap_times,
            times,
            "EditorTimingRes must be seeded from legacy State at Runtime::new"
        );
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .bpm_tap_result,
            result,
            "EditorTimingRes.bpm_tap_result must be seeded from legacy State"
        );

        // Run a frame with no bpm message. The change-guarded sync
        // must see no diff and must NOT invoke the legacy setter, so
        // the user value survives the first frame.
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime.state.editor_bpm_tap_times(),
            times.as_slice(),
            "first-frame sync must not clobber user bpm_tap_times"
        );
        assert_eq!(
            runtime.state.editor_bpm_tap_result(),
            result,
            "first-frame sync must not clobber user bpm_tap_result"
        );
    }

    // --- Phase 3.8 / Session carve-out tests ---
    //
    // The session event handlers in `eg_state::events::session`
    // write directly to `SessionStateRes.editor_level_name`,
    // `editor_music_metadata`, `editor_menu_preview_camera`,
    // `editor_show_metadata`,
    // `editor_show_settings`, `editor_settings_section`,
    // `editor_keybind_capture_action`, and
    // `app_settings.ui_scale_multiplier` / `keybinds`,
    // `editor_creator_metadata`, and `editor_sky_color`.
    // `Runtime::run_app_frame` then calls `sync_app_session_state`
    // to copy the resource to the legacy `State.session` fields
    // before any dispatch.

    use eg_state::events::session::{
        CaptureMenuPreviewCamera, ClearKeybindSlot, RenameLevel, ResetKeybind, ResetKeybinds,
        SetGraphicsBackend, SetKeybind, SetKeybindCapture, SetSettingsSection, SetShowMetadata,
        SetShowSettings, SetUiScaleMultiplier, ToggleSettings, UpdateCreatorMetadata, UpdateMusic,
        UpdateSkyColor, UseAutoMenuPreviewCamera,
    };
    use eg_types::{
        BasicLevelMetadata, KeyChord, KeybindBinding, LevelPreviewCameraMetadata, MusicMetadata,
        SettingsSection, default_essential_keybinds,
    };

    #[tokio::test]
    async fn runtime_carve_out_rename_level_propagates_to_legacy_state() {
        let mut state = State::new_test().await;
        state.set_editor_level_name("BeforeRename".to_string());
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .write_message(RenameLevel("AfterRename".to_string()));
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_level_name
                .as_deref(),
            Some("AfterRename"),
            "RenameLevel handler must mutate SessionStateRes.editor_level_name"
        );
        assert_eq!(
            runtime.state.editor_level_name().as_deref(),
            Some("AfterRename"),
            "sync_app_session_state must propagate editor_level_name to legacy state"
        );
    }

    #[tokio::test]
    async fn runtime_carve_out_update_music_propagates_to_legacy_state() {
        let mut state = State::new_test().await;
        state.set_editor_music_metadata(MusicMetadata::default());
        let mut runtime = Runtime::new(state);
        let metadata = MusicMetadata {
            title: Some("Runtime Track".to_string()),
            author: Some("App World".to_string()),
        };

        runtime
            .app
            .world_mut()
            .write_message(UpdateMusic(metadata.clone()));
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            &runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_music_metadata,
            &metadata,
            "UpdateMusic handler must mutate SessionStateRes.editor_music_metadata"
        );
        assert_eq!(
            runtime.state.editor_music_metadata(),
            &metadata,
            "sync_app_session_state must propagate editor_music_metadata to legacy state"
        );
    }

    #[tokio::test]
    async fn runtime_carve_out_legacy_name_and_music_survive_next_frame_without_message() {
        let mut state = State::new_test().await;
        state.set_editor_level_name("LegacyName".to_string());
        let metadata = MusicMetadata {
            title: Some("Legacy Track".to_string()),
            author: Some("Legacy Artist".to_string()),
        };
        state.set_editor_music_metadata(metadata.clone());
        let mut runtime = Runtime::new(state);

        {
            let mut session = runtime.app.world_mut().resource_mut::<SessionStateRes>();
            session.editor_level_name = Some("StaleName".to_string());
            session.editor_music_metadata = MusicMetadata::default();
        }

        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime.state.editor_level_name().as_deref(),
            Some("LegacyName")
        );
        assert_eq!(runtime.state.editor_music_metadata(), &metadata);
        let session = runtime.app.world().resource::<SessionStateRes>();
        assert_eq!(session.editor_level_name.as_deref(), Some("LegacyName"));
        assert_eq!(
            &session.editor_music_metadata, &metadata,
            "legacy name/music mirror must protect against stale resource clobber"
        );
    }

    /// Writing a `SetShowMetadata(true)` message reaches the App
    /// world, mutates `SessionStateRes.editor_show_metadata`, and
    /// the per-frame sync propagates the value to the legacy state.
    #[tokio::test]
    async fn runtime_carve_out_set_show_metadata_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        // Reset the resource to a known value so we can detect the
        // mutation. The seeded value reflects the legacy state's
        // default (`editor_show_metadata = false`).
        runtime
            .app
            .world_mut()
            .resource_mut::<SessionStateRes>()
            .editor_show_metadata = false;

        runtime.app.world_mut().write_message(SetShowMetadata(true));
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_show_metadata,
            "SetShowMetadata handler must mutate SessionStateRes.editor_show_metadata"
        );
        assert!(
            runtime.state.editor_show_metadata(),
            "sync_app_session_state must propagate editor_show_metadata to legacy state"
        );
    }

    /// `SetShowSettings` carve-out: same pattern as
    /// `SetShowMetadata`.
    #[tokio::test]
    async fn runtime_carve_out_set_show_settings_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .resource_mut::<SessionStateRes>()
            .editor_show_settings = false;
        runtime.state.set_editor_show_settings(false);

        runtime.app.world_mut().write_message(SetShowSettings(true));
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(runtime.state.editor_show_settings());
        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_show_settings
        );
    }

    /// `ToggleSettings` flips the resource value; the sync
    /// propagates the new value to the legacy state.
    #[tokio::test]
    async fn runtime_carve_out_toggle_settings_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        let initial = runtime
            .app
            .world()
            .resource::<SessionStateRes>()
            .editor_show_settings;
        runtime.state.set_editor_show_settings(initial);

        runtime.app.world_mut().write_message(ToggleSettings);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        let expected = !initial;
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_show_settings,
            expected,
            "ToggleSettings must flip SessionStateRes.editor_show_settings"
        );
        assert_eq!(
            runtime.state.editor_show_settings(),
            expected,
            "sync_app_session_state must propagate toggled editor_show_settings to legacy state"
        );
    }

    /// `SetSettingsSection` propagates a non-default `SettingsSection`
    /// value to the legacy state.
    #[tokio::test]
    async fn runtime_carve_out_set_settings_section_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .resource_mut::<SessionStateRes>()
            .editor_settings_section = SettingsSection::Backends;
        runtime
            .state
            .set_editor_settings_section(SettingsSection::Backends);

        let new_section = SettingsSection::Keybinds;
        runtime
            .app
            .world_mut()
            .write_message(SetSettingsSection(new_section));
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_settings_section,
            new_section
        );
        assert_eq!(runtime.state.editor_settings_section(), new_section);
    }

    /// `SetKeybindCapture` propagates a `Some((String, usize))` to
    /// the legacy state.
    #[tokio::test]
    async fn runtime_carve_out_set_keybind_capture_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .resource_mut::<SessionStateRes>()
            .editor_keybind_capture_action = None;
        runtime.state.set_editor_keybind_capture_action(None);

        let capture = Some(("editor.place".to_string(), 0));
        runtime
            .app
            .world_mut()
            .write_message(SetKeybindCapture(capture.clone()));
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_keybind_capture_action,
            capture
        );
        assert_eq!(
            runtime.state.editor_keybind_capture_action(),
            Some(&("editor.place".to_string(), 0))
        );
    }

    #[tokio::test]
    async fn runtime_carve_out_set_ui_scale_multiplier_propagates_to_legacy_state() {
        let mut state = State::new_test().await;
        state.set_ui_scale_multiplier(1.0);
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .write_message(SetUiScaleMultiplier(8.0));
        let _ = runtime.run_app_frame(egui::RawInput::default());

        approx::assert_abs_diff_eq!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .app_settings
                .ui_scale_multiplier,
            3.0,
            epsilon = 1e-6
        );
        approx::assert_abs_diff_eq!(
            runtime.state.app_settings().ui_scale_multiplier,
            3.0,
            epsilon = 1e-6
        );
    }

    #[tokio::test]
    async fn runtime_carve_out_set_graphics_backend_propagates_to_legacy_state() {
        let mut state = State::new_test().await;
        state.set_preferred_graphics_backend("Auto".to_string());
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .write_message(SetGraphicsBackend("ManualBackend".to_string()));
        let _ = runtime.run_app_frame(egui::RawInput::default());

        let session = runtime.app.world().resource::<SessionStateRes>();
        assert_eq!(session.app_settings.graphics_backend, "ManualBackend");
        assert!(
            session.settings_restart_required,
            "SetGraphicsBackend handler must mark restart required in SessionStateRes"
        );
        assert_eq!(
            runtime.state.app_settings().graphics_backend,
            "ManualBackend",
            "sync_app_session_state must propagate graphics backend to legacy state"
        );
        assert!(runtime.state.settings_restart_required());
    }

    #[tokio::test]
    async fn runtime_carve_out_set_graphics_backend_respects_legacy_lock() {
        let mut state = State::new_test().await;
        state.set_preferred_graphics_backend("LockedBackend".to_string());
        state.set_graphics_backend_locked_for_test(true);
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .write_message(SetGraphicsBackend("IgnoredBackend".to_string()));
        let _ = runtime.run_app_frame(egui::RawInput::default());

        let session = runtime.app.world().resource::<SessionStateRes>();
        assert_eq!(session.app_settings.graphics_backend, "LockedBackend");
        assert_eq!(
            runtime.state.app_settings().graphics_backend,
            "LockedBackend",
            "locked graphics backend must ignore App-world SetGraphicsBackend"
        );
    }

    #[tokio::test]
    async fn runtime_carve_out_set_keybind_propagates_to_legacy_state() {
        let mut state = State::new_test().await;
        state.set_keybind_for_action(
            "copy".to_string(),
            0,
            KeyChord::new("c", true, false, false),
        );
        let mut runtime = Runtime::new(state);
        let chord = KeyChord::new("k", true, false, false);

        runtime.app.world_mut().write_message(SetKeybind {
            action: "copy".to_string(),
            slot: 0,
            chord: chord.clone(),
        });
        let _ = runtime.run_app_frame(egui::RawInput::default());

        let session = runtime.app.world().resource::<SessionStateRes>();
        assert!(
            session
                .app_settings
                .keybinds_for_action("copy")
                .iter()
                .any(|candidate| **candidate == chord),
            "SetKeybind handler must mutate SessionStateRes.app_settings.keybinds"
        );
        assert!(
            runtime
                .state
                .app_settings()
                .keybinds_for_action("copy")
                .iter()
                .any(|candidate| **candidate == chord),
            "sync_app_session_state must propagate keybinds to legacy state"
        );
    }

    #[tokio::test]
    async fn runtime_carve_out_clear_keybind_slot_propagates_to_legacy_state() {
        let mut state = State::new_test().await;
        state.set_keybind_for_action(
            "copy".to_string(),
            0,
            KeyChord::new("k", true, false, false),
        );
        let mut runtime = Runtime::new(state);

        runtime.app.world_mut().write_message(ClearKeybindSlot {
            action: "copy".to_string(),
            slot: 0,
        });
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .app_settings
                .keybinds_for_action("copy")
                .is_empty(),
            "ClearKeybindSlot handler must mutate SessionStateRes.app_settings.keybinds"
        );
        assert!(
            runtime
                .state
                .app_settings()
                .keybinds_for_action("copy")
                .is_empty(),
            "sync_app_session_state must propagate cleared keybinds to legacy state"
        );
    }

    #[tokio::test]
    async fn runtime_carve_out_reset_keybind_propagates_to_legacy_state() {
        let mut state = State::new_test().await;
        state.set_keybind_for_action(
            "copy".to_string(),
            0,
            KeyChord::new("k", true, false, false),
        );
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .write_message(ResetKeybind("copy".to_string()));
        let _ = runtime.run_app_frame(egui::RawInput::default());

        let expected_defaults: Vec<_> = default_essential_keybinds()
            .into_iter()
            .filter(|binding| binding.action == "copy")
            .collect();
        let resource_actual: Vec<_> = runtime
            .app
            .world()
            .resource::<SessionStateRes>()
            .app_settings
            .keybinds
            .iter()
            .filter(|binding| binding.action == "copy")
            .cloned()
            .collect();
        let legacy_actual: Vec<_> = runtime
            .state
            .app_settings()
            .keybinds
            .iter()
            .filter(|binding| binding.action == "copy")
            .cloned()
            .collect();
        assert_eq!(resource_actual, expected_defaults.clone());
        assert_eq!(legacy_actual, expected_defaults);
    }

    #[tokio::test]
    async fn runtime_carve_out_reset_keybinds_propagates_to_legacy_state() {
        let mut state = State::new_test().await;
        state.set_keybind_for_action(
            "copy".to_string(),
            0,
            KeyChord::new("k", true, false, false),
        );
        let custom = KeybindBinding {
            action: "custom.action".to_string(),
            chord: KeyChord::new("x", false, false, false),
        };
        let mut custom_keybinds = state.app_settings().keybinds.clone();
        custom_keybinds.push(custom.clone());
        state.set_app_settings_keybinds(custom_keybinds);
        let mut runtime = Runtime::new(state);

        runtime.app.world_mut().write_message(ResetKeybinds);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        let session = runtime.app.world().resource::<SessionStateRes>();
        assert!(session.app_settings.keybinds.contains(&custom));
        assert!(runtime.state.app_settings().keybinds.contains(&custom));
        for default_binding in default_essential_keybinds() {
            assert!(session.app_settings.keybinds.contains(&default_binding));
            assert!(
                runtime
                    .state
                    .app_settings()
                    .keybinds
                    .contains(&default_binding)
            );
        }
    }

    #[tokio::test]
    async fn runtime_carve_out_legacy_keybinds_survive_next_frame_without_message() {
        let mut state = State::new_test().await;
        let chord = KeyChord::new("k", true, false, false);
        state.set_keybind_for_action("copy".to_string(), 0, chord.clone());
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .resource_mut::<SessionStateRes>()
            .app_settings
            .keybinds
            .clear();

        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(
            runtime
                .state
                .app_settings()
                .keybinds_for_action("copy")
                .iter()
                .any(|candidate| **candidate == chord)
        );
        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .app_settings
                .keybinds_for_action("copy")
                .iter()
                .any(|candidate| **candidate == chord),
            "legacy keybind mirror must protect against stale resource clobber"
        );
    }

    #[tokio::test]
    async fn runtime_carve_out_legacy_graphics_backend_survives_next_frame_without_message() {
        let mut state = State::new_test().await;
        state.set_preferred_graphics_backend("LegacyBackend".to_string());
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .resource_mut::<SessionStateRes>()
            .app_settings
            .graphics_backend = "StaleBackend".to_string();

        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime.state.app_settings().graphics_backend,
            "LegacyBackend"
        );
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .app_settings
                .graphics_backend,
            "LegacyBackend",
            "legacy graphics backend mirror must protect against stale resource clobber"
        );
    }

    #[tokio::test]
    async fn runtime_carve_out_legacy_ui_scale_survives_next_frame_without_message() {
        let mut state = State::new_test().await;
        state.set_ui_scale_multiplier(1.75);
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .resource_mut::<SessionStateRes>()
            .app_settings
            .ui_scale_multiplier = 1.0;

        let _ = runtime.run_app_frame(egui::RawInput::default());

        approx::assert_abs_diff_eq!(
            runtime.state.app_settings().ui_scale_multiplier,
            1.75,
            epsilon = 1e-6
        );
        approx::assert_abs_diff_eq!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .app_settings
                .ui_scale_multiplier,
            1.75,
            epsilon = 1e-6
        );
    }

    #[tokio::test]
    async fn runtime_carve_out_update_creator_metadata_propagates_to_legacy_state() {
        let mut state = State::new_test().await;
        state.set_editor_creator_metadata(BasicLevelMetadata::default());
        let mut runtime = Runtime::new(state);
        let metadata = BasicLevelMetadata {
            name: "Runtime Metadata".to_string(),
            creator: Some("codex".to_string()),
            description: Some("direct session metadata".to_string()),
            stars: 7.5,
            tags: vec!["ecs".to_string(), "session".to_string()],
        };

        runtime
            .app
            .world_mut()
            .write_message(UpdateCreatorMetadata(metadata.clone()));
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            &runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_creator_metadata,
            &metadata,
            "UpdateCreatorMetadata handler must mutate SessionStateRes.editor_creator_metadata"
        );
        assert_eq!(
            runtime.state.editor_creator_metadata(),
            &metadata,
            "sync_app_session_state must propagate editor_creator_metadata to legacy state"
        );
    }

    #[tokio::test]
    async fn runtime_carve_out_legacy_creator_metadata_survives_next_frame_without_message() {
        let mut state = State::new_test().await;
        let metadata = BasicLevelMetadata {
            name: "Legacy Metadata".to_string(),
            creator: Some("legacy".to_string()),
            description: Some("survives stale app resource".to_string()),
            stars: 5.5,
            tags: vec!["legacy".to_string()],
        };
        state.set_editor_creator_metadata(metadata.clone());
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .resource_mut::<SessionStateRes>()
            .editor_creator_metadata = BasicLevelMetadata::default();

        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(runtime.state.editor_creator_metadata(), &metadata);
        assert_eq!(
            &runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_creator_metadata,
            &metadata,
            "legacy creator metadata mirror must protect against stale resource clobber"
        );
    }

    #[tokio::test]
    async fn runtime_carve_out_update_sky_color_propagates_to_legacy_state() {
        let mut state = State::new_test().await;
        state.set_editor_sky_color([0.0, 0.0, 0.0]);
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .write_message(UpdateSkyColor([-1.0, 0.5, 2.0]));
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_sky_color,
            [0.0, 0.5, 1.0],
            "UpdateSkyColor handler must clamp and mutate SessionStateRes.editor_sky_color"
        );
        assert_eq!(
            runtime.state.editor_sky_color(),
            [0.0, 0.5, 1.0],
            "sync_app_session_state must propagate editor_sky_color to legacy state"
        );
    }

    #[tokio::test]
    async fn runtime_carve_out_legacy_sky_color_survives_next_frame_without_message() {
        let mut state = State::new_test().await;
        state.set_editor_sky_color([0.25, 0.5, 0.75]);
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .resource_mut::<SessionStateRes>()
            .editor_sky_color = [0.0, 0.0, 0.0];

        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(runtime.state.editor_sky_color(), [0.25, 0.5, 0.75]);
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_sky_color,
            [0.25, 0.5, 0.75],
            "legacy sky color mirror must protect against stale resource clobber"
        );
    }

    #[tokio::test]
    async fn runtime_carve_out_capture_menu_preview_camera_propagates_to_legacy_state() {
        let mut state = State::new_test().await;
        state.enter_editor_phase_for_test("PreviewCameraCapture");
        state.set_editor_camera_pose_for_test(
            [2.0, -4.0],
            3.0,
            30.0f32.to_radians(),
            40.0f32.to_radians(),
        );
        let mut runtime = Runtime::new(state);
        let expected = {
            let camera = &runtime
                .app
                .world()
                .resource::<eg_state::state::EditorCameraRes>()
                .0;
            let target = camera.editor_camera_target();
            LevelPreviewCameraMetadata {
                position: (target + camera.editor_camera_offset()).to_array(),
                target: target.to_array(),
            }
        };

        runtime
            .app
            .world_mut()
            .write_message(CaptureMenuPreviewCamera);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_menu_preview_camera,
            Some(expected.clone()),
            "CaptureMenuPreviewCamera handler must mutate SessionStateRes.editor_menu_preview_camera"
        );
        assert_eq!(
            runtime.state.editor_menu_preview_camera(),
            Some(&expected),
            "sync_app_session_state must propagate menu preview camera to legacy state"
        );
    }

    #[tokio::test]
    async fn runtime_carve_out_use_auto_menu_preview_camera_propagates_to_legacy_state() {
        let mut state = State::new_test().await;
        state.enter_editor_phase_for_test("PreviewCameraClear");
        state.set_editor_menu_preview_camera(Some(LevelPreviewCameraMetadata {
            position: [9.0, 8.0, 7.0],
            target: [6.0, 5.0, 4.0],
        }));
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .write_message(UseAutoMenuPreviewCamera);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_menu_preview_camera,
            None,
            "UseAutoMenuPreviewCamera handler must clear SessionStateRes.editor_menu_preview_camera"
        );
        assert_eq!(
            runtime.state.editor_menu_preview_camera(),
            None,
            "sync_app_session_state must propagate cleared menu preview camera to legacy state"
        );
    }

    #[tokio::test]
    async fn runtime_carve_out_legacy_menu_preview_camera_survives_next_frame_without_message() {
        let mut state = State::new_test().await;
        let manual = LevelPreviewCameraMetadata {
            position: [9.0, 8.0, 7.0],
            target: [6.0, 5.0, 4.0],
        };
        state.set_editor_menu_preview_camera(Some(manual.clone()));
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .resource_mut::<SessionStateRes>()
            .editor_menu_preview_camera = None;

        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(runtime.state.editor_menu_preview_camera(), Some(&manual));
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_menu_preview_camera,
            Some(manual),
            "legacy menu preview camera mirror must protect against stale resource clobber"
        );
    }

    /// Regression: `SessionStateRes` (carved-out fields) must be
    /// seeded from the legacy `State` at `Runtime::new` so the
    /// change-guarded session sync does not clobber a user-driven
    /// UI-window state on the first frame.
    #[tokio::test]
    async fn runtime_seeds_session_state_res_from_legacy_state_no_first_frame_clobber() {
        let mut state = State::new_test().await;
        // User-driven UI-window state. The default resource has
        // `editor_show_metadata = false`, `editor_show_settings =
        // false`, `editor_settings_section = Backends`,
        // `editor_keybind_capture_action = None`. We flip one of
        // them to a non-default value so the first-frame sync
        // would clobber it without the seeding fix.
        state.set_editor_show_metadata(true);
        state.set_editor_show_settings(true);
        state.set_editor_settings_section(SettingsSection::Keybinds);
        state.set_editor_keybind_capture_action(Some(("editor.place".into(), 0)));
        state.set_ui_scale_multiplier(1.75);
        state.set_preferred_graphics_backend("SeededBackend".to_string());
        let seeded_chord = KeyChord::new("k", true, false, false);
        state.set_keybind_for_action("copy".to_string(), 0, seeded_chord.clone());
        state.set_editor_level_name("SeededLevel".to_string());
        let music_metadata = MusicMetadata {
            title: Some("Seeded Track".to_string()),
            author: Some("Seeded Artist".to_string()),
        };
        state.set_editor_music_metadata(music_metadata.clone());
        let creator_metadata = BasicLevelMetadata {
            name: "Seeded Metadata".to_string(),
            creator: Some("seed".to_string()),
            description: Some("session resource seed".to_string()),
            stars: 8.0,
            tags: vec!["seed".to_string(), "session".to_string()],
        };
        state.set_editor_creator_metadata(creator_metadata.clone());
        state.set_editor_sky_color([0.25, 0.5, 0.75]);
        let menu_preview_camera = LevelPreviewCameraMetadata {
            position: [9.0, 8.0, 7.0],
            target: [6.0, 5.0, 4.0],
        };
        state.set_editor_menu_preview_camera(Some(menu_preview_camera.clone()));

        let mut runtime = Runtime::new(state);

        // Seeding: the resource must reflect the legacy user
        // values, not the `SessionStateRes` default.
        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_show_metadata,
            "SessionStateRes.editor_show_metadata must be seeded from legacy State"
        );
        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_show_settings,
            "SessionStateRes.editor_show_settings must be seeded from legacy State"
        );
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_settings_section,
            SettingsSection::Keybinds,
            "SessionStateRes.editor_settings_section must be seeded from legacy State"
        );
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_keybind_capture_action,
            Some(("editor.place".to_string(), 0))
        );
        approx::assert_abs_diff_eq!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .app_settings
                .ui_scale_multiplier,
            1.75,
            epsilon = 1e-6
        );
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_sky_color,
            [0.25, 0.5, 0.75],
            "SessionStateRes.editor_sky_color must be seeded from legacy State"
        );
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_menu_preview_camera,
            Some(menu_preview_camera.clone()),
            "SessionStateRes.editor_menu_preview_camera must be seeded from legacy State"
        );
        assert_eq!(
            &runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_creator_metadata,
            &creator_metadata,
            "SessionStateRes.editor_creator_metadata must be seeded from legacy State"
        );
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_level_name
                .as_deref(),
            Some("SeededLevel"),
            "SessionStateRes.editor_level_name must be seeded from legacy State"
        );
        assert_eq!(
            &runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_music_metadata,
            &music_metadata,
            "SessionStateRes.editor_music_metadata must be seeded from legacy State"
        );
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .app_settings
                .graphics_backend,
            "SeededBackend",
            "SessionStateRes.app_settings.graphics_backend must be seeded from legacy State"
        );
        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .settings_restart_required,
            "SessionStateRes.settings_restart_required must be seeded from legacy State"
        );
        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .app_settings
                .keybinds_for_action("copy")
                .iter()
                .any(|candidate| **candidate == seeded_chord),
            "SessionStateRes.app_settings.keybinds must be seeded from legacy State"
        );

        // Run a frame with no session message. The change-guarded
        // sync must see no diff and must NOT invoke the legacy
        // setters, so the user values survive the first frame.
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(runtime.state.editor_show_metadata());
        assert!(runtime.state.editor_show_settings());
        assert_eq!(
            runtime.state.editor_settings_section(),
            SettingsSection::Keybinds
        );
        assert_eq!(
            runtime.state.editor_keybind_capture_action(),
            Some(&("editor.place".to_string(), 0))
        );
        approx::assert_abs_diff_eq!(
            runtime.state.app_settings().ui_scale_multiplier,
            1.75,
            epsilon = 1e-6
        );
        assert_eq!(
            runtime.state.editor_sky_color(),
            [0.25, 0.5, 0.75],
            "first-frame sync must not clobber user editor_sky_color"
        );
        assert_eq!(
            runtime.state.editor_menu_preview_camera(),
            Some(&menu_preview_camera),
            "first-frame sync must not clobber user editor_menu_preview_camera"
        );
        assert_eq!(
            runtime.state.editor_creator_metadata(),
            &creator_metadata,
            "first-frame sync must not clobber user editor_creator_metadata"
        );
        assert_eq!(
            runtime.state.editor_level_name().as_deref(),
            Some("SeededLevel"),
            "first-frame sync must not clobber user editor_level_name"
        );
        assert_eq!(
            runtime.state.editor_music_metadata(),
            &music_metadata,
            "first-frame sync must not clobber user editor_music_metadata"
        );
        assert_eq!(
            runtime.state.app_settings().graphics_backend,
            "SeededBackend",
            "first-frame sync must not clobber user graphics_backend"
        );
        assert!(
            runtime
                .state
                .app_settings()
                .keybinds_for_action("copy")
                .iter()
                .any(|candidate| **candidate == seeded_chord),
            "first-frame sync must not clobber user keybinds"
        );
    }

    // ---- Phase 3.9 timing carve-out tests ----
    //
    // All 6 `events::timing::*` messages are carve-outs that mutate
    // `EditorTimingRes.0.{timing_points, timing_selected_index}`
    // directly. The runtime tests below pin the end-to-end path:
    // message → App-world resource → legacy State field.

    use eg_state::events::timing::{
        AddTimingPoint, RemoveTimingPoint, SetTimingPointBpm, SetTimingPointTime,
        SetTimingPointTimeSignature, SetTimingSelected,
    };

    /// `AddTimingPoint` end-to-end: the message reaches the App world,
    /// mutates `EditorTimingRes.0.timing_points`, and the per-frame
    /// sync propagates the new list to the legacy `State.editor.timing`.
    #[tokio::test]
    async fn runtime_carve_out_add_timing_point_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        // Precondition: empty list on both sides (the seed is empty for
        // a fresh State::new_test).
        assert!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .timing_points
                .is_empty()
        );
        assert!(runtime.state.editor_timing_points().is_empty());

        runtime.app.world_mut().write_message(AddTimingPoint {
            time_seconds: 1.0,
            bpm: 120.0,
        });
        let _ = runtime.run_app_frame(egui::RawInput::default());

        // Resource holds the new point.
        let timing = &runtime.app.world().resource::<EditorTimingRes>().0;
        assert_eq!(timing.timing_points.len(), 1);
        assert_eq!(timing.timing_points[0].time_seconds, 1.0);
        // Legacy state matches the resource after the sync.
        assert_eq!(runtime.state.editor_timing_points().len(), 1);
        approx::assert_abs_diff_eq!(
            runtime.state.editor_timing_points()[0].time_seconds,
            1.0,
            epsilon = 1e-6
        );
        approx::assert_abs_diff_eq!(
            runtime.state.editor_timing_points()[0].bpm,
            120.0,
            epsilon = 1e-6
        );
    }

    /// `RemoveTimingPoint` end-to-end: list shrinks via the resource,
    /// the legacy state mirrors it after the sync.
    #[tokio::test]
    async fn runtime_carve_out_remove_timing_point_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        // Pre-populate both sides with a single timing point so we can
        // remove it through the Bevy path.
        let initial = eg_types::TimingPoint {
            time_seconds: 0.5,
            bpm: 90.0,
            time_signature_numerator: 4,
            time_signature_denominator: 4,
        };
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorTimingRes>()
            .0
            .timing_points = vec![initial.clone()];
        runtime
            .state
            .set_editor_timing_state(vec![initial.clone()], None);

        runtime.app.world_mut().write_message(RemoveTimingPoint(0));
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .timing_points
                .is_empty()
        );
        assert!(runtime.state.editor_timing_points().is_empty());
    }

    /// `SetTimingPointTime` end-to-end: the timestamp update flows
    /// through the resource and the legacy state picks it up via the
    /// change-guarded sync.
    #[tokio::test]
    async fn runtime_carve_out_set_timing_point_time_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        let initial = eg_types::TimingPoint {
            time_seconds: 0.0,
            bpm: 120.0,
            time_signature_numerator: 4,
            time_signature_denominator: 4,
        };
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorTimingRes>()
            .0
            .timing_points = vec![initial.clone()];
        runtime
            .state
            .set_editor_timing_state(vec![initial.clone()], None);

        runtime.app.world_mut().write_message(SetTimingPointTime {
            index: 0,
            time: 2.5,
        });
        let _ = runtime.run_app_frame(egui::RawInput::default());

        approx::assert_abs_diff_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .timing_points[0]
                .time_seconds,
            2.5,
            epsilon = 1e-6
        );
        approx::assert_abs_diff_eq!(
            runtime.state.editor_timing_points()[0].time_seconds,
            2.5,
            epsilon = 1e-6
        );
    }

    /// `SetTimingPointBpm` end-to-end: BPM update propagates with
    /// the >= 1.0 clamp.
    #[tokio::test]
    async fn runtime_carve_out_set_timing_point_bpm_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        let initial = eg_types::TimingPoint {
            time_seconds: 0.0,
            bpm: 120.0,
            time_signature_numerator: 4,
            time_signature_denominator: 4,
        };
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorTimingRes>()
            .0
            .timing_points = vec![initial.clone()];
        runtime
            .state
            .set_editor_timing_state(vec![initial.clone()], None);

        runtime.app.world_mut().write_message(SetTimingPointBpm {
            index: 0,
            bpm: 0.25,
        });
        let _ = runtime.run_app_frame(egui::RawInput::default());

        approx::assert_abs_diff_eq!(
            runtime.state.editor_timing_points()[0].bpm,
            1.0,
            epsilon = 1e-6
        );
    }

    /// `SetTimingPointTimeSignature` end-to-end: the time-signature
    /// fields update on both sides.
    #[tokio::test]
    async fn runtime_carve_out_set_timing_point_time_signature_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        let initial = eg_types::TimingPoint {
            time_seconds: 0.0,
            bpm: 120.0,
            time_signature_numerator: 4,
            time_signature_denominator: 4,
        };
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorTimingRes>()
            .0
            .timing_points = vec![initial.clone()];
        runtime
            .state
            .set_editor_timing_state(vec![initial.clone()], None);

        runtime
            .app
            .world_mut()
            .write_message(SetTimingPointTimeSignature {
                index: 0,
                numerator: 3,
                denominator: 8,
            });
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime.state.editor_timing_points()[0].time_signature_numerator,
            3
        );
        assert_eq!(
            runtime.state.editor_timing_points()[0].time_signature_denominator,
            8
        );
    }

    /// `SetTimingSelected` end-to-end: the selected-index update
    /// propagates to the legacy state.
    #[tokio::test]
    async fn runtime_carve_out_set_timing_selected_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        // Initial: None on both sides.
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .timing_selected_index,
            None
        );
        assert_eq!(runtime.state.editor_timing_selected_index(), None);

        runtime
            .app
            .world_mut()
            .write_message(SetTimingSelected(Some(2)));
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .timing_selected_index,
            Some(2)
        );
        assert_eq!(runtime.state.editor_timing_selected_index(), Some(2));
    }

    /// No-op frame: when no timing message is written and the
    /// resource matches the legacy state (the seeded invariant), the
    /// sync must not re-invoke `set_editor_timing_state`. Pin the
    /// list length stays the same.
    #[tokio::test]
    async fn runtime_carve_out_no_op_frame_does_not_resync_timing() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        let initial = eg_types::TimingPoint {
            time_seconds: 0.0,
            bpm: 120.0,
            time_signature_numerator: 4,
            time_signature_denominator: 4,
        };
        {
            let mut res = runtime.app.world_mut().resource_mut::<EditorTimingRes>();
            res.0.timing_points = vec![initial.clone()];
            res.0.timing_selected_index = Some(0);
        }
        runtime
            .state
            .set_editor_timing_state(vec![initial.clone()], Some(0));

        // Pre-condition: both sides match.
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .timing_points
                .len(),
            1
        );
        assert_eq!(runtime.state.editor_timing_points().len(), 1);
        assert_eq!(runtime.state.editor_timing_selected_index(), Some(0));

        let _ = runtime.run_app_frame(egui::RawInput::default());

        // Post-condition: still matching, no extra writes.
        assert_eq!(runtime.state.editor_timing_points().len(), 1);
        assert_eq!(runtime.state.editor_timing_selected_index(), Some(0));
    }

    /// Regression: `EditorTimingRes` is seeded from the legacy
    /// `State` at `Runtime::new` (this is what the bpm-tap carve-out
    /// added). The new timing-points fields ride on the same
    /// resource, so the same seeding covers them: a non-default
    /// list on the legacy state at construction time must survive
    /// the first frame even when no timing message is written.
    #[tokio::test]
    async fn runtime_seeds_editor_timing_res_for_timing_points_no_first_frame_clobber() {
        let mut state = State::new_test().await;
        let user_points = vec![
            eg_types::TimingPoint {
                time_seconds: 0.0,
                bpm: 140.0,
                time_signature_numerator: 4,
                time_signature_denominator: 4,
            },
            eg_types::TimingPoint {
                time_seconds: 4.0,
                bpm: 160.0,
                time_signature_numerator: 3,
                time_signature_denominator: 4,
            },
        ];
        state.set_editor_timing_state(user_points.clone(), Some(1));
        let mut runtime = Runtime::new(state);

        // Seeding: the resource reflects the legacy user value.
        let seeded = &runtime.app.world().resource::<EditorTimingRes>().0;
        assert_eq!(seeded.timing_points.len(), 2);
        assert_eq!(seeded.timing_points[0].bpm, 140.0);
        assert_eq!(seeded.timing_points[1].bpm, 160.0);
        assert_eq!(seeded.timing_selected_index, Some(1));

        // No-op frame: the change-guarded sync sees no diff and
        // does not clobber the user value.
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(runtime.state.editor_timing_points().len(), 2);
        assert_eq!(runtime.state.editor_timing_selected_index(), Some(1));
    }

    // ---- Phase 3.16 timeline setting carve-out tests ----
    //
    // The pure timeline-setting messages in `events::timeline`
    // (`ShiftTimeline`, `SetTimelineTime`, `SetTimelineDuration`,
    // `SetPlaybackSpeed`, `SetWaveformZoom`, `SetWaveformScroll`)
    // now write directly to `EditorTimelineRes` / `EditorTimingRes`.
    // The runtime tests below pin the end-to-end path:
    // message → App-world resource → legacy State field.

    use eg_state::events::timeline::{
        AddTap, ClearTaps, RemoveTap, RemoveTapAt, SetPlaybackSpeed, SetSelectedTap,
        SetSelectedTapTime, SetTimelineDuration, SetTimelineTime, SetWaveformScroll,
        SetWaveformZoom, ShiftTimeline, ToggleTimelinePlayback,
    };

    #[tokio::test]
    async fn runtime_carve_out_shift_timeline_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .resource_mut::<EditorTimelineRes>()
            .0
            .clock
            .duration_seconds = 4.0;
        runtime.state.set_editor_timeline_duration_seconds(4.0);
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorTimelineRes>()
            .0
            .clock
            .time_seconds = 1.0;
        runtime.state.set_editor_timeline_time_seconds(1.0);

        runtime.app.world_mut().write_message(ShiftTimeline(1.5));
        let _ = runtime.run_app_frame(egui::RawInput::default());

        approx::assert_abs_diff_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimelineRes>()
                .0
                .clock
                .time_seconds,
            2.5,
            epsilon = 1e-6
        );
        approx::assert_abs_diff_eq!(
            runtime.state.editor_timeline_time_seconds(),
            2.5,
            epsilon = 1e-6
        );
    }

    #[tokio::test]
    async fn runtime_carve_out_set_timeline_time_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .resource_mut::<EditorTimelineRes>()
            .0
            .clock
            .duration_seconds = 3.0;
        runtime.state.set_editor_timeline_duration_seconds(3.0);

        runtime.app.world_mut().write_message(SetTimelineTime(1.25));
        let _ = runtime.run_app_frame(egui::RawInput::default());

        approx::assert_abs_diff_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimelineRes>()
                .0
                .clock
                .time_seconds,
            1.25,
            epsilon = 1e-6
        );
        approx::assert_abs_diff_eq!(
            runtime.state.editor_timeline_time_seconds(),
            1.25,
            epsilon = 1e-6
        );
    }

    #[tokio::test]
    async fn runtime_carve_out_set_timeline_duration_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .resource_mut::<EditorTimelineRes>()
            .0
            .clock
            .duration_seconds = 8.0;
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorTimelineRes>()
            .0
            .clock
            .time_seconds = 6.0;
        runtime.state.set_editor_timeline_duration_seconds(8.0);
        runtime.state.set_editor_timeline_time_seconds(6.0);

        runtime
            .app
            .world_mut()
            .write_message(SetTimelineDuration(2.0));
        let _ = runtime.run_app_frame(egui::RawInput::default());

        approx::assert_abs_diff_eq!(
            runtime.state.editor_timeline_duration_seconds(),
            2.0,
            epsilon = 1e-6
        );
        approx::assert_abs_diff_eq!(
            runtime.state.editor_timeline_time_seconds(),
            2.0,
            epsilon = 1e-6
        );
    }

    #[tokio::test]
    async fn runtime_carve_out_set_selected_tap_propagates_to_legacy_state() {
        let mut state = State::new_test().await;
        state.enter_editor_phase_for_test("TimelineSelection");
        state.set_editor_mode(eg_types::EditorMode::Tapping);
        state.set_editor_tap_state_for_test(
            vec![0.25, 1.0],
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 1.0]],
            None,
        );
        let mut runtime = Runtime::new(state);
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .mode = eg_types::EditorMode::Tapping;

        runtime
            .app
            .world_mut()
            .write_message(SetSelectedTap(Some(1)));
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimelineRes>()
                .0
                .taps
                .selected_index,
            Some(1)
        );
        assert_eq!(runtime.state.editor_selected_tap_index_value(), Some(1));
    }

    #[tokio::test]
    async fn runtime_carve_out_set_selected_tap_time_propagates_to_legacy_state() {
        let mut state = State::new_test().await;
        state.enter_editor_phase_for_test("TimelineTapTime");
        state.set_editor_mode(eg_types::EditorMode::Tapping);
        state.set_editor_timeline_duration_seconds(2.0);
        state.set_editor_timeline_time_seconds(1.0);
        state.set_editor_tap_state_for_test(
            vec![0.25, 1.0, 1.5],
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 1.0], [2.0, 0.0, 2.0]],
            Some(1),
        );
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .write_message(SetSelectedTapTime(1.75));
        let _ = runtime.run_app_frame(egui::RawInput::default());

        let timeline = &runtime.app.world().resource::<EditorTimelineRes>().0;
        assert_eq!(timeline.taps.tap_times, vec![0.25, 1.5, 1.75]);
        assert_eq!(
            timeline.taps.tap_indicator_positions,
            runtime.state.editor_tap_indicator_positions(),
            "runtime sync must copy legacy's final derived tap indicators back into the App world"
        );
        assert_eq!(timeline.taps.selected_index, Some(2));
        approx::assert_abs_diff_eq!(timeline.clock.time_seconds, 1.75, epsilon = 1e-6);
        assert_eq!(
            runtime.app.world().resource::<EditorInputRes>().0.cursor,
            runtime.state.editor_cursor_position(),
            "runtime sync must copy legacy's final selected-tap cursor back into the App world"
        );

        assert_eq!(runtime.state.editor_tap_times(), &[0.25, 1.5, 1.75]);
        assert_eq!(runtime.state.editor_selected_tap_index_value(), Some(2));
        approx::assert_abs_diff_eq!(
            runtime.state.editor_timeline_time_seconds(),
            1.75,
            epsilon = 1e-6
        );
        assert_eq!(
            runtime.state.editor_cursor_position(),
            runtime.state.editor_tap_indicator_positions()[2]
        );

        let pending = runtime
            .app
            .world()
            .resource::<eg_state::events::PendingCommands>();
        assert!(
            pending.app_commands.is_empty() && pending.editor_commands.is_empty(),
            "SetSelectedTapTime must not stage a legacy command"
        );
    }

    #[tokio::test]
    async fn runtime_carve_out_toggle_timeline_playback_propagates_to_legacy_state() {
        let mut state = State::new_test().await;
        state.enter_editor_phase_for_test("TimelinePlayback");
        state.set_editor_mode(eg_types::EditorMode::Timing);
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .write_message(ToggleTimelinePlayback);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimelineRes>()
                .0
                .playback
                .playing,
            runtime.state.editor_is_playing(),
            "post-frame mirror must keep App playback state converged after legacy update"
        );
        assert_eq!(
            runtime.app.world().resource::<EditorInputRes>().0.mode,
            runtime.state.editor_mode()
        );
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorRuntimeRes>()
                .0
                .interaction
                .last_mode,
            runtime.state.editor_playback_last_mode()
        );

        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimelineRes>()
                .0
                .playback
                .playing,
            runtime.state.editor_is_playing(),
            "a no-message frame must not resurrect stale App playback state"
        );
        assert_eq!(
            runtime.app.world().resource::<EditorInputRes>().0.mode,
            runtime.state.editor_mode()
        );
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorRuntimeRes>()
                .0
                .interaction
                .last_mode,
            runtime.state.editor_playback_last_mode()
        );

        let pending = runtime
            .app
            .world()
            .resource::<eg_state::events::PendingCommands>();
        assert!(
            pending.app_commands.is_empty() && pending.editor_commands.is_empty(),
            "ToggleTimelinePlayback must not stage a legacy command"
        );
    }

    #[tokio::test]
    async fn runtime_carve_out_add_tap_propagates_to_legacy_state() {
        let mut state = State::new_test().await;
        state.enter_editor_phase_for_test("TimelineAddTap");
        state.set_editor_mode(eg_types::EditorMode::Tapping);
        state.set_editor_timeline_duration_seconds(2.0);
        state.set_editor_timeline_time_seconds(1.0);
        state.set_editor_tap_state_for_test(
            vec![0.25, 1.5],
            vec![[0.0, 0.0, 0.0], [2.0, 0.0, 2.0]],
            None,
        );
        let mut runtime = Runtime::new(state);

        runtime.app.world_mut().write_message(AddTap);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        let timeline = &runtime.app.world().resource::<EditorTimelineRes>().0;
        assert_eq!(timeline.taps.tap_times, vec![0.25, 1.0, 1.5]);
        assert_eq!(timeline.taps.selected_index, Some(1));
        assert_eq!(runtime.state.editor_tap_times(), &[0.25, 1.0, 1.5]);
        assert_eq!(runtime.state.editor_selected_tap_index_value(), Some(1));
        assert_eq!(
            timeline.taps.tap_indicator_positions,
            runtime.state.editor_tap_indicator_positions(),
            "runtime sync must copy legacy's final derived tap indicators back into the App world"
        );

        let pending = runtime
            .app
            .world()
            .resource::<eg_state::events::PendingCommands>();
        assert!(
            pending.app_commands.is_empty() && pending.editor_commands.is_empty(),
            "AddTap must not stage a legacy command"
        );
    }

    #[tokio::test]
    async fn runtime_carve_out_remove_tap_messages_propagate_to_legacy_state() {
        let mut state = State::new_test().await;
        state.enter_editor_phase_for_test("TimelineRemoveTap");
        state.set_editor_mode(eg_types::EditorMode::Tapping);
        state.set_editor_timeline_duration_seconds(2.0);
        state.set_editor_timeline_time_seconds(1.0);
        state.set_editor_tap_state_for_test(
            vec![0.25, 1.0, 1.5],
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 1.0], [2.0, 0.0, 2.0]],
            Some(2),
        );
        let mut runtime = Runtime::new(state);

        runtime.app.world_mut().write_message(RemoveTap);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        let timeline = &runtime.app.world().resource::<EditorTimelineRes>().0;
        assert_eq!(timeline.taps.tap_times, vec![0.25, 1.5]);
        assert_eq!(timeline.taps.selected_index, Some(1));
        assert_eq!(runtime.state.editor_tap_times(), &[0.25, 1.5]);
        assert_eq!(runtime.state.editor_selected_tap_index_value(), Some(1));
        assert_eq!(
            timeline.taps.tap_indicator_positions,
            runtime.state.editor_tap_indicator_positions(),
            "runtime sync must copy legacy's final derived tap indicators back into the App world"
        );

        runtime.app.world_mut().write_message(RemoveTapAt(1.5));
        let _ = runtime.run_app_frame(egui::RawInput::default());

        let timeline = &runtime.app.world().resource::<EditorTimelineRes>().0;
        assert_eq!(timeline.taps.tap_times, vec![0.25]);
        assert_eq!(timeline.taps.selected_index, None);
        assert_eq!(runtime.state.editor_tap_times(), &[0.25]);
        assert_eq!(runtime.state.editor_selected_tap_index_value(), None);
        assert_eq!(
            timeline.taps.tap_indicator_positions,
            runtime.state.editor_tap_indicator_positions()
        );

        let pending = runtime
            .app
            .world()
            .resource::<eg_state::events::PendingCommands>();
        assert!(
            pending.app_commands.is_empty() && pending.editor_commands.is_empty(),
            "RemoveTap/RemoveTapAt must not stage legacy commands"
        );
    }

    #[tokio::test]
    async fn runtime_carve_out_clear_taps_propagates_to_legacy_state() {
        let mut state = State::new_test().await;
        state.enter_editor_phase_for_test("TimelineClearTaps");
        state.set_editor_mode(eg_types::EditorMode::Tapping);
        state.set_editor_tap_state_for_test(
            vec![0.25, 1.0],
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 1.0]],
            Some(1),
        );
        let mut runtime = Runtime::new(state);

        runtime.app.world_mut().write_message(ClearTaps);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        let timeline = &runtime.app.world().resource::<EditorTimelineRes>().0;
        assert!(timeline.taps.tap_times.is_empty());
        assert!(timeline.taps.tap_indicator_positions.is_empty());
        assert_eq!(timeline.taps.selected_index, None);
        assert!(runtime.state.editor_tap_times().is_empty());
        assert!(runtime.state.editor_tap_indicator_positions().is_empty());
        assert_eq!(runtime.state.editor_selected_tap_index_value(), None);

        let pending = runtime
            .app
            .world()
            .resource::<eg_state::events::PendingCommands>();
        assert!(
            pending.app_commands.is_empty() && pending.editor_commands.is_empty(),
            "ClearTaps must not stage a legacy command"
        );
    }

    #[tokio::test]
    async fn runtime_carve_out_set_playback_speed_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        runtime.app.world_mut().write_message(SetPlaybackSpeed(1.5));
        let _ = runtime.run_app_frame(egui::RawInput::default());

        approx::assert_abs_diff_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimingRes>()
                .0
                .playback_speed,
            1.5,
            epsilon = 1e-6
        );
        approx::assert_abs_diff_eq!(runtime.state.editor_playback_speed(), 1.5, epsilon = 1e-6);
    }

    #[tokio::test]
    async fn runtime_carve_out_set_waveform_zoom_and_scroll_propagate_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        runtime.app.world_mut().write_message(SetWaveformZoom(3.5));
        runtime
            .app
            .world_mut()
            .write_message(SetWaveformScroll(1.25));
        let _ = runtime.run_app_frame(egui::RawInput::default());

        approx::assert_abs_diff_eq!(runtime.state.editor_waveform_zoom(), 3.5, epsilon = 1e-6);
        approx::assert_abs_diff_eq!(runtime.state.editor_waveform_scroll(), 1.25, epsilon = 1e-6);
    }

    #[tokio::test]
    async fn runtime_carve_out_no_op_frame_does_not_resync_timeline_settings() {
        let mut state = State::new_test().await;
        state.set_editor_timeline_duration_seconds(5.0);
        state.set_editor_timeline_time_seconds(1.0);
        state.set_editor_playback_speed(1.5);
        state.set_editor_waveform_zoom(2.5);
        state.set_editor_waveform_scroll(0.75);
        let mut runtime = Runtime::new(state);

        let _ = runtime.run_app_frame(egui::RawInput::default());

        approx::assert_abs_diff_eq!(
            runtime.state.editor_timeline_duration_seconds(),
            5.0,
            epsilon = 1e-6
        );
        approx::assert_abs_diff_eq!(
            runtime.state.editor_timeline_time_seconds(),
            1.0,
            epsilon = 1e-6
        );
        approx::assert_abs_diff_eq!(runtime.state.editor_playback_speed(), 1.5, epsilon = 1e-6);
        approx::assert_abs_diff_eq!(runtime.state.editor_waveform_zoom(), 2.5, epsilon = 1e-6);
        approx::assert_abs_diff_eq!(runtime.state.editor_waveform_scroll(), 0.75, epsilon = 1e-6);
    }

    #[tokio::test]
    async fn runtime_seeds_editor_timeline_res_from_legacy_state_no_first_frame_clobber() {
        let mut state = State::new_test().await;
        state.enter_editor_phase_for_test("TimelineSeed");
        state.set_editor_mode(eg_types::EditorMode::Tapping);
        state.set_editor_timeline_duration_seconds(6.0);
        state.set_editor_timeline_time_seconds(2.5);
        state.set_editor_tap_state_for_test(
            vec![0.5, 1.5],
            vec![[0.0, 0.0, 0.0], [1.0, 0.0, 1.0]],
            Some(1),
        );
        let mut runtime = Runtime::new(state);
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .mode = eg_types::EditorMode::Tapping;

        approx::assert_abs_diff_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimelineRes>()
                .0
                .clock
                .duration_seconds,
            6.0,
            epsilon = 1e-6
        );
        approx::assert_abs_diff_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimelineRes>()
                .0
                .clock
                .time_seconds,
            2.5,
            epsilon = 1e-6
        );
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimelineRes>()
                .0
                .taps
                .tap_times,
            vec![0.5, 1.5]
        );
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTimelineRes>()
                .0
                .taps
                .selected_index,
            Some(1)
        );

        let _ = runtime.run_app_frame(egui::RawInput::default());

        approx::assert_abs_diff_eq!(
            runtime.state.editor_timeline_duration_seconds(),
            6.0,
            epsilon = 1e-6
        );
        approx::assert_abs_diff_eq!(
            runtime.state.editor_timeline_time_seconds(),
            2.5,
            epsilon = 1e-6
        );
        assert_eq!(runtime.state.editor_selected_tap_index_value(), Some(1));
    }

    // ---- Phase 3.10 spawn carve-out tests ----
    //
    // All 3 `events::spawn::*` messages are carve-outs that mutate
    // `EditorSpawnRes.0` (SetSpawnHere, RotateSpawnDirection) or
    // `EditorConfigRes.0.selected_block_rotation_degrees[1]`
    // (RotatePlacementPreview) directly. The runtime tests below
    // pin the end-to-end path: message → App-world resource → legacy
    // State field.

    use eg_state::events::spawn::{RotatePlacementPreview, RotateSpawnDirection, SetSpawnHere};
    use eg_state::state::EditorSpawnRes;
    use eg_types::Direction;

    /// `SetSpawnHere` end-to-end: writing the message reaches the App
    /// world, mutates `EditorSpawnRes.0.position` to the current
    /// editor cursor (read from `EditorInputRes.0.cursor`), and the
    /// per-frame sync propagates the value to the legacy
    /// `State.editor.spawn.position`.
    #[tokio::test]
    async fn runtime_carve_out_set_spawn_here_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        // Set a non-default cursor and spawn position on both sides
        // so the message has something to flow through.
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .cursor = [3.0, 6.0, 9.0];
        let initial = eg_types::SpawnMetadata {
            position: [0.0, 0.0, 0.0],
            direction: Direction::Forward,
        };
        runtime
            .app
            .world_mut()
            .insert_resource(EditorSpawnRes(initial.clone()));
        runtime.state.set_editor_spawn_state(initial.clone());

        runtime.app.world_mut().write_message(SetSpawnHere);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        // Resource holds the new spawn position (the cursor).
        assert_eq!(
            runtime.app.world().resource::<EditorSpawnRes>().0.position,
            [3.0, 6.0, 9.0]
        );
        // Legacy state matches the resource after the sync.
        assert_eq!(runtime.state.editor_spawn_position(), [3.0, 6.0, 9.0]);
    }

    /// `RotateSpawnDirection` end-to-end: the direction update
    /// propagates to the legacy state.
    #[tokio::test]
    async fn runtime_carve_out_rotate_spawn_direction_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        // Pin the initial state on both sides.
        let initial = eg_types::SpawnMetadata {
            position: [0.0, 0.0, 0.0],
            direction: Direction::Forward,
        };
        runtime
            .app
            .world_mut()
            .insert_resource(EditorSpawnRes(initial.clone()));
        runtime.state.set_editor_spawn_state(initial.clone());

        runtime.app.world_mut().write_message(RotateSpawnDirection);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime.app.world().resource::<EditorSpawnRes>().0.direction,
            Direction::Right
        );
        assert_eq!(runtime.state.editor_spawn_direction(), Direction::Right);
    }

    /// `RotatePlacementPreview` end-to-end: the block-placement
    /// preview yaw update flows through `EditorConfigRes.0` and the
    /// per-frame `sync_app_editor_config_state` propagates it to the
    /// legacy `State.editor.config.selected_block_rotation_degrees`.
    #[tokio::test]
    async fn runtime_carve_out_rotate_placement_preview_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        // Initial yaw 0.0 on both sides.
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorConfigRes>()
            .0
            .selected_block_rotation_degrees = [0.0, 0.0, 0.0];
        // The seeded resource matches the legacy state default
        // (zeroed rotation), so the first frame is a no-op.
        runtime
            .app
            .world_mut()
            .write_message(RotatePlacementPreview);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        // Resource holds the new rotation.
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorConfigRes>()
                .0
                .selected_block_rotation_degrees,
            [0.0, 90.0, 0.0]
        );
        // Legacy state matches the resource after the sync.
        assert_eq!(
            runtime.state.editor_selected_block_rotation_degrees(),
            [0.0, 90.0, 0.0]
        );
    }

    /// No-op frame for spawn: when no spawn message is written and
    /// the resource matches the legacy state (the seeded invariant),
    /// the sync must not re-invoke `set_editor_spawn_state`.
    #[tokio::test]
    async fn runtime_carve_out_no_op_frame_does_not_resync_spawn() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        let user_spawn = eg_types::SpawnMetadata {
            position: [2.0, 4.0, 6.0],
            direction: Direction::Right,
        };
        runtime
            .app
            .world_mut()
            .insert_resource(EditorSpawnRes(user_spawn.clone()));
        runtime.state.set_editor_spawn_state(user_spawn.clone());

        let _ = runtime.run_app_frame(egui::RawInput::default());

        // Both sides still match.
        assert_eq!(runtime.state.editor_spawn_position(), [2.0, 4.0, 6.0]);
        assert_eq!(runtime.state.editor_spawn_direction(), Direction::Right);
    }

    /// Regression: `EditorSpawnRes` is seeded from the legacy
    /// `State.editor.spawn` at `Runtime::new`. A non-default spawn
    /// on the legacy state at construction time must survive the
    /// first frame even when no spawn message is written.
    #[tokio::test]
    async fn runtime_seeds_editor_spawn_res_from_legacy_state_no_first_frame_clobber() {
        let mut state = State::new_test().await;
        let user_spawn = eg_types::SpawnMetadata {
            position: [5.0, 10.0, 15.0],
            direction: Direction::Right,
        };
        state.set_editor_spawn_state(user_spawn.clone());
        let mut runtime = Runtime::new(state);

        // Seeding: the resource reflects the legacy user value.
        assert_eq!(
            runtime.app.world().resource::<EditorSpawnRes>().0.position,
            [5.0, 10.0, 15.0]
        );
        assert_eq!(
            runtime.app.world().resource::<EditorSpawnRes>().0.direction,
            Direction::Right
        );

        // No-op frame: the change-guarded sync sees no diff and
        // does not clobber the user value.
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(runtime.state.editor_spawn_position(), [5.0, 10.0, 15.0]);
        assert_eq!(runtime.state.editor_spawn_direction(), Direction::Right);
    }

    // ---- Phase 3.11 camera + trigger-selected carve-out tests ----
    //
    // 3 of the 5 `events::camera::*` messages are carve-outs that
    // mutate `EditorCameraRes.0` (AdjustZoom, SetCameraOrientation)
    // or `EditorTriggerRes.0.selected_index` (SetTriggerSelected)
    // directly. The runtime tests below pin the end-to-end path:
    // message → App-world resource → legacy State field. The
    // `AddCameraTrigger` and `AddCameraFollowTrigger` messages
    // keep the legacy `PendingCommands` path because they push to
    // `editor.objects` (out of scope for the Phase 3.11 carve-out).

    use eg_state::events::camera::{AdjustZoom, SetCameraOrientation, SetTriggerSelected};

    /// `AdjustZoom` end-to-end: the zoom delta flows through the
    /// resource, the per-frame sync propagates the new pan to the
    /// legacy `State.editor.camera`.
    #[tokio::test]
    async fn runtime_carve_out_adjust_zoom_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        // Pin the seeded initial pan + target_z on both sides.
        let initial_pan = runtime.state.editor_camera_pan();
        let initial_z = runtime.state.editor_camera_target_z();

        runtime.app.world_mut().write_message(AdjustZoom(1.0));
        let _ = runtime.run_app_frame(egui::RawInput::default());

        // Resource reflects the new pan (some component should have
        // moved).
        let camera = &runtime.app.world().resource::<EditorCameraRes>().0;
        assert!(
            camera.editor_pan != initial_pan || camera.editor_target_z != initial_z,
            "AdjustZoom handler must move the editor camera"
        );
        // Legacy state matches the resource after the sync.
        assert_eq!(runtime.state.editor_camera_pan(), camera.editor_pan);
        assert_eq!(
            runtime.state.editor_camera_target_z(),
            camera.editor_target_z
        );
    }

    /// `SetCameraOrientation` (no transition) end-to-end: the
    /// yaw/pitch update flows through the resource, the per-frame
    /// sync propagates it to the legacy state.
    #[tokio::test]
    async fn runtime_carve_out_set_camera_orientation_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        let new_yaw = 1.5_f32;
        let new_pitch = 0.4_f32;
        runtime.app.world_mut().write_message(SetCameraOrientation {
            rotation: new_yaw,
            pitch: new_pitch,
            transition_seconds: None,
        });
        let _ = runtime.run_app_frame(egui::RawInput::default());

        approx::assert_abs_diff_eq!(
            runtime
                .app
                .world()
                .resource::<EditorCameraRes>()
                .0
                .editor_yaw,
            new_yaw,
            epsilon = 1e-6
        );
        approx::assert_abs_diff_eq!(
            runtime
                .app
                .world()
                .resource::<EditorCameraRes>()
                .0
                .editor_pitch,
            new_pitch,
            epsilon = 1e-6
        );
        approx::assert_abs_diff_eq!(runtime.state.editor_camera_yaw(), new_yaw, epsilon = 1e-6);
        approx::assert_abs_diff_eq!(
            runtime.state.editor_camera_pitch(),
            new_pitch,
            epsilon = 1e-6
        );
    }

    /// `SetTriggerSelected` end-to-end: the index update flows
    /// through the resource, the per-frame sync propagates it.
    #[tokio::test]
    async fn runtime_carve_out_set_trigger_selected_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .write_message(SetTriggerSelected(Some(1)));
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTriggerRes>()
                .0
                .selected_index,
            Some(1)
        );
        assert_eq!(runtime.state.editor_selected_trigger_index_value(), Some(1));
    }

    /// No-op frame: when no camera message is written and the
    /// resource matches the legacy state (the seeded invariant),
    /// the sync must not re-invoke the legacy setters.
    #[tokio::test]
    async fn runtime_carve_out_no_op_frame_does_not_resync_camera() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        // Capture the seeded initial state.
        let initial_pan = runtime.state.editor_camera_pan();
        let initial_yaw = runtime.state.editor_camera_yaw();
        let initial_pitch = runtime.state.editor_camera_pitch();

        let _ = runtime.run_app_frame(egui::RawInput::default());

        // Post-condition: camera unchanged.
        assert_eq!(runtime.state.editor_camera_pan(), initial_pan);
        approx::assert_abs_diff_eq!(
            runtime.state.editor_camera_yaw(),
            initial_yaw,
            epsilon = 1e-6
        );
        approx::assert_abs_diff_eq!(
            runtime.state.editor_camera_pitch(),
            initial_pitch,
            epsilon = 1e-6
        );
    }

    /// Regression: `EditorCameraRes` and `EditorTriggerRes` are
    /// seeded from the legacy `State.editor.camera` and
    /// `State.editor.triggers` at `Runtime::new`. Non-default
    /// values on the legacy state at construction time must
    /// survive the first frame.
    #[tokio::test]
    async fn runtime_seeds_editor_camera_and_trigger_res_no_first_frame_clobber() {
        use eg_state::state::EditorCameraState;
        let mut state = State::new_test().await;
        // Modify the camera to a non-default pose.
        state.set_editor_camera_state(EditorCameraState {
            editor_pan: [10.0, 20.0],
            editor_target_z: 30.0,
            editor_yaw: 1.0,
            editor_pitch: 0.4,
            playing_rotation: 0.0,
            playing_pitch: 0.0,
            transition: None,
        });
        // And the trigger selection.
        state.set_editor_trigger_selected_index(Some(2));
        let mut runtime = Runtime::new(state);

        // Seeding: the resources reflect the legacy user values.
        let camera = &runtime.app.world().resource::<EditorCameraRes>().0;
        assert_eq!(camera.editor_pan, [10.0, 20.0]);
        approx::assert_abs_diff_eq!(camera.editor_target_z, 30.0, epsilon = 1e-6);
        approx::assert_abs_diff_eq!(camera.editor_yaw, 1.0, epsilon = 1e-6);
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<EditorTriggerRes>()
                .0
                .selected_index,
            Some(2)
        );

        // No-op frame: the change-guarded syncs see no diff and
        // do not clobber the user values.
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(runtime.state.editor_camera_pan(), [10.0, 20.0]);
        assert_eq!(runtime.state.editor_selected_trigger_index_value(), Some(2));
    }

    /// `AddCameraTrigger` and `AddCameraFollowTrigger` are NOT
    /// carve-outs: they keep the legacy `PendingCommands` path
    /// because the legacy setters push to `editor.objects` (a
    /// complex store with history + sync side effects). The
    /// schedule-level test `add_camera_trigger_still_uses_pending_commands`
    /// in `events::tests` pins the dispatch path. The
    /// `editor_dispatch` integration tests in
    /// `state::editor_dispatch` cover the full legacy path. This
    /// carve-out test set focuses on the 3 messages that DO
    /// carve out (AdjustZoom, SetCameraOrientation,
    /// SetTriggerSelected); the legacy-path messages are
    /// covered elsewhere.
    #[tokio::test]
    async fn runtime_add_camera_trigger_not_a_carve_out_note() {
        // This is a placeholder test that documents why we
        // intentionally did not add a runtime-level test for
        // AddCameraTrigger/AddCameraFollowTrigger. The legacy
        // path is covered by `events::tests::add_camera_trigger_still_uses_pending_commands`
        // and the existing `editor_dispatch` integration tests.
        // The placeholder pins the carve-out decision so a
        // future refactor that flips the message to a carve-out
        // knows to also move the test.
    }

    // --- Phase 3.12 / Misc carve-out tests ---
    //
    // The `misc` event handlers for `ToggleHitboxVisualization`,
    // `TogglePerfOverlay`, `TogglePlaceWindow`, and `Escape` write
    // directly to App-world resources. The end-to-end tests below
    // exercise the full message → App-world resource → legacy
    // State field propagation.

    use eg_state::events::misc::{ToggleHitboxVisualization, TogglePlaceWindow};

    /// Writing a `ToggleHitboxVisualization` message reaches the
    /// App world, mutates `EditorInputRes.0.show_hitbox_visualization`,
    /// and the per-frame sync propagates the value to the legacy
    /// `State.editor.input.show_hitbox_visualization` (which
    /// marks `EditorDirtyFlags::hitbox_visualization_changed()`).
    #[tokio::test]
    async fn runtime_carve_out_toggle_hitbox_visualization_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        // Reset both sides to a known value.
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .show_hitbox_visualization = false;
        runtime.state.set_editor_hitbox_visualization(false);

        runtime
            .app
            .world_mut()
            .write_message(ToggleHitboxVisualization);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(
            runtime
                .app
                .world()
                .resource::<EditorInputRes>()
                .0
                .show_hitbox_visualization,
            "ToggleHitboxVisualization handler must mutate EditorInputRes"
        );
        assert!(
            runtime.state.editor_hitbox_visualization(),
            "sync_app_editor_input_state must propagate show_hitbox_visualization to legacy state"
        );
    }

    /// No-op frame: when the resource and the legacy state are
    /// unchanged, the sync must not re-invoke the legacy setter
    /// (which marks `EditorDirtyFlags::hitbox_visualization_changed()`).
    #[tokio::test]
    async fn runtime_carve_out_no_op_frame_does_not_resync_hitbox_visualization() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .show_hitbox_visualization = true;
        runtime.state.set_editor_hitbox_visualization(true);

        let _ = runtime.run_app_frame(egui::RawInput::default());

        // No message -> handler does not run -> resource unchanged.
        // Sync sees no diff -> setter not invoked.
        assert!(runtime.state.editor_hitbox_visualization());
        assert!(
            runtime
                .app
                .world()
                .resource::<EditorInputRes>()
                .0
                .show_hitbox_visualization
        );
    }

    // --- Phase 3.15 / Navigation carve-out tests ---
    //
    // The `GameSetPracticeMode` handler in
    // `events::navigation` writes directly to
    // `SessionStateRes.practice_mode_enabled`. The change-guarded
    // sync in `Runtime::sync_app_session_state` invokes the
    // legacy `set_practice_mode_enabled` setter (which has
    // heavy side effects: clear practice checkpoints, rebuild
    // checkpoint vertices, restart level, resume game) only on
    // real user-driven changes. The legacy setter is phase-
    // guarded (no-op outside `AppPhase::Playing`), so the
    // tests below drive the legacy state into the Playing
    // phase before each write.

    use eg_state::events::navigation::{GameResume, GameSetPracticeMode};

    #[tokio::test]
    async fn runtime_carve_out_game_resume_propagates_to_legacy_state() {
        let mut state = State::new_test().await;
        state.set_phase_for_test(eg_types::AppPhase::Playing);
        state.set_playtesting_editor_for_test(false);
        state.set_game_paused_for_test(true);
        let mut runtime = Runtime::new(state);

        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .game_paused,
            "SessionStateRes.game_paused must be seeded from legacy State"
        );

        runtime.app.world_mut().write_message(GameResume);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(
            !runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .game_paused,
            "GameResume handler must clear SessionStateRes.game_paused"
        );
        assert!(
            !runtime.state.game_paused_value(),
            "sync_app_session_state must resume the legacy State"
        );
    }

    #[tokio::test]
    async fn runtime_carve_out_next_level_in_menu_propagates_to_legacy_state() {
        let mut state = State::new_test().await;
        state.set_phase_for_test(eg_types::AppPhase::Menu);
        state
            .set_menu_levels_for_test(vec!["RuntimeMenuA".to_string(), "RuntimeMenuB".to_string()]);
        state.set_menu_selected_level_index(0);
        let mut runtime = Runtime::new(state);

        eg_ui::enqueue_ui_command(
            runtime.app.world_mut(),
            eg_state::commands::AppCommand::NextLevel,
        );
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime
                .app
                .world()
                .resource::<MenuStateRes>()
                .0
                .selected_level,
            1,
            "NextLevel in Menu must mutate MenuStateRes.selected_level"
        );
        assert_eq!(
            runtime.state.menu_selected_level_index(),
            1,
            "sync_app_menu_state must propagate selected_level to legacy State"
        );

        let pending = runtime
            .app
            .world()
            .resource::<eg_state::events::PendingCommands>();
        assert!(
            pending.app_commands.is_empty() && pending.editor_commands.is_empty(),
            "menu NextLevel must not stage a legacy command"
        );
    }

    #[tokio::test]
    async fn runtime_carve_out_next_level_in_editor_propagates_cursor_to_legacy_state() {
        let mut state = State::new_test().await;
        state.set_phase_for_test(eg_types::AppPhase::Editor);
        state.set_editor_snap_to_grid(true);
        state.set_editor_snap_step(0.25);
        state.set_editor_cursor_position([0.0, 0.0, 0.0]);
        let mut runtime = Runtime::new(state);

        eg_ui::enqueue_ui_command(
            runtime.app.world_mut(),
            eg_state::commands::AppCommand::NextLevel,
        );
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime.app.world().resource::<EditorInputRes>().0.cursor,
            [0.25, 0.0, 0.0],
            "NextLevel in Editor must mutate EditorInputRes.cursor"
        );
        assert_eq!(
            runtime.state.editor_cursor_position(),
            [0.25, 0.0, 0.0],
            "sync_app_editor_input_state must propagate cursor movement to legacy State"
        );

        let pending = runtime
            .app
            .world()
            .resource::<eg_state::events::PendingCommands>();
        assert!(
            pending.app_commands.is_empty() && pending.editor_commands.is_empty(),
            "editor NextLevel must not stage a legacy command"
        );
    }

    #[tokio::test]
    async fn runtime_carve_out_legacy_menu_selection_survives_next_frame_without_message() {
        let mut state = State::new_test().await;
        state.set_phase_for_test(eg_types::AppPhase::Menu);
        state
            .set_menu_levels_for_test(vec!["RuntimeMenuA".to_string(), "RuntimeMenuB".to_string()]);
        state.set_menu_selected_level_index(1);
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .resource_mut::<MenuStateRes>()
            .0
            .selected_level = 0;

        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime.state.menu_selected_level_index(),
            1,
            "legacy menu selection must not be clobbered without a navigation message"
        );
        assert_eq!(
            runtime
                .app
                .world()
                .resource::<MenuStateRes>()
                .0
                .selected_level,
            1,
            "prepare_app_world_for_state_schedule must mirror legacy menu selection"
        );
    }

    #[tokio::test]
    async fn runtime_carve_out_legacy_pause_survives_next_frame_without_resume_message() {
        let mut state = State::new_test().await;
        state.set_phase_for_test(eg_types::AppPhase::Playing);
        state.set_playtesting_editor_for_test(false);
        state.set_game_paused_for_test(true);
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .resource_mut::<SessionStateRes>()
            .game_paused = false;

        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(
            runtime.state.game_paused_value(),
            "legacy pause must not be undone without a GameResume message"
        );
        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .game_paused,
            "prepare_app_world_for_state_schedule must mirror the legacy pause flag"
        );
    }

    /// Writing a `GameSetPracticeMode(true)` message reaches the
    /// App world, mutates `SessionStateRes.practice_mode_enabled`,
    /// and the per-frame sync propagates the value to the legacy
    /// `State.session.practice_mode_enabled` (which clears
    /// `practice_checkpoints` and, when the toggle flips from
    /// false to true, calls `resume_game`).
    #[tokio::test]
    async fn runtime_carve_out_game_set_practice_mode_propagates_to_legacy_state() {
        let mut state = State::new_test().await;
        // Force the legacy state into `AppPhase::Playing` so the
        // phase-guarded legacy setter is a no-op-free.
        state.set_phase_for_test(eg_types::AppPhase::Playing);
        state.set_playtesting_editor_for_test(false);
        state.set_session_practice_mode_enabled_for_test(false);
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .resource_mut::<SessionStateRes>()
            .practice_mode_enabled = false;

        // Write the message and run a frame so the schedule's
        // handler runs, the per-frame sync runs, and the legacy
        // setter fires.
        runtime
            .app
            .world_mut()
            .write_message(GameSetPracticeMode { enabled: true });
        let _ = runtime.run_app_frame(egui::RawInput::default());

        // App world is the source of truth.
        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .practice_mode_enabled,
            "GameSetPracticeMode(true) handler must mutate SessionStateRes.practice_mode_enabled"
        );
        // Legacy state was synced to match the resource.
        assert!(
            runtime.state.practice_mode_enabled_value(),
            "sync_app_session_state must propagate practice_mode_enabled to legacy state"
        );
        // No legacy command was staged (carve-out).
        let pending = runtime
            .app
            .world()
            .resource::<eg_state::events::PendingCommands>();
        assert!(
            pending.app_commands.is_empty() && pending.editor_commands.is_empty(),
            "GameSetPracticeMode must not stage any legacy command (carve-out)"
        );
    }

    /// No-op frame for practice mode: when no
    /// `GameSetPracticeMode` message is written and the
    /// resource matches the legacy state (the seeded
    /// invariant), the sync must not re-invoke the legacy
    /// `set_practice_mode_enabled` setter (which would
    /// trigger `restart_level` + `resume_game` again).
    #[tokio::test]
    async fn runtime_carve_out_no_op_frame_does_not_resync_practice_mode() {
        let mut state = State::new_test().await;
        // Seed the legacy state to a known value. The
        // resource is seeded from the legacy state at
        // Runtime::new, so the first frame is a true no-op.
        state.set_phase_for_test(eg_types::AppPhase::Playing);
        state.set_playtesting_editor_for_test(false);
        state.set_session_practice_mode_enabled_for_test(true);
        let mut runtime = Runtime::new(state);

        // Run a frame with no message. The handler does not
        // run, the resource stays at the seeded value, the
        // sync sees no diff, the legacy setter is not
        // invoked.
        let _ = runtime.run_app_frame(egui::RawInput::default());

        // Both the resource and the legacy state keep the
        // user value.
        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .practice_mode_enabled
        );
        assert!(runtime.state.practice_mode_enabled_value());
    }

    /// Regression: `SessionStateRes.practice_mode_enabled` must
    /// be seeded from the legacy `State` at `Runtime::new` so
    /// the change-guarded sync does not clobber a user-enabled
    /// practice mode on the first frame. Same first-frame
    /// clobber pattern as the snap / bpm / timing / spawn /
    /// camera / trigger / perf carve-outs.
    #[tokio::test]
    async fn runtime_seeds_practice_mode_from_legacy_state_no_first_frame_clobber() {
        let mut state = State::new_test().await;
        // User-enabled practice mode. The default resource
        // value is `false`; without seeding, the first-frame
        // sync would see a diff and clobber the user value
        // back to `false` (and the legacy setter would call
        // `restart_level` + `resume_game` — both heavy).
        state.set_phase_for_test(eg_types::AppPhase::Playing);
        state.set_playtesting_editor_for_test(false);
        state.set_session_practice_mode_enabled_for_test(true);
        let mut runtime = Runtime::new(state);

        // Seeding: the resource must reflect the legacy user
        // value, not the `SessionStateRes` default.
        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .practice_mode_enabled,
            "SessionStateRes.practice_mode_enabled must be seeded from legacy State at Runtime::new"
        );

        // Run a frame with no GameSetPracticeMode message.
        // The change-guarded sync must see no diff and must
        // NOT invoke the legacy setter, so the user value
        // survives the first frame.
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(
            runtime.state.practice_mode_enabled_value(),
            "first-frame sync must not clobber user practice_mode_enabled setting"
        );
    }

    /// Writing a `TogglePerfOverlay` message reaches the App
    /// world, mutates `PerfRes.0.overlay_enabled`, and the
    /// per-frame sync propagates the value to the legacy
    /// `State.editor.perf.overlay_enabled` (which calls
    /// `puffin::set_scopes_on(...)`).
    #[tokio::test]
    async fn runtime_carve_out_toggle_perf_overlay_propagates_to_legacy_state() {
        use eg_state::events::misc::TogglePerfOverlay;

        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        // Reset both sides to a known value.
        runtime
            .app
            .world_mut()
            .resource_mut::<eg_state::state::PerfRes>()
            .0
            .overlay_enabled = false;
        runtime.state.set_perf_overlay_enabled(false);

        runtime.app.world_mut().write_message(TogglePerfOverlay);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(
            runtime
                .app
                .world()
                .resource::<eg_state::state::PerfRes>()
                .0
                .overlay_enabled,
            "TogglePerfOverlay handler must mutate PerfRes"
        );
        assert!(
            runtime.state.perf_overlay_enabled(),
            "sync_app_perf_state must propagate overlay_enabled to legacy state"
        );

        // Toggle back.
        runtime.app.world_mut().write_message(TogglePerfOverlay);
        let _ = runtime.run_app_frame(egui::RawInput::default());
        assert!(!runtime.state.perf_overlay_enabled());
    }

    /// No-op frame: when the perf overlay resource matches the
    /// legacy state, the sync must not re-invoke the legacy
    /// setter (which has the `puffin::set_scopes_on` global side
    /// effect).
    #[tokio::test]
    async fn runtime_carve_out_no_op_frame_does_not_resync_perf_overlay() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .resource_mut::<eg_state::state::PerfRes>()
            .0
            .overlay_enabled = true;
        runtime.state.set_perf_overlay_enabled(true);

        let _ = runtime.run_app_frame(egui::RawInput::default());

        // No message -> handler does not run -> resource unchanged.
        // Sync sees no diff -> setter not invoked.
        assert!(runtime.state.perf_overlay_enabled());
        assert!(
            runtime
                .app
                .world()
                .resource::<eg_state::state::PerfRes>()
                .0
                .overlay_enabled
        );
    }

    /// Writing a `TogglePlaceWindow` message reaches the App
    /// world, mutates `SessionStateRes.editor_show_place_window`,
    /// and the per-frame sync propagates the value to the legacy
    /// `State.session.editor_show_place_window`.
    #[tokio::test]
    async fn runtime_carve_out_toggle_place_window_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        // Reset both sides to a known value.
        runtime
            .app
            .world_mut()
            .resource_mut::<SessionStateRes>()
            .editor_show_place_window = false;
        runtime.state.set_editor_show_place_window(false);

        runtime.app.world_mut().write_message(TogglePlaceWindow);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_show_place_window,
            "TogglePlaceWindow handler must mutate SessionStateRes"
        );
        assert!(
            runtime.state.editor_show_place_window(),
            "sync_app_session_state must propagate editor_show_place_window to legacy state"
        );
    }

    /// No-op frame: when the place-window field matches the
    /// legacy state, the sync must not re-invoke the legacy
    /// setter.
    #[tokio::test]
    async fn runtime_carve_out_no_op_frame_does_not_resync_place_window() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        runtime
            .app
            .world_mut()
            .resource_mut::<SessionStateRes>()
            .editor_show_place_window = true;
        runtime.state.set_editor_show_place_window(true);

        let _ = runtime.run_app_frame(egui::RawInput::default());

        // No message -> handler does not run -> resource unchanged.
        // Sync sees no diff -> setter not invoked.
        assert!(runtime.state.editor_show_place_window());
        assert!(
            runtime
                .app
                .world()
                .resource::<SessionStateRes>()
                .editor_show_place_window
        );
    }

    /// Regression: `PerfRes` must be seeded from the legacy
    /// `State` at `Runtime::new` so the change-guarded perf sync
    /// does not clobber the user overlay setting on the first
    /// frame. The legacy setter calls `puffin::set_scopes_on`,
    /// so without seeding, the first frame would re-invoke the
    /// side effect with the default value.
    #[tokio::test]
    async fn runtime_seeds_perf_res_from_legacy_state_no_first_frame_clobber() {
        let mut state = State::new_test().await;
        // User-loaded settings: perf overlay enabled. The
        // resource default is `overlay_enabled = false`, so this
        // differs from the default and would be clobbered
        // without the seeding fix.
        state.set_perf_overlay_enabled(true);
        let mut runtime = Runtime::new(state);

        // Seeding: the resource must reflect the legacy user value.
        assert!(
            runtime
                .app
                .world()
                .resource::<eg_state::state::PerfRes>()
                .0
                .overlay_enabled,
            "PerfRes must be seeded from legacy State at Runtime::new"
        );

        // Run a frame with no perf message. The change-guarded
        // sync must see no diff and must NOT invoke the legacy
        // setter, so the user value survives the first frame.
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert!(
            runtime.state.perf_overlay_enabled(),
            "first-frame sync must not clobber user perf overlay setting"
        );
    }

    /// `Escape` is a Phase 3.46 carve-out: the App-world handler
    /// writes `EscapeRequest.pending = true`, the runtime sync
    /// consumes the request and calls the legacy
    /// `State::apply_editor_escape` compatibility method, and no
    /// legacy `EditorCommand::Escape` is staged in `PendingCommands`.
    #[tokio::test]
    async fn runtime_carve_out_escape_consumes_request_without_pending_commands() {
        use eg_state::events::misc::Escape;

        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        runtime.app.world_mut().write_message(Escape);
        let _ = runtime.run_app_frame(egui::RawInput::default());

        let request = runtime.app.world().resource::<EscapeRequest>();
        assert!(
            !request.pending,
            "runtime must consume EscapeRequest after applying it"
        );
        let pending = runtime.app.world().resource::<PendingCommands>();
        assert!(
            pending.app_commands.is_empty() && pending.editor_commands.is_empty(),
            "Escape must not stage a legacy command"
        );
    }

    /// `Escape` is a no-op on a no-message frame. Pin the change
    /// guard: stale requests don't accumulate across frames.
    #[tokio::test]
    async fn runtime_carve_out_escape_no_message_frame_is_noop() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);
        // No message; pre-condition: request is not pending.
        assert!(!runtime.app.world().resource::<EscapeRequest>().pending);
        let _ = runtime.run_app_frame(egui::RawInput::default());
        let request = runtime.app.world().resource::<EscapeRequest>();
        assert!(
            !request.pending,
            "no-message frame must not request an App-world escape"
        );
    }

    // --- Phase 3.13 / Editor-mode carve-out test ---
    //
    // The `SetEditorMode` handler writes `EditorInputRes.0.mode`
    // directly. The per-frame sync invokes `state.set_editor_mode`
    // (with the heavy vertex rebuild + dirty flag side effects)
    // only when the resource value differs from the legacy
    // `State.editor.input.mode`.

    use eg_state::events::editor_mode::SetEditorMode;

    /// Writing a `SetEditorMode` message reaches the App world,
    /// mutates `EditorInputRes.0.mode`, and the per-frame sync
    /// propagates the value to the legacy
    /// `State.editor.input.mode` (via `state.set_editor_mode`,
    /// which rebuilds vertices and marks dirty flags).
    #[tokio::test]
    async fn runtime_carve_out_set_editor_mode_propagates_to_legacy_state() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        // Force a non-default mode on both sides.
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .mode = eg_types::EditorMode::Place;
        runtime.state.set_editor_mode(eg_types::EditorMode::Place);

        runtime
            .app
            .world_mut()
            .write_message(SetEditorMode(eg_types::EditorMode::Tapping));
        let _ = runtime.run_app_frame(egui::RawInput::default());

        assert_eq!(
            runtime.app.world().resource::<EditorInputRes>().0.mode,
            eg_types::EditorMode::Tapping,
            "SetEditorMode handler must mutate EditorInputRes.mode"
        );
        assert_eq!(
            runtime.state.editor_mode(),
            eg_types::EditorMode::Tapping,
            "sync_app_editor_input_state must propagate mode to legacy state"
        );
    }

    /// No-op frame: when the resource mode matches the legacy
    /// state mode, the sync must not re-invoke `set_editor_mode`
    /// (which rebuilds vertices and marks dirty flags).
    #[tokio::test]
    async fn runtime_carve_out_no_op_frame_does_not_resync_editor_mode() {
        let state = State::new_test().await;
        let mut runtime = Runtime::new(state);

        // Seed the resource and the legacy state to the same mode.
        runtime
            .app
            .world_mut()
            .resource_mut::<EditorInputRes>()
            .0
            .mode = eg_types::EditorMode::Select;
        runtime.state.set_editor_mode(eg_types::EditorMode::Select);

        let _ = runtime.run_app_frame(egui::RawInput::default());

        // No message -> handler does not run -> resource unchanged.
        // Sync sees no diff -> setter not invoked.
        assert_eq!(
            runtime.app.world().resource::<EditorInputRes>().0.mode,
            eg_types::EditorMode::Select
        );
        assert_eq!(runtime.state.editor_mode(), eg_types::EditorMode::Select);
    }
}
