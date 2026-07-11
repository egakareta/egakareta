/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Rendering extraction plugin.
//!
//! Owns the render resource boundary + system-set graph. `State`
//! construction inserts the authoritative heavy resources
//! ([`RenderGpuRes`], [`RenderMeshesRes`], [`RenderSurfaceHostRes`]);
//! the per-frame schedule mirrors POD frame/uniform values and emits
//! rebuild events from ECS change detection. Production still presents
//! through `State::render_egui` until Phase 7 moves frame ownership to
//! `bevy_app::App`.
//!
//! REFACTOR POINT(full-bevy): this plugin + system set graph is the
//! placeholder for `bevy_render`'s render-world + render graph. The
//! `extract_render_state_system` here becomes a
//! `RenderApp::extract` system in the full-Bevy migration; the
//! `RenderPresentSet` is replaced by `bevy_render`'s `Render` set.

use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::{Schedule, SystemSet};
use bevy_ecs::system::SystemParam;
use eg_types::{AppPhase, CameraUniform, ColorSpaceUniform, Direction, LevelMetadata, LineUniform};
use glam::Vec3;

use super::resources::{
    RenderFrameInputRes, RenderGpuRes, RenderMeshesRes, RenderResizeRequest, RenderSurfaceHostRes,
    RenderUniformsRes,
};
use crate::events::mesh_rebuild::{RebuildMeshEvent, RebuildMeshKind};
use crate::state::{
    AppPhaseRes, EditorCameraRes, EditorInputRes, EditorObjectsRes, EditorTimelineRes,
    FrameTimeRes, MenuStateRes, SessionStateRes, evaluate_camera_track_for_objects,
};

/// System sets for the rendering extraction plugin.
///
/// These sets mirror the legacy `render_to_view` /
/// `render_with_overlay` call order so we can slot Bevy systems in
/// without changing the visible behavior.
///
/// Ordering contract (see `docs/bevy-migration.md` Phase 4):
/// ```text
///   RebuildMeshSet      // Changed<> detection -> RebuildMeshEvent
///   RenderExtractSet    // build SceneMeshes + uniform values
///   RenderPresentSet    // read authoritative render resources
/// ```
///
/// In a future full-Bevy frame the order is:
///   `Simulation → RebuildMesh → Extract → Present` (see the
///   per-frame schedule graph in `state/sets.rs`).
#[allow(dead_code)]
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderSet {
    /// Detect `Changed<>` on authored object components and emit a
    /// [`RebuildMeshEvent`].
    RebuildMesh,
    /// Snapshot the world into render resources.
    ///
    /// Today this mirrors POD uniform values from the legacy frame
    /// update into [`RenderUniformsRes`] and [`RenderFrameInputRes`].
    /// Future phases add a `rebuild_mesh_batches_system` that walks object
    /// entities (`Query<(&Position, &Rotation, &Size, &BlockId,
    /// &ColorTint, Option<&PlayState>)>` + player + camera + effects)
    /// and fills [`RenderMeshesRes`] via `eg_mesh` builders.
    Extract,
    /// Issue the wgpu present.
    ///
    /// Reads the authoritative render resources. The compatibility
    /// frame pipeline still performs the actual egui present until
    /// Phase 7 moves frame ownership into Bevy.
    Present,
}

/// The rendering extraction plugin.
///
/// Registers the POD resources + render messages for an `App`.
/// `State` construction inserts real heavy resources after wgpu
/// initialization.
pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RenderUniformsRes>()
            .init_resource::<RenderFrameInputRes>()
            .init_resource::<RenderResizeRequest>()
            .init_non_send::<RenderMeshesRes>()
            .init_non_send::<RenderSurfaceHostRes>()
            .add_message::<RebuildMeshEvent>();
    }
}

/// `World`-flavoured registration for callers that build a `World`
/// directly (the `State` constructor). Mirrors
/// [`RenderPlugin::build`] without going through an `App`.
#[allow(dead_code)]
pub fn register_render_pod_resources(world: &mut World) {
    world.init_resource::<RenderUniformsRes>();
    world.init_resource::<RenderFrameInputRes>();
    world.init_resource::<RenderResizeRequest>();
    // Phase 4c: insert default heavy resources so standalone worlds
    // can query them. `State` construction overwrites these defaults
    // with the authoritative `SceneMeshes` / `SurfaceHost` values.
    world.init_non_send::<RenderMeshesRes>();
    world.init_non_send::<RenderSurfaceHostRes>();
    // Phase 4d: register the RebuildMeshEvent message buffer.
    world.init_resource::<Messages<RebuildMeshEvent>>();
}

/// Resource-backed `RenderSet::Present` boundary.
///
/// Production still calls `State::render_egui` from the frame pipeline
/// until Phase 7 moves frame ownership to `bevy_app::App`, but the
/// Bevy schedule now reads the same authoritative resources that own
/// GPU, mesh, surface, uniform, and frame-input state. This keeps the
/// present set wired to the final resource boundary without acquiring
/// the surface twice in the current compatibility path.
pub fn present_render_frame_system(
    gpu: Option<NonSend<RenderGpuRes>>,
    meshes: NonSend<RenderMeshesRes>,
    surface_host: NonSend<RenderSurfaceHostRes>,
    uniforms: Res<RenderUniformsRes>,
    frame_input: Res<RenderFrameInputRes>,
) {
    let _ = (&gpu, &meshes, &surface_host, &uniforms, &frame_input);
}

/// Build the per-frame render input directly from App-world resources.
///
/// This replaces the legacy `State::render_to_view` frame-input snapshot.
/// The compatibility renderer consumes the resulting POD value until its
/// draw passes are also scheduled in the App world.
#[derive(SystemParam)]
pub(crate) struct RenderFrameInputParams<'w, 's> {
    phase: Res<'w, AppPhaseRes>,
    input: Res<'w, EditorInputRes>,
    session: Res<'w, SessionStateRes>,
    camera: Res<'w, EditorCameraRes>,
    timeline: Res<'w, EditorTimelineRes>,
    clock: Option<Res<'w, eg_game::GameClock>>,
    menu: Option<Res<'w, MenuStateRes>>,
    gpu: Option<NonSend<'w, RenderGpuRes>>,
    player: Query<'w, 's, &'static eg_game::PlayerPosition>,
    frame_input: ResMut<'w, RenderFrameInputRes>,
}

/// Build the per-frame render input from the grouped App-world parameters.
pub(crate) fn update_render_frame_input_system(mut params: RenderFrameInputParams) {
    let current_phase = params.phase.current;
    let editor_is_playing = params.timeline.0.playback.playing;
    let player_position = params
        .player
        .iter()
        .next()
        .map(|position| position.0)
        .unwrap_or([0.0, 0.0, 0.0]);
    let sky_color = match current_phase {
        AppPhase::Menu => params
            .menu
            .as_ref()
            .map(|menu| menu.0.preview_sky_color)
            .unwrap_or(LevelMetadata::default().sky_color),
        AppPhase::Editor => params.session.editor_sky_color,
        AppPhase::Playing => params.session.playing_sky_color,
        _ => LevelMetadata::default().sky_color,
    };
    let grid_center = match current_phase {
        AppPhase::Editor if editor_is_playing => [
            params.timeline.0.preview.position[0],
            params.timeline.0.preview.position[2],
        ],
        AppPhase::Editor => params.camera.0.editor_pan,
        AppPhase::Playing | AppPhase::GameOver => [player_position[0], player_position[2]],
        _ => [0.0, 0.0],
    };

    params.frame_input.0 = eg_render::RenderFrameInput {
        phase: current_phase,
        editor_mode: params.input.0.mode,
        game_over: params.clock.as_ref().is_some_and(|clock| clock.game_over),
        sky_color,
        grid_center,
        apply_gamma_correction: params
            .gpu
            .as_ref()
            .is_some_and(|gpu| gpu.0.apply_gamma_correction),
    };
}

#[derive(SystemParam)]
pub(crate) struct RenderUniformsParams<'w, 's> {
    phase: Res<'w, AppPhaseRes>,
    camera: Res<'w, EditorCameraRes>,
    objects: Res<'w, EditorObjectsRes>,
    timeline: Res<'w, EditorTimelineRes>,
    frame_time: Res<'w, FrameTimeRes>,
    clock: Option<Res<'w, eg_game::GameClock>>,
    menu: Option<Res<'w, MenuStateRes>>,
    gpu: Option<NonSend<'w, RenderGpuRes>>,
    player: Query<
        'w,
        's,
        (
            &'static eg_game::PlayerPosition,
            &'static eg_game::PlayerDirection,
        ),
    >,
    uniforms: ResMut<'w, RenderUniformsRes>,
}

/// Build camera, line, and color-space POD uniforms from App-world state.
///
/// The compatibility `State` receives a copy after extraction so legacy
/// update code can continue to observe the scheduled line uniform while the
/// GPU upload itself is owned by this system.
pub(crate) fn update_render_uniforms_system(mut params: RenderUniformsParams) {
    let Some(gpu) = params.gpu.as_ref() else {
        return;
    };
    let (width, height, apply_gamma_correction) = (
        gpu.0.config.width,
        gpu.0.config.height,
        gpu.0.apply_gamma_correction,
    );
    let (player_position, player_direction) = params
        .player
        .iter()
        .next()
        .map(|(position, direction)| (position.0, direction.0))
        .unwrap_or(([0.0, 0.0, 0.0], Direction::Forward));
    let (elapsed_seconds, started, game_over) = params
        .clock
        .as_ref()
        .map(|clock| (clock.elapsed_seconds, clock.started, clock.game_over))
        .unwrap_or((0.0, false, false));

    let (eye, target) = match params.phase.current {
        AppPhase::Menu => params
            .menu
            .as_ref()
            .map(|menu| {
                (
                    Vec3::from_array(menu.0.preview_camera_position),
                    Vec3::from_array(menu.0.preview_camera_target),
                )
            })
            .unwrap_or((Vec3::ZERO, Vec3::ZERO)),
        AppPhase::Playing | AppPhase::GameOver => {
            let target = Vec3::from_array(player_position);
            let sample = evaluate_camera_track_for_objects(
                &params.camera.0,
                &params.objects.objects,
                elapsed_seconds,
                target,
                started && !game_over,
            );
            (sample.eye, sample.target)
        }
        AppPhase::Editor if params.timeline.0.playback.playing => {
            let target = Vec3::from_array(params.timeline.0.preview.position);
            let sample = evaluate_camera_track_for_objects(
                &params.camera.0,
                &params.objects.objects,
                params.timeline.0.clock.time_seconds,
                target,
                true,
            );
            (sample.eye, sample.target)
        }
        AppPhase::Editor => {
            let target = params.camera.0.editor_camera_target();
            (target + params.camera.0.editor_camera_offset(), target)
        }
    };

    let view = glam::Mat4::look_at_rh(eye, target, Vec3::Y);
    let aspect = width as f32 / height as f32;
    let proj = glam::Mat4::perspective_rh_gl(45f32.to_radians(), aspect, 0.1, 10000.0);
    let mut line = params.uniforms.line;
    if matches!(params.phase.current, AppPhase::Playing | AppPhase::GameOver) {
        line = LineUniform {
            offset: [
                (player_position[0] * 100.0).round() / 100.0,
                (player_position[2] * 100.0).round() / 100.0,
            ],
            rotation: match player_direction {
                Direction::Forward => 0.0,
                Direction::Right => -std::f32::consts::FRAC_PI_2,
            },
            _pad: 0.0,
        };
    }

    params.uniforms.camera = CameraUniform {
        view_proj: (proj * view).to_cols_array_2d(),
        camera_position: [eye.x, eye.y, eye.z, 0.0],
    };
    params.uniforms.line = line;
    params.uniforms.color_space = ColorSpaceUniform {
        apply_gamma_correction: if apply_gamma_correction { 1.0 } else { 0.0 },
        time_seconds: params.frame_time.global_time_seconds,
        viewport_size: [width.max(1) as f32, height.max(1) as f32],
    };
}

/// Upload the extracted camera and line POD values before the compatibility
/// renderer runs. Color-space upload remains in its dedicated system.
pub(crate) fn upload_render_uniforms_system(
    gpu: Option<NonSend<RenderGpuRes>>,
    uniforms: Res<RenderUniformsRes>,
) {
    let Some(gpu) = gpu else {
        return;
    };
    gpu.0.queue.write_buffer(
        &gpu.0.camera_uniform_buffer,
        0,
        bytemuck::bytes_of(&uniforms.camera),
    );
    gpu.0.queue.write_buffer(
        &gpu.0.line_uniform_buffer,
        0,
        bytemuck::bytes_of(&uniforms.line),
    );
}

/// Update the scheduled color-space POD uniform from App-world frame
/// timing. The compatibility `State::update` path consumes this value
/// to upload the GPU buffer until render presentation moves fully into
/// systems.
pub fn update_color_space_uniform_system(
    frame_time: Res<FrameTimeRes>,
    mut uniforms: ResMut<RenderUniformsRes>,
) {
    uniforms.color_space.time_seconds = frame_time.global_time_seconds;
}

/// Upload the scheduled color-space POD value before presentation. This is
/// the first GPU side effect removed from `State::update`; the compatibility
/// shell only records that the scheduled upload already happened so direct
/// legacy callers still retain their fallback behavior.
pub fn upload_color_space_uniform_system(
    gpu: Option<NonSend<RenderGpuRes>>,
    uniforms: Res<RenderUniformsRes>,
) {
    let Some(gpu) = gpu else {
        return;
    };
    gpu.0.queue.write_buffer(
        &gpu.0.color_space_uniform_buffer,
        0,
        bytemuck::bytes_of(&uniforms.color_space),
    );
}

pub fn apply_resize_surface_system(
    mut request: ResMut<RenderResizeRequest>,
    mut gpu: Option<NonSendMut<RenderGpuRes>>,
    surface_host: Option<NonSend<RenderSurfaceHostRes>>,
) {
    let Some(size) = request.take() else {
        return;
    };
    if size.width == 0 || size.height == 0 {
        return;
    }
    request.mark_applied(size);

    if let Some(surface_host) = surface_host
        && let Some(host) = surface_host.0.as_ref()
    {
        host.prepare_resize(size);
    }
    if let Some(gpu) = gpu.as_deref_mut() {
        gpu.0.apply_resize(size);
    }
}

/// Bevy [`QueryFilter`] for the authored object fields that trigger
/// a mesh rebuild on change.
///
/// Factored into a type alias to keep the
/// [`mark_mesh_dirty_system`] signature readable and to stay under
/// the `clippy::type_complexity` threshold without `#[allow]`s.
type AuthoredObjectChangedFilter = (
    With<eg_game::Object>,
    Or<(
        Changed<eg_game::ObjectData>,
        Changed<eg_game::Position>,
        Changed<eg_game::Rotation>,
        Changed<eg_game::Size>,
        Changed<eg_game::BlockId>,
        Changed<eg_game::ColorTint>,
    )>,
);

/// Detect authored object changes and emit mesh rebuild events.
///
/// This bridges ECS change detection to the current coarse rebuild
/// contract. Any authored object field change requests a gameplay
/// object sync and a block mesh rebuild. Later phases move the rebuild
/// consumer itself into a Bevy system that writes [`RenderMeshesRes`]
/// directly.
pub fn mark_mesh_dirty_system(
    changed_objects: Query<(), AuthoredObjectChangedFilter>,
    mut writer: MessageWriter<RebuildMeshEvent>,
) {
    if changed_objects.iter().next().is_none() {
        return;
    }

    writer.write(RebuildMeshEvent::new(RebuildMeshKind::SyncGameObjects));
    writer.write(RebuildMeshEvent::new(RebuildMeshKind::RebuildBlockMesh));
}

/// Add the per-frame mirror system to a [`Schedule`].
///
/// The schedule itself is added by the
/// [`super::super::plugins::register_state_command_pipeline`] helper
/// or by the Phase 7 `App` work; this function only adds the systems
/// that populate the POD render resources.
///
/// Callers are expected to add a `RenderSet::Present` system set
/// afterwards (e.g. `add_command_systems` chains an `add_systems` for
/// the present boundary).
pub fn add_mirror_render_systems(schedule: &mut Schedule) {
    schedule.add_systems((
        mark_mesh_dirty_system.in_set(RenderSet::RebuildMesh),
        // The mirror is a closure-driven system (see the docs on
        // `mirror_render_pod_state_system`). The closure is provided
        // by `State::tick_state` so the system can reach the legacy
        // `State::render` fields. We register a small wrapper that
        // invokes the closure through a thread-local, which the
        // tick_state path populates just before running the schedule.
        //
        // Implementation note: the mirror is a system on its own
        // because Bevy `IntoSystemConfigs` requires `SystemParam`
        // args only. The actual data source is a per-frame
        // `OnceLock<RenderMirrorState>` shared with `tick_state`.
        // See `state/render/mirror.rs` for the bridge.
        mirror_pod_state_via_bridge,
        update_render_frame_input_system
            .after(crate::state::async_bridge::ApplyAsyncResultsSet)
            .before(present_render_frame_system),
        update_color_space_uniform_system
            .after(mirror_pod_state_via_bridge)
            .after(crate::state::frame_time::advance_frame_time_system),
        upload_color_space_uniform_system
            .after(update_color_space_uniform_system)
            .after(apply_resize_surface_system)
            .before(present_render_frame_system)
            .in_set(RenderSet::Present),
        apply_resize_surface_system
            .after(crate::events::auth::handle_resize_surface)
            .before(present_render_frame_system)
            .in_set(RenderSet::Present),
        present_render_frame_system.in_set(RenderSet::Present),
    ));
}

/// Bridge system that reads the per-frame mirror closure from a
/// shared `OnceLock` (populated by `State::tick_state` right before
/// the schedule runs) and dispatches it to
/// [`mirror_render_pod_state_system`].
///
/// The `OnceLock` pattern lets us call the closure in a system
/// without `unsafe` (the closure doesn't escape the system call —
/// `tick_state` overwrites it each frame and waits for the schedule
/// to drain before reading back).
fn mirror_pod_state_via_bridge(mut uniforms: ResMut<RenderUniformsRes>) {
    use crate::state::render::mirror::take_pending_mirror;

    if let Some(mut mirror) = take_pending_mirror() {
        mirror(&mut uniforms);
    }
    // If no mirror was queued, the resources stay at their default
    // values. This is the correct behavior for tests that don't go
    // through the full `State::tick_state` path. The `let _ =` is a
    // deliberate no-op sink that makes the function nominally
    // consume its args so the system stays in the schedule graph.
    let _ = &mut uniforms;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::render::resources::{
        mirror_render_pod_state, render_heavy_resources_present, render_pod_resources_present,
    };
    use crate::state::{AppPhaseRes, EditorCameraRes, EditorInputRes, SessionStateRes};
    use bevy_ecs::world::World;
    use eg_render::RenderFrameInput;
    use eg_types::{AppPhase, EditorMode, LevelMetadata, PhysicalSize};

    #[test]
    fn register_render_pod_resources_inserts_defaults() {
        let mut world = World::new();
        register_render_pod_resources(&mut world);
        assert!(world.contains_resource::<RenderUniformsRes>());
        assert!(world.contains_resource::<RenderFrameInputRes>());
        assert!(world.contains_resource::<RenderResizeRequest>());
    }

    #[test]
    fn render_pod_resources_present_returns_true_after_register() {
        let mut world = World::new();
        register_render_pod_resources(&mut world);
        assert!(render_pod_resources_present(&world));
    }

    #[test]
    fn render_frame_input_system_reads_app_resources() {
        let mut world = World::new();
        world.init_resource::<AppPhaseRes>();
        world.init_resource::<EditorInputRes>();
        world.init_resource::<SessionStateRes>();
        world.init_resource::<EditorCameraRes>();
        world.init_resource::<EditorTimelineRes>();
        world.init_resource::<RenderFrameInputRes>();
        world.resource_mut::<AppPhaseRes>().current = AppPhase::Editor;
        world.resource_mut::<EditorInputRes>().0.mode = EditorMode::Timing;
        world.resource_mut::<EditorCameraRes>().0.editor_pan = [4.0, -2.0];
        world.resource_mut::<SessionStateRes>().editor_sky_color = [0.2, 0.4, 0.6];

        let mut schedule = Schedule::default();
        schedule.add_systems(update_render_frame_input_system);
        schedule.run(&mut world);

        let frame_input = world.resource::<RenderFrameInputRes>().0;
        assert_eq!(frame_input.phase, AppPhase::Editor);
        assert_eq!(frame_input.editor_mode, EditorMode::Timing);
        assert_eq!(frame_input.sky_color, [0.2, 0.4, 0.6]);
        assert_eq!(frame_input.grid_center, [4.0, -2.0]);
    }

    /// `RenderMeshesRes` + `RenderSurfaceHostRes` are inserted (with
    /// default values) by `register_render_pod_resources`. `RenderGpuRes`
    /// is inserted by `State` construction because it has no cheap default.
    #[test]
    fn register_inserts_phase4c_heavy_resources() {
        let mut world = World::new();
        register_render_pod_resources(&mut world);
        assert!(world.contains_non_send::<RenderMeshesRes>());
        assert!(world.contains_non_send::<RenderSurfaceHostRes>());
        assert!(!world.contains_non_send::<RenderGpuRes>());
    }

    /// Bare worlds do not fabricate a GPU resource; `State` construction
    /// inserts the real `RenderGpuRes` once wgpu initialization succeeds.
    #[test]
    fn render_heavy_resources_present_returns_false_for_bare_world() {
        let mut world = World::new();
        register_render_pod_resources(&mut world);
        assert!(!render_heavy_resources_present(&world));
    }

    #[test]
    fn mirror_render_pod_state_writes_via_closure() {
        let mut world = World::new();
        register_render_pod_resources(&mut world);

        world.resource_scope(
            |world, mut uniforms: bevy_ecs::world::Mut<RenderUniformsRes>| {
                let mut frame_input = world.resource_mut::<RenderFrameInputRes>();
                mirror_render_pod_state(&mut uniforms, &mut frame_input, |u, f| {
                    u.color_space.time_seconds = 1.5;
                    f.0 = RenderFrameInput {
                        phase: AppPhase::Playing,
                        editor_mode: EditorMode::Place,
                        game_over: false,
                        sky_color: LevelMetadata::default().sky_color,
                        grid_center: [0.0, 0.0],
                        apply_gamma_correction: false,
                    };
                });
            },
        );

        assert_eq!(
            world
                .resource::<RenderUniformsRes>()
                .color_space
                .time_seconds,
            1.5
        );
        assert_eq!(
            world.resource::<RenderFrameInputRes>().0.phase,
            AppPhase::Playing
        );
    }

    /// Phase 4d: changed authored object components emit rebuild events.
    #[test]
    fn mark_mesh_dirty_system_emits_rebuild_events_for_changed_object() {
        let mut world = World::new();
        register_render_pod_resources(&mut world);

        let object = eg_types::LevelObject::new("core/stone");
        world.spawn(eg_game::ObjectBundle {
            object: eg_game::Object,
            id: eg_game::ObjectId("test-object".to_string()),
            data: eg_game::ObjectData(object.clone()),
            position: eg_game::Position(object.position),
            rotation: eg_game::Rotation(object.rotation),
            size: eg_game::Size(object.size),
            block_id: eg_game::BlockId(object.block_id.clone()),
            color_tint: eg_game::ColorTint(object.color_tint),
            cached_behavior: eg_game::CachedBlockBehavior::from_block_id(&object.block_id),
        });

        let mut schedule = bevy_ecs::schedule::Schedule::default();
        schedule.add_systems(mark_mesh_dirty_system);
        schedule.run(&mut world);

        let kinds = world
            .resource_mut::<Messages<RebuildMeshEvent>>()
            .drain()
            .map(|event| event.kind)
            .collect::<Vec<_>>();

        assert_eq!(
            kinds,
            vec![
                RebuildMeshKind::SyncGameObjects,
                RebuildMeshKind::RebuildBlockMesh,
            ]
        );
    }

    /// Phase 4e: the present system reads the authoritative render
    /// resources. Bare test worlds do not have a real GPU, so the GPU
    /// resource is optional until constructed by `State`.
    #[test]
    fn present_render_frame_system_runs_with_render_resources() {
        let mut world = World::new();
        register_render_pod_resources(&mut world);

        let mut schedule = bevy_ecs::schedule::Schedule::default();
        schedule.add_systems(present_render_frame_system);
        schedule.run(&mut world);
    }

    #[test]
    fn apply_resize_surface_system_consumes_request_without_gpu() {
        let mut world = World::new();
        register_render_pod_resources(&mut world);
        world
            .resource_mut::<RenderResizeRequest>()
            .request(PhysicalSize::new(320, 240));

        let mut schedule = bevy_ecs::schedule::Schedule::default();
        schedule.add_systems(apply_resize_surface_system);
        schedule.run(&mut world);

        assert!(
            !world.resource::<RenderResizeRequest>().pending,
            "resize request must be consumed even in bare render test worlds"
        );
        let applied = world
            .resource_mut::<RenderResizeRequest>()
            .take_applied_size()
            .expect("scheduled resize must publish the accepted size");
        assert_eq!(applied.width, 320);
        assert_eq!(applied.height, 240);
    }
}
