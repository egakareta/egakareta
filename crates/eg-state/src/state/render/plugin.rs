/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Rendering extraction plugin.
//!
//! Owns the render resource boundary + system-set graph. `Runtime`
//! construction inserts the authoritative heavy resources
//! ([`RenderGpuRes`], [`RenderMeshesRes`], [`RenderSurfaceHostRes`]);
//! the per-frame schedule mirrors POD frame/uniform values, uploads extracted
//! mesh buffers, and presents the scene plus egui from App-owned resources.
//!
//! REFACTOR POINT(full-bevy): this plugin + system set graph is the
//! placeholder for `bevy_render`'s render-world + render graph. The
//! `extract_render_state_system` here becomes a
//! `RenderApp::extract` system in the full-Bevy migration; the
//! `RenderPresentSet` is replaced by `bevy_render`'s `Render` set.

use bevy_app::{App, Plugin};
use bevy_ecs::{
    prelude::*,
    schedule::{Schedule, SystemSet},
    system::SystemParam,
};
use bevy_state::prelude::State;
use eg_blocks::{BlockDefinition, BlockRenderProfile};
use eg_editor_domain::{
    TapDivisionPreviewRange, derive_timing_division_tap_previews, tap_time_is_timing_division,
};
use eg_game::trigger_transformed_objects_at_time;
use eg_mesh::{
    BlockEffectInstance, GizmoParams, GizmoPart, PracticeCheckpointFlagInstance,
    TransformTriggerMarker, Vertex, build_block_effect_vertices, build_block_geometry_for_object,
    build_block_geometry_from_refs, build_block_geometry_from_refs_at_time,
    build_block_instance_batches_from_refs_at_time, build_camera_arrow_vertices,
    build_chunked_block_mesh_from_refs, build_chunked_block_mesh_from_refs_at_time,
    build_colored_tap_indicator_vertices, build_editor_cursor_vertices,
    build_editor_gizmo_vertices, build_editor_hitbox_visualization_vertices,
    build_editor_hover_outline_vertices, build_editor_selection_outline_vertices,
    build_editor_tap_cursor_vertices, build_editor_transform_trigger_target_outline_vertices,
    build_practice_checkpoint_flag_geometry, build_spawn_marker_vertices,
    build_tap_division_preview_vertices, build_tap_division_tap_marker_vertices,
    build_transform_trigger_marker_vertices,
};
use eg_render::EditorOutlineInstance;
use eg_types::{
    AppPhase, CameraUniform, ColorSpaceUniform, Direction, EditorMode, LevelMetadata, LevelObject,
    LineUniform, TransformVisibility, TriggerTarget,
};
use glam::{Mat4, Vec2, Vec3};

use super::{
    draw::render_egui_with_resources,
    resources::{
        RenderBlockEffectVerticesRes, RenderConfigRes, RenderEditorBlockMeshesRes,
        RenderEditorCameraArrowVerticesRes, RenderEditorCursorVerticesRes,
        RenderEditorGizmoVerticesRes, RenderEditorHitboxVerticesRes, RenderEditorHoverOverlayRes,
        RenderEditorSelectionOverlayRes, RenderEditorSpawnMarkerVerticesRes,
        RenderEditorTapIndicatorVerticesRes, RenderEditorTransformTriggerMarkerVerticesRes,
        RenderEguiRes, RenderFrameInputRes, RenderGpuRes, RenderMenuPreviewBlockMeshesRes,
        RenderMeshesRes, RenderPlayingBlockMeshesRes, RenderPracticeCheckpointVerticesRes,
        RenderResizeRequest, RenderSurfaceHostRes, RenderTrailVerticesRes, RenderUniformUploadRes,
        RenderUniformsRes,
    },
};
use crate::state::{
    BlockEffectsRes, EditorCameraRes, EditorConfigRes, EditorInputRes, EditorRuntimeRes,
    EditorSpawnRes, EditorTimelineRes, EditorTimingRes, EditorTriggerRes, FrameTimeRes,
    MenuPreviewObjectsRes, MenuStateRes, SessionStateRes,
    editor_trail::{build_editor_playback_trail_vertices, build_editor_scrub_trail_vertices},
    evaluate_camera_track_for_objects,
};

/// System sets for the rendering extraction plugin.
///
/// These sets define the extraction and presentation order for the custom
/// renderer.
///
/// Ordering contract:
/// ```text
///   RenderExtractSet    // build SceneMeshes + uniform values from ECS
///   RenderPresentSet    // read authoritative render resources
/// ```
///
/// The custom renderer remains the presentation backend; Bevy owns the
/// scheduling and authoritative world data.
#[allow(dead_code)]
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderSet {
    /// Snapshot the world into render resources.
    ///
    /// Extraction walks authored and gameplay entities plus App resources and
    /// fills the custom renderer's mesh/POD resources via `eg_mesh` builders.
    Extract,
    /// Issue the wgpu present.
    ///
    /// Reads the authoritative render resources and performs the custom egui
    /// and wgpu present.
    Present,
}

/// The rendering extraction plugin.
///
/// Registers the POD resources for an `App`. `AppBootstrap` inserts real heavy
/// resources after wgpu initialization.
pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RenderUniformsRes>()
            .init_resource::<RenderFrameInputRes>()
            .init_resource::<RenderTrailVerticesRes>()
            .init_resource::<RenderEditorCameraArrowVerticesRes>()
            .init_resource::<RenderEditorCursorVerticesRes>()
            .init_resource::<RenderEditorHitboxVerticesRes>()
            .init_resource::<RenderEditorTapIndicatorVerticesRes>()
            .init_resource::<RenderEditorSelectionOverlayRes>()
            .init_resource::<RenderEditorHoverOverlayRes>()
            .init_resource::<RenderEditorTransformTriggerMarkerVerticesRes>()
            .init_resource::<RenderBlockEffectVerticesRes>()
            .init_resource::<RenderPracticeCheckpointVerticesRes>()
            .init_resource::<RenderPlayingBlockMeshesRes>()
            .init_resource::<RenderMenuPreviewBlockMeshesRes>()
            .init_resource::<RenderEditorBlockMeshesRes>()
            .init_resource::<RenderEditorSpawnMarkerVerticesRes>()
            .init_resource::<RenderEditorGizmoVerticesRes>()
            .init_resource::<RenderConfigRes>()
            .init_resource::<RenderResizeRequest>()
            .init_non_send::<RenderMeshesRes>()
            .init_non_send::<RenderSurfaceHostRes>();
    }
}

/// `World`-flavoured registration for callers that build a `World` directly.
/// Mirrors
/// [`RenderPlugin::build`] without going through an `App`.
#[allow(dead_code)]
pub fn register_render_pod_resources(world: &mut World) {
    world.init_resource::<RenderUniformsRes>();
    world.init_resource::<RenderFrameInputRes>();
    world.init_resource::<RenderTrailVerticesRes>();
    world.init_resource::<RenderEditorCameraArrowVerticesRes>();
    world.init_resource::<RenderEditorCursorVerticesRes>();
    world.init_resource::<RenderEditorHitboxVerticesRes>();
    world.init_resource::<RenderEditorTapIndicatorVerticesRes>();
    world.init_resource::<RenderEditorSelectionOverlayRes>();
    world.init_resource::<RenderEditorHoverOverlayRes>();
    world.init_resource::<RenderEditorTransformTriggerMarkerVerticesRes>();
    world.init_resource::<RenderBlockEffectVerticesRes>();
    world.init_resource::<RenderPracticeCheckpointVerticesRes>();
    world.init_resource::<RenderPlayingBlockMeshesRes>();
    world.init_resource::<RenderMenuPreviewBlockMeshesRes>();
    world.init_resource::<RenderEditorBlockMeshesRes>();
    world.init_resource::<RenderEditorSpawnMarkerVerticesRes>();
    world.init_resource::<RenderEditorGizmoVerticesRes>();
    world.init_resource::<RenderConfigRes>();
    world.init_resource::<RenderResizeRequest>();
    // Insert default heavy resources so standalone worlds can query them.
    // `AppBootstrap` replaces these defaults with the authoritative
    // `SceneMeshes` / `SurfaceHost` values.
    world.init_non_send::<RenderMeshesRes>();
    world.init_non_send::<RenderSurfaceHostRes>();
}

/// Resource-backed `RenderSet::Present` boundary.
///
/// Production consumes App-owned GPU, mesh, surface, uniform, frame-input,
/// and egui resources directly.
pub fn present_render_frame_system(
    mut gpu: Option<NonSendMut<RenderGpuRes>>,
    mut meshes: Option<NonSendMut<RenderMeshesRes>>,
    surface_host: Option<NonSend<RenderSurfaceHostRes>>,
    mut egui: Option<NonSendMut<RenderEguiRes>>,
    uniforms: Res<RenderUniformsRes>,
    frame_input: Res<RenderFrameInputRes>,
) {
    let (Some(mut gpu), Some(mut meshes), Some(mut egui)) =
        (gpu.take(), meshes.take(), egui.take())
    else {
        return;
    };
    let RenderEguiRes {
        renderer,
        paint_jobs,
        screen_descriptor,
    } = &mut *egui;
    let result = render_egui_with_resources(
        &mut gpu.0,
        &mut meshes.0,
        renderer,
        paint_jobs,
        screen_descriptor,
        frame_input.0,
        uniforms.camera.view_proj,
    );
    if matches!(
        result,
        Err(eg_render::RenderSurfaceError::Lost) | Err(eg_render::RenderSurfaceError::Outdated)
    ) {
        let size = surface_host
            .as_ref()
            .and_then(|host| host.0.as_ref())
            .map(|host| host.current_size())
            .unwrap_or(gpu.0.size);
        if size.width > 0 && size.height > 0 {
            if let Some(host) = surface_host
                .as_ref()
                .and_then(|surface_host| surface_host.0.as_ref())
            {
                host.prepare_resize(size);
            }
            gpu.0.apply_resize(size);
        }
    } else if let Err(error) = result {
        log::error!("Render error: {error:?}");
    }
}

/// Build the per-frame render input directly from App-world resources.
///
/// This builds the POD frame-input value consumed by the custom presenter.
#[derive(SystemParam)]
pub(crate) struct RenderFrameInputParams<'w, 's> {
    phase: Res<'w, State<AppPhase>>,
    input: Res<'w, EditorInputRes>,
    session: Res<'w, SessionStateRes>,
    camera: Res<'w, EditorCameraRes>,
    timeline: Res<'w, EditorTimelineRes>,
    clock: Option<Res<'w, eg_game::GameClock>>,
    menu: Option<Res<'w, MenuStateRes>>,
    config: Res<'w, RenderConfigRes>,
    player: Query<'w, 's, &'static eg_game::PlayerPosition>,
    frame_input: ResMut<'w, RenderFrameInputRes>,
}

/// Build the per-frame render input from the grouped App-world parameters.
pub(crate) fn update_render_frame_input_system(mut params: RenderFrameInputParams) {
    let current_phase = *params.phase.get();
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
        apply_gamma_correction: params.config.apply_gamma_correction,
    };
}

/// Extract the gameplay ghost trail from App-owned gameplay components.
///
/// The scheduled upload system writes this extracted vertex list to the
/// App-owned GPU mesh storage.
pub(crate) fn extract_trail_vertices_system(
    phase: Res<State<AppPhase>>,
    input: Res<EditorInputRes>,
    timeline: Res<EditorTimelineRes>,
    clock: Option<Res<eg_game::GameClock>>,
    player: Query<(
        &eg_game::PlayerPosition,
        &eg_game::PlayerDirection,
        &eg_game::Grounded,
        &eg_game::Trail,
    )>,
    mut trail_vertices: ResMut<RenderTrailVerticesRes>,
) {
    if *phase.get() == AppPhase::Editor {
        if input.0.mode == EditorMode::Timing {
            trail_vertices.0.clear();
        } else if timeline.0.playback.playing {
            trail_vertices.0 = timeline
                .0
                .playback
                .runtime
                .as_ref()
                .map(build_editor_playback_trail_vertices)
                .unwrap_or_default();
        } else {
            let target_time = timeline
                .0
                .clock
                .time_seconds
                .clamp(0.0, timeline.0.clock.duration_seconds.max(0.0));
            trail_vertices.0 = build_editor_scrub_trail_vertices(&timeline.0, target_time);
        }
        return;
    }

    if !matches!(*phase.get(), AppPhase::Playing | AppPhase::GameOver) {
        trail_vertices.0.clear();
        return;
    }

    let Some((position, direction, grounded, trail)) = player.iter().next() else {
        trail_vertices.0.clear();
        return;
    };

    let game_over = clock.as_ref().is_some_and(|clock| clock.game_over);
    const CULL_DISTANCE_SQ: f32 = 120.0 * 120.0;
    const MAX_RENDERED_PLAYING_TRAIL_POINTS_PER_SEGMENT: usize = 1024;

    let mut vertices = Vec::new();
    for (segment_index, segment) in trail.segments.iter().enumerate() {
        if segment.is_empty() {
            continue;
        }

        let is_last_segment = segment_index + 1 == trail.segments.len();
        if !is_last_segment {
            let Some(last_point) = segment.last() else {
                continue;
            };
            let dx = last_point[0] - position.0[0];
            let dy = last_point[1] - position.0[1];
            let dz = last_point[2] - position.0[2];
            if dx * dx + dy * dy + dz * dz > CULL_DISTANCE_SQ {
                continue;
            }
        }

        let start = segment
            .len()
            .saturating_sub(MAX_RENDERED_PLAYING_TRAIL_POINTS_PER_SEGMENT);
        if is_last_segment && grounded.0 {
            let mut points = segment[start..].to_vec();
            points.push(position.0);
            vertices.extend(eg_mesh::build_trail_vertices(&points, game_over));
        } else {
            vertices.extend(eg_mesh::build_trail_vertices(&segment[start..], game_over));
        }
    }

    if !grounded.0 {
        let head_length = 0.22;
        let dir = match direction.0 {
            eg_types::Direction::Forward => [0.0, 1.0],
            eg_types::Direction::Right => [1.0, 0.0],
        };
        let head_start = [
            position.0[0] - dir[0] * head_length,
            position.0[1],
            position.0[2] - dir[1] * head_length,
        ];
        vertices.extend(eg_mesh::build_trail_vertices(
            &[head_start, position.0],
            game_over,
        ));
    }

    trail_vertices.0 = vertices;
}

/// Extract active block-effect vertices from the App-owned effect lifecycle.
/// The scheduled upload system writes the derived vertices to the GPU mesh.
pub(crate) fn extract_block_effect_vertices_system(
    effects: Res<BlockEffectsRes>,
    mut effect_vertices: ResMut<RenderBlockEffectVerticesRes>,
) {
    if effects.effects.is_empty() {
        effect_vertices.0.clear();
        return;
    }

    let instances = effects
        .effects
        .iter()
        .map(|effect| BlockEffectInstance {
            position: effect.position,
            size: effect.size,
            color: effect.color,
            effect: effect.effect,
            age_seconds: effect.age_seconds,
        })
        .collect::<Vec<_>>();
    effect_vertices.0 = build_block_effect_vertices(&instances);
}

/// Extract practice-checkpoint flags from the App-owned session resource.
/// The custom renderer receives the resulting vertices through the scheduled
/// mesh upload path.
pub(crate) fn extract_practice_checkpoint_vertices_system(
    phase: Res<State<AppPhase>>,
    session: Res<SessionStateRes>,
    mut checkpoint_vertices: ResMut<RenderPracticeCheckpointVerticesRes>,
) {
    if *phase.get() != AppPhase::Playing
        || session.playtesting_editor
        || !session.practice_mode_enabled
        || session.practice_checkpoints.is_empty()
    {
        checkpoint_vertices.0.clear();
        return;
    }

    let latest_index = session.practice_checkpoints.len().saturating_sub(1);
    let flags = session
        .practice_checkpoints
        .iter()
        .enumerate()
        .map(|(index, checkpoint)| PracticeCheckpointFlagInstance {
            position: checkpoint.gameplay.position,
            direction: checkpoint.gameplay.direction,
            is_latest: index == latest_index,
        })
        .collect::<Vec<_>>();
    checkpoint_vertices.0 = build_practice_checkpoint_flag_geometry(&flags).vertices;
}
fn gameplay_object_order_key(id: &str) -> (u8, usize, &str) {
    id.strip_prefix("__obj_")
        .and_then(|suffix| suffix.parse::<usize>().ok())
        .map(|index| (0, index, id))
        .unwrap_or((1, 0, id))
}

type GameplayRenderObjects<'w, 's> = Query<
    'w,
    's,
    (&'static eg_game::ObjectId, &'static eg_game::ObjectData),
    (
        With<eg_game::Object>,
        Without<super::super::editor_objects::EditorAuthoredObject>,
    ),
>;

fn editor_trigger_objects_for_render(objects: &[LevelObject]) -> Vec<LevelObject> {
    let mut triggers = objects
        .iter()
        .filter(|object| object.trigger.is_some())
        .cloned()
        .collect::<Vec<_>>();
    triggers.sort_by(|left, right| {
        let left_time = left
            .trigger
            .as_ref()
            .map(|trigger| trigger.time_seconds)
            .unwrap_or(0.0);
        let right_time = right
            .trigger
            .as_ref()
            .map(|trigger| trigger.time_seconds)
            .unwrap_or(0.0);
        f32::total_cmp(&left_time, &right_time)
    });
    triggers
}

fn is_play_world_render_object(object: &LevelObject) -> bool {
    BlockDefinition::get(&object.block_id).render.profile != BlockRenderProfile::CameraTrigger
}

fn has_object_transform_triggers(triggers: &[LevelObject]) -> bool {
    triggers.iter().any(|object| {
        let Some(trigger) = object.trigger.as_ref() else {
            return false;
        };
        trigger.transform && !matches!(trigger.target, TriggerTarget::Camera)
    })
}

fn has_camera_timeline_triggers(triggers: &[LevelObject]) -> bool {
    triggers.iter().any(|object| {
        let Some(trigger) = object.trigger.as_ref() else {
            return false;
        };
        matches!(trigger.target, TriggerTarget::Camera) && trigger.camera.is_some()
    })
}

fn editor_objects_for_render(
    objects: &[LevelObject],
    timeline: &EditorTimelineRes,
    trigger_objects: &[LevelObject],
) -> Vec<LevelObject> {
    if !timeline.0.playback.playing {
        if has_object_transform_triggers(trigger_objects) {
            return trigger_transformed_objects_at_time(
                objects,
                trigger_objects,
                timeline.0.clock.time_seconds,
            );
        }
        return objects.to_vec();
    }

    if !has_object_transform_triggers(trigger_objects) {
        return timeline
            .0
            .playback
            .runtime
            .as_ref()
            .map(|runtime| runtime.objects().to_vec())
            .unwrap_or_else(|| objects.to_vec());
    }

    trigger_transformed_objects_at_time(objects, trigger_objects, timeline.0.clock.time_seconds)
}

/// Extract time-dependent gameplay block batches from App-owned gameplay
/// entities.
#[derive(PartialEq)]
pub(crate) struct PlayingStaticBlockMeshCache {
    objects: Vec<LevelObject>,
}

pub(crate) fn extract_playing_block_meshes_system(
    phase: Res<State<AppPhase>>,
    session: Res<SessionStateRes>,
    clock: Option<Res<eg_game::GameClock>>,
    authored_objects: Query<
        (
            &super::super::editor_objects::EditorObjectOrder,
            &eg_game::ObjectData,
        ),
        With<super::super::editor_objects::EditorAuthoredObject>,
    >,
    objects: GameplayRenderObjects<'_, '_>,
    mut block_meshes: ResMut<RenderPlayingBlockMeshesRes>,
    mut static_cache: Local<Option<PlayingStaticBlockMeshCache>>,
) {
    if !matches!(*phase.get(), AppPhase::Playing | AppPhase::GameOver) || session.game_paused {
        if static_cache.take().is_some() {
            block_meshes.result = None;
            block_meshes.chunked = None;
        }
        return;
    }
    let Some(clock) = clock else {
        return;
    };

    block_meshes.result = None;
    block_meshes.chunked = None;

    let mut gameplay_objects = objects
        .iter()
        .map(|(id, data)| (gameplay_object_order_key(&id.0), data.0.clone()))
        .collect::<Vec<_>>();
    gameplay_objects.sort_by(|left, right| left.0.cmp(&right.0));
    let mut gameplay_objects = gameplay_objects
        .into_iter()
        .map(|(_, object)| object)
        .collect::<Vec<_>>();
    let authored_objects =
        super::super::editor_objects::collect_editor_object_data(&authored_objects);
    let trigger_objects = editor_trigger_objects_for_render(&authored_objects);
    let has_object_transform_triggers = has_object_transform_triggers(&trigger_objects);
    let has_camera_timeline_triggers = has_camera_timeline_triggers(&trigger_objects);
    let has_animated_blocks = gameplay_objects.iter().any(|object| {
        BlockDefinition::get(&object.block_id)
            .animation
            .idle
            .has_motion()
    });
    if has_object_transform_triggers {
        gameplay_objects = trigger_transformed_objects_at_time(
            &gameplay_objects,
            &trigger_objects,
            clock.elapsed_seconds.max(0.0),
        );
    }
    let render_object_refs = gameplay_objects
        .iter()
        .filter(|object| is_play_world_render_object(object))
        .collect::<Vec<_>>();
    if has_animated_blocks || has_object_transform_triggers || has_camera_timeline_triggers {
        *static_cache = None;
        block_meshes.result = Some(build_block_instance_batches_from_refs_at_time(
            &render_object_refs,
            Some(clock.elapsed_seconds),
        ));
    } else {
        let cache = PlayingStaticBlockMeshCache {
            objects: gameplay_objects.clone(),
        };
        if static_cache.as_ref() == Some(&cache) {
            return;
        }
        *static_cache = Some(cache);
        block_meshes.chunked = Some(build_chunked_block_mesh_from_refs(&render_object_refs));
    }
}

/// Extract the selected menu preview blocks from App-owned preview objects.
/// The cache key is the selected preview index so an idle menu does not
/// rebuild the same static chunk mesh every frame.
pub(crate) fn extract_menu_preview_block_mesh_system(
    phase: Res<State<AppPhase>>,
    menu: Option<Res<MenuStateRes>>,
    preview_objects: Res<MenuPreviewObjectsRes>,
    mut block_meshes: ResMut<RenderMenuPreviewBlockMeshesRes>,
) {
    if *phase.get() != AppPhase::Menu {
        return;
    }
    let Some(menu) = menu else {
        return;
    };
    if block_meshes.preview_level_index == menu.0.preview_level_index
        && block_meshes.chunked.is_some()
    {
        return;
    }

    let objects = preview_objects.objects.iter().collect::<Vec<_>>();
    block_meshes.preview_level_index = menu.0.preview_level_index;
    block_meshes.chunked = Some(build_chunked_block_mesh_from_refs(&objects));
}

/// Extract the editor static and selected block meshes from App-owned editor
/// resources.
#[derive(PartialEq)]
pub(crate) struct EditorBlockMeshCache {
    objects: Vec<LevelObject>,
    selected_indices: Vec<usize>,
    time_seconds: Option<f32>,
}

pub(crate) fn extract_editor_block_meshes_system(
    phase: Res<State<AppPhase>>,
    input: Res<EditorInputRes>,
    authored_objects: Query<
        (
            &super::super::editor_objects::EditorObjectOrder,
            &eg_game::ObjectData,
        ),
        With<super::super::editor_objects::EditorAuthoredObject>,
    >,
    timeline: Res<EditorTimelineRes>,
    mut block_meshes: ResMut<RenderEditorBlockMeshesRes>,
    mut cache: Local<Option<EditorBlockMeshCache>>,
) {
    if *phase.get() != AppPhase::Editor {
        if cache.take().is_some() {
            block_meshes.static_mesh = eg_mesh::ChunkedBlockMesh::default();
            block_meshes.selected_geometry = eg_mesh::MeshGeometry::default();
        }
        return;
    }

    let authored_objects =
        super::super::editor_objects::collect_editor_object_data(&authored_objects);
    let trigger_objects = editor_trigger_objects_for_render(&authored_objects);
    let object_source = editor_objects_for_render(&authored_objects, &timeline, &trigger_objects);
    let selected_indices = normalized_editor_selection_indices(&input, object_source.len());
    let selected_mask = selected_indices
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    let static_objects = object_source
        .iter()
        .enumerate()
        .filter_map(|(index, object)| (!selected_mask.contains(&index)).then_some(object))
        .collect::<Vec<_>>();
    let time_seconds =
        has_camera_timeline_triggers(&trigger_objects).then_some(timeline.0.clock.time_seconds);
    let cache_key = EditorBlockMeshCache {
        objects: object_source.clone(),
        selected_indices: selected_indices.clone(),
        time_seconds,
    };
    if cache.as_ref() == Some(&cache_key) {
        return;
    }
    *cache = Some(cache_key);
    block_meshes.static_mesh = match time_seconds {
        Some(time_seconds) => build_chunked_block_mesh_from_refs_at_time(
            &static_objects,
            Some(time_seconds),
            eg_mesh::DEFAULT_RENDER_CHUNK_SIZE,
        ),
        None => build_chunked_block_mesh_from_refs(&static_objects),
    };

    if selected_indices.len() > 100_000 {
        log::warn!(
            "Skipping selected block mesh for {} selected objects to avoid an oversized expanded mesh",
            selected_indices.len()
        );
        return;
    }
    let selected_objects = object_source
        .iter()
        .enumerate()
        .filter_map(|(index, object)| selected_mask.contains(&index).then_some(object))
        .collect::<Vec<_>>();
    block_meshes.selected_geometry = match time_seconds {
        Some(time_seconds) => {
            build_block_geometry_from_refs_at_time(&selected_objects, Some(time_seconds))
        }
        None => build_block_geometry_from_refs(&selected_objects),
    };
}

/// Extract the editor spawn marker from App-owned spawn metadata.
pub(crate) fn extract_editor_spawn_marker_vertices_system(
    phase: Res<State<AppPhase>>,
    spawn: Res<EditorSpawnRes>,
    mut marker_vertices: ResMut<RenderEditorSpawnMarkerVerticesRes>,
) {
    if *phase.get() != AppPhase::Editor {
        marker_vertices.0.clear();
        return;
    }

    marker_vertices.0 = build_spawn_marker_vertices(
        spawn.0.position,
        matches!(spawn.0.direction, Direction::Right),
    );
}

fn editor_selection_bounds_for_gizmo(
    objects: &[LevelObject],
    selected_indices: &[usize],
    trigger_target: Option<&LevelObject>,
) -> Option<([f32; 3], [f32; 3])> {
    let Some(first_index) = selected_indices.first().copied() else {
        return trigger_target.map(|object| (object.position, object.size));
    };
    let first = objects.get(first_index)?;
    let mut min = first.position;
    let mut max = [
        first.position[0] + first.size[0],
        first.position[1] + first.size[1],
        first.position[2] + first.size[2],
    ];
    for index in selected_indices.iter().copied().skip(1) {
        let Some(object) = objects.get(index) else {
            continue;
        };
        min[0] = min[0].min(object.position[0]);
        min[1] = min[1].min(object.position[1]);
        min[2] = min[2].min(object.position[2]);
        max[0] = max[0].max(object.position[0] + object.size[0]);
        max[1] = max[1].max(object.position[1] + object.size[1]);
        max[2] = max[2].max(object.position[2] + object.size[2]);
    }
    Some((min, [max[0] - min[0], max[1] - min[1], max[2] - min[2]]))
}

fn gizmo_pixels_to_world_scale(
    center: Vec3,
    pixels: f32,
    uniforms: &RenderUniformsRes,
    config: &RenderConfigRes,
) -> f32 {
    let view_proj = Mat4::from_cols_array_2d(&uniforms.camera.view_proj);
    let viewport = Vec2::new(config.width.max(1) as f32, config.height.max(1) as f32);
    let world_to_screen = |point: Vec3| {
        let clip = view_proj * point.extend(1.0);
        if clip.w.abs() <= f32::EPSILON {
            return None;
        }
        let ndc = clip.truncate() / clip.w;
        Some(Vec2::new(
            (ndc.x + 1.0) * 0.5 * viewport.x,
            (1.0 - ndc.y) * 0.5 * viewport.y,
        ))
    };
    let Some(origin) = world_to_screen(center) else {
        return 0.06;
    };
    [Vec3::X, Vec3::Y, Vec3::Z]
        .into_iter()
        .filter_map(|axis| {
            let screen = world_to_screen(center + axis)?;
            let pixels_per_world = screen.distance(origin);
            (pixels_per_world > f32::EPSILON).then_some(pixels / pixels_per_world)
        })
        .reduce(f32::min)
        .unwrap_or(0.06)
}

/// Extract the editor transform gizmo from App-owned selection, objects,
/// interaction state, camera uniform, and viewport configuration.
#[derive(SystemParam)]
pub(crate) struct EditorGizmoExtractionParams<'w, 's> {
    phase: Res<'w, State<AppPhase>>,
    input: Res<'w, EditorInputRes>,
    authored_objects: Query<
        'w,
        's,
        (
            &'static super::super::editor_objects::EditorObjectOrder,
            &'static eg_game::ObjectData,
        ),
        With<super::super::editor_objects::EditorAuthoredObject>,
    >,
    triggers: Res<'w, EditorTriggerRes>,
    runtime: Res<'w, EditorRuntimeRes>,
    uniforms: Res<'w, RenderUniformsRes>,
    config: Res<'w, RenderConfigRes>,
    gizmo_vertices: ResMut<'w, RenderEditorGizmoVerticesRes>,
}

pub(crate) fn extract_editor_gizmo_vertices_system(mut params: EditorGizmoExtractionParams) {
    params.gizmo_vertices.0.clear();
    if *params.phase.get() != AppPhase::Editor {
        return;
    }

    let authored_objects =
        super::super::editor_objects::collect_editor_object_data(&params.authored_objects);
    let trigger_objects = editor_trigger_objects_for_render(&authored_objects);
    let trigger_target =
        if normalized_editor_selection_indices(&params.input, authored_objects.len()).is_empty() {
            params
                .triggers
                .0
                .selected_index
                .and_then(|index| trigger_objects.get(index))
                .filter(|object| object.is_transform_trigger())
        } else {
            None
        };
    let selected_indices =
        normalized_editor_selection_indices(&params.input, authored_objects.len());
    let Some((bounds_position, bounds_size)) =
        editor_selection_bounds_for_gizmo(&authored_objects, &selected_indices, trigger_target)
    else {
        return;
    };
    let visibility = if selected_indices.is_empty() {
        trigger_target
            .map(|_| TransformVisibility::all())
            .unwrap_or_else(TransformVisibility::none)
    } else {
        selected_indices
            .iter()
            .filter_map(|index| authored_objects.get(*index))
            .map(LevelObject::transform_field_visibility)
            .fold(
                TransformVisibility::all(),
                TransformVisibility::intersection,
            )
    };
    let mode = params.input.0.mode;
    let show_move_handles = mode.shows_move_gizmo() && visibility.position;
    let show_scale_handles = mode.shows_scale_gizmo() && visibility.size;
    let show_rotate_handles = mode.shows_rotate_gizmo() && visibility.rotation;
    if !show_move_handles && !show_scale_handles && !show_rotate_handles {
        return;
    }

    let center = Vec3::new(
        bounds_position[0] + bounds_size[0] * 0.5,
        bounds_position[1] + bounds_size[1] * 0.5,
        bounds_position[2] + bounds_size[2] * 0.5,
    );
    let scale = gizmo_pixels_to_world_scale(center, 100.0, &params.uniforms, &params.config);
    let axis_width = gizmo_pixels_to_world_scale(center, 4.5, &params.uniforms, &params.config);
    let resize_radius = gizmo_pixels_to_world_scale(center, 8.5, &params.uniforms, &params.config);
    let resize_offsets = [
        gizmo_pixels_to_world_scale(center, 9.0, &params.uniforms, &params.config),
        gizmo_pixels_to_world_scale(center, 9.0, &params.uniforms, &params.config),
        gizmo_pixels_to_world_scale(center, 9.0, &params.uniforms, &params.config),
    ];
    let rotation = selected_indices
        .first()
        .and_then(|index| authored_objects.get(*index))
        .map(|object| object.rotation)
        .or_else(|| trigger_target.map(|object| object.rotation))
        .unwrap_or([0.0, 0.0, 0.0]);
    let dragged_part = params
        .runtime
        .0
        .interaction
        .gizmo_drag
        .as_ref()
        .map(|drag| GizmoPart::from_axis_kind(drag.axis, drag.kind));
    let hovered_part = params
        .runtime
        .0
        .interaction
        .hovered_gizmo
        .map(|(kind, axis)| GizmoPart::from_axis_kind(axis, kind));
    params.gizmo_vertices.0 = build_editor_gizmo_vertices(GizmoParams {
        position: bounds_position,
        size: bounds_size,
        rotation,
        axis_lengths: [scale, scale, scale],
        axis_width,
        resize_radius,
        resize_offsets,
        show_move_handles,
        show_scale_handles,
        show_rotate_handles,
        hovered_part,
        dragged_part,
    });
}

/// Extract the editor camera arrow from App-owned camera/timeline resources.
/// The scheduled upload system writes the derived vertices to the GPU mesh.
pub(crate) fn extract_editor_camera_arrow_vertices_system(
    phase: Res<State<AppPhase>>,
    authored_objects: Query<
        (
            &super::super::editor_objects::EditorObjectOrder,
            &eg_game::ObjectData,
        ),
        With<super::super::editor_objects::EditorAuthoredObject>,
    >,
    camera: Res<EditorCameraRes>,
    timeline: Res<EditorTimelineRes>,
    mut arrow_vertices: ResMut<RenderEditorCameraArrowVerticesRes>,
) {
    if *phase.get() != AppPhase::Editor || timeline.0.playback.playing {
        arrow_vertices.0.clear();
        return;
    }

    let authored_objects =
        super::super::editor_objects::collect_editor_object_data(&authored_objects);
    let target = Vec3::from_array(timeline.0.preview.position);
    let sample = evaluate_camera_track_for_objects(
        &camera.0,
        &authored_objects,
        timeline.0.clock.time_seconds,
        target,
        true,
    );
    let forward = {
        let direction = sample.target - sample.eye;
        if direction.length_squared() > f32::EPSILON {
            direction.normalize().to_array()
        } else {
            [0.0, 0.0, 1.0]
        }
    };
    let editor_camera_eye =
        (camera.0.editor_camera_target() + camera.0.editor_camera_offset()).to_array();
    arrow_vertices.0 =
        build_camera_arrow_vertices(sample.eye.to_array(), forward, editor_camera_eye);
}

/// Extract the editor cursor from App-owned input/config/timeline resources.
/// The scheduled upload system writes the derived vertices to the GPU mesh.
pub(crate) fn extract_editor_cursor_vertices_system(
    phase: Res<State<AppPhase>>,
    input: Res<EditorInputRes>,
    config: Res<EditorConfigRes>,
    runtime: Res<EditorRuntimeRes>,
    timeline: Res<EditorTimelineRes>,
    mut cursor_vertices: ResMut<RenderEditorCursorVerticesRes>,
) {
    if *phase.get() != AppPhase::Editor {
        cursor_vertices.0.clear();
        return;
    }

    let effective_mode = if timeline.0.playback.playing && input.0.mode == EditorMode::Null {
        runtime.0.interaction.last_mode.unwrap_or(input.0.mode)
    } else {
        input.0.mode
    };

    if effective_mode == EditorMode::Tapping {
        let overlap_index = runtime
            .0
            .interaction
            .hovered_tap_index
            .or(timeline.0.taps.selected_index)
            .filter(|index| {
                timeline
                    .0
                    .taps
                    .tap_indicator_positions
                    .get(*index)
                    .is_some_and(|position| positions_match(*position, input.0.cursor))
            });
        if overlap_index.is_some() {
            cursor_vertices.0.clear();
            return;
        }
        cursor_vertices.0 = build_editor_tap_cursor_vertices(input.0.cursor);
        return;
    }

    let size = BlockDefinition::get(&config.0.selected_block_id)
        .render
        .default_size;
    cursor_vertices.0 = build_editor_cursor_vertices(
        input.0.cursor,
        size,
        &config.0.selected_block_id,
        config.0.selected_block_rotation_degrees,
    );
}

/// Extract editor hitbox visualization from App-owned editor resources.
/// The scheduled upload system writes the derived vertices to the GPU mesh.
pub(crate) fn extract_editor_hitbox_vertices_system(
    phase: Res<State<AppPhase>>,
    input: Res<EditorInputRes>,
    runtime: Res<EditorRuntimeRes>,
    authored_objects: Query<
        (
            &super::super::editor_objects::EditorObjectOrder,
            &eg_game::ObjectData,
        ),
        With<super::super::editor_objects::EditorAuthoredObject>,
    >,
    timeline: Res<EditorTimelineRes>,
    mut hitbox_vertices: ResMut<RenderEditorHitboxVerticesRes>,
) {
    let effective_mode = if timeline.0.playback.playing && input.0.mode == EditorMode::Null {
        runtime.0.interaction.last_mode.unwrap_or(input.0.mode)
    } else {
        input.0.mode
    };
    if *phase.get() != AppPhase::Editor
        || effective_mode == EditorMode::Timing
        || !input.0.show_hitbox_visualization
    {
        hitbox_vertices.0.clear();
        return;
    }

    let player_hitbox = editor_preview_player_hitbox(timeline.0.preview.position);
    let authored_objects =
        super::super::editor_objects::collect_editor_object_data(&authored_objects);
    hitbox_vertices.0 =
        build_editor_hitbox_visualization_vertices(&authored_objects, Some(player_hitbox));
}

fn editor_preview_player_hitbox(position: [f32; 3]) -> LevelObject {
    const PLAYER_WIDTH: f32 = 0.8;
    const PLAYER_HEIGHT: f32 = 0.8;
    const PLAYER_FOOTPRINT_TOLERANCE: f32 = PLAYER_WIDTH * 0.05;

    let inset_width = PLAYER_WIDTH - PLAYER_FOOTPRINT_TOLERANCE * 2.0;
    let inset_height = PLAYER_HEIGHT - PLAYER_FOOTPRINT_TOLERANCE * 2.0;
    LevelObject {
        position: [
            position[0] - inset_width * 0.5,
            position[1] + PLAYER_FOOTPRINT_TOLERANCE,
            position[2] - inset_width * 0.5,
        ],
        size: [inset_width, inset_height, inset_width],
        ..LevelObject::new("core/void")
    }
}

/// Extract editor tap indicators from App-owned timeline and interaction
/// resources. The scheduled upload system writes the derived vertices to the
/// GPU mesh.
#[derive(SystemParam)]
pub(crate) struct EditorTapIndicatorExtractionParams<'w, 's> {
    phase: Res<'w, State<AppPhase>>,
    input: Res<'w, EditorInputRes>,
    runtime: Res<'w, EditorRuntimeRes>,
    authored_objects: Query<
        'w,
        's,
        (
            &'static super::super::editor_objects::EditorObjectOrder,
            &'static eg_game::ObjectData,
        ),
        With<super::super::editor_objects::EditorAuthoredObject>,
    >,
    spawn: Res<'w, EditorSpawnRes>,
    timeline: Res<'w, EditorTimelineRes>,
    timing: Res<'w, EditorTimingRes>,
    tap_vertices: ResMut<'w, RenderEditorTapIndicatorVerticesRes>,
}

pub(crate) fn extract_editor_tap_indicator_vertices_system(
    mut params: EditorTapIndicatorExtractionParams,
) {
    let effective_mode =
        if params.timeline.0.playback.playing && params.input.0.mode == EditorMode::Null {
            params
                .runtime
                .0
                .interaction
                .last_mode
                .unwrap_or(params.input.0.mode)
        } else {
            params.input.0.mode
        };
    if *params.phase.get() != AppPhase::Editor || effective_mode == EditorMode::Timing {
        params.tap_vertices.0.clear();
        return;
    }

    let show_timing_division_previews = effective_mode == EditorMode::Tapping;
    let hovered_tap_index = params.runtime.0.interaction.hovered_tap_index;
    let selected_tap_index = params.timeline.0.taps.selected_index;
    let hovered_division = params.runtime.0.interaction.hovered_tap_division;
    let duration_seconds = params.timeline.0.clock.duration_seconds.max(0.0);
    let authored_objects =
        super::super::editor_objects::collect_editor_object_data(&params.authored_objects);
    let preview_indicators: Vec<([f32; 3], [f32; 4])> = if show_timing_division_previews {
        let previews = derive_timing_division_tap_previews(
            params.spawn.0.position,
            params.spawn.0.direction,
            &params.timeline.0.taps.tap_times,
            &params.timing.0.timing_points,
            duration_seconds,
            TapDivisionPreviewRange {
                start_seconds: 0.0,
                end_seconds: duration_seconds,
            },
            &authored_objects,
        );
        let tap_positions = &params.timeline.0.taps.tap_indicator_positions;
        let mut indicators: Vec<([f32; 3], [f32; 4])> = previews
            .iter()
            .copied()
            .filter(|preview| {
                !tap_positions
                    .iter()
                    .any(|position| positions_match(*position, preview.indicator_position))
            })
            .map(|preview| {
                let is_hovered = hovered_division.is_some_and(|hovered| {
                    (hovered.time_seconds - preview.time_seconds).abs() <= 0.001
                        && positions_match(hovered.indicator_position, preview.indicator_position)
                });
                let alpha = if is_hovered { 0.95 } else { 0.24 };
                (preview.indicator_position, [0.05, 0.48, 0.95, alpha])
            })
            .collect::<Vec<_>>();
        indicators.sort_unstable_by(|a, b| {
            a.0[0]
                .total_cmp(&b.0[0])
                .then(a.0[1].total_cmp(&b.0[1]))
                .then(a.0[2].total_cmp(&b.0[2]))
                .then(a.1[3].total_cmp(&b.1[3]))
        });
        indicators.dedup_by(|a, b| positions_match(a.0, b.0) && (a.1[3] - b.1[3]).abs() < 0.001);
        indicators
    } else {
        Vec::new()
    };

    let tap_positions = &params.timeline.0.taps.tap_indicator_positions;
    let mut indicators = tap_positions
        .iter()
        .copied()
        .enumerate()
        .map(|(index, position)| {
            (
                position,
                editor_tap_indicator_color(index, hovered_tap_index, selected_tap_index),
            )
        })
        .collect::<Vec<_>>();
    indicators.sort_unstable_by(|a, b| {
        a.0[0]
            .total_cmp(&b.0[0])
            .then(a.0[1].total_cmp(&b.0[1]))
            .then(a.0[2].total_cmp(&b.0[2]))
            .then(a.1[3].total_cmp(&b.1[3]))
    });
    indicators.dedup_by(|a, b| {
        positions_match(a.0, b.0)
            && (a.1[0] - b.1[0]).abs() < 0.001
            && (a.1[1] - b.1[1]).abs() < 0.001
            && (a.1[2] - b.1[2]).abs() < 0.001
            && (a.1[3] - b.1[3]).abs() < 0.001
    });

    let division_tap_indicators = if show_timing_division_previews {
        params
            .timeline
            .0
            .taps
            .tap_times
            .iter()
            .copied()
            .zip(tap_positions.iter().copied())
            .filter(|(tap_time, _)| {
                tap_time_is_timing_division(
                    *tap_time,
                    &params.timing.0.timing_points,
                    duration_seconds,
                )
            })
            .map(|(_, position)| (position, [0.0, 0.0, 0.0, 1.0]))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    if indicators.is_empty() && preview_indicators.is_empty() && division_tap_indicators.is_empty()
    {
        params.tap_vertices.0.clear();
        return;
    }

    if indicators.is_empty() && division_tap_indicators.is_empty() {
        params.tap_vertices.0 = build_tap_division_preview_vertices(&preview_indicators);
        return;
    }

    let mut vertices = build_tap_division_preview_vertices(&preview_indicators);
    vertices.extend(build_colored_tap_indicator_vertices(&indicators));
    vertices.extend(build_tap_division_tap_marker_vertices(
        &division_tap_indicators,
    ));
    params.tap_vertices.0 = vertices;
}

fn editor_tap_indicator_color(
    index: usize,
    hovered_tap_index: Option<usize>,
    selected_tap_index: Option<usize>,
) -> [f32; 4] {
    if selected_tap_index == Some(index) {
        [0.2, 0.85, 0.95, 1.0]
    } else if hovered_tap_index == Some(index) {
        [0.1, 0.45, 0.5, 1.0]
    } else {
        [0.0, 0.0, 0.0, 1.0]
    }
}

const SIMPLE_SELECTION_OUTLINE_BLOCK_THRESHOLD: usize = 700;

fn normalized_editor_selection_indices(input: &EditorInputRes, object_count: usize) -> Vec<usize> {
    let mut indices = input
        .0
        .selected_block_indices
        .iter()
        .copied()
        .chain(input.0.selected_block_index)
        .filter(|index| *index < object_count)
        .collect::<Vec<_>>();
    indices.sort_unstable();
    indices.dedup();
    indices
}

fn is_camera_trigger_block(object: &LevelObject) -> bool {
    BlockDefinition::get(&object.block_id).render.profile == BlockRenderProfile::CameraTrigger
}

fn build_editor_object_outline_vertices(
    object: &LevelObject,
    selection: bool,
    line_width: f32,
) -> Vec<Vertex> {
    if is_camera_trigger_block(object) {
        let color = if selection {
            [0.098, 0.6, 1.0, 1.0]
        } else {
            [0.698, 0.898, 1.0, 1.0]
        };
        let outline_width_pixels = (line_width * 1.35).max(1.0);
        let mut vertices = build_block_geometry_for_object(object).to_triangle_vertices();
        for vertex in &mut vertices {
            vertex.color = color;
            vertex.color_outline = [
                object.position[0],
                object.position[1],
                object.position[2],
                outline_width_pixels,
            ];
            vertex.render_profile = 3.0;
        }
        vertices
    } else if selection {
        build_editor_selection_outline_vertices(
            object.position,
            object.size,
            object.rotation,
            line_width,
        )
    } else {
        build_editor_hover_outline_vertices(
            object.position,
            object.size,
            object.rotation,
            line_width,
        )
    }
}

fn transform_trigger_target_indices_for_block(
    objects: &[LevelObject],
    trigger_block_index: usize,
) -> Vec<usize> {
    let Some(trigger) = objects
        .get(trigger_block_index)
        .and_then(|object| object.trigger.as_ref())
    else {
        return Vec::new();
    };
    if !trigger.transform {
        return Vec::new();
    }
    let TriggerTarget::Objects { object_ids } = &trigger.target else {
        return Vec::new();
    };
    let mut indices = objects
        .iter()
        .enumerate()
        .filter_map(|(index, object)| object_ids.contains(&object.id).then_some(index))
        .collect::<Vec<_>>();
    indices.sort_unstable();
    indices.dedup();
    indices
}

fn append_outline_instance(
    mask_vertices: &mut Vec<Vertex>,
    outline_vertices: &mut Vec<Vertex>,
    instances: &mut Vec<EditorOutlineInstance>,
    object: &LevelObject,
    object_outline_vertices: Vec<Vertex>,
) {
    let mask_start = mask_vertices.len() as u32;
    mask_vertices.extend(build_block_geometry_for_object(object).to_triangle_vertices());
    let mask_end = mask_vertices.len() as u32;
    let outline_start = outline_vertices.len() as u32;
    outline_vertices.extend(object_outline_vertices);
    let outline_end = outline_vertices.len() as u32;
    instances.push(EditorOutlineInstance {
        mask_vertices: mask_start..mask_end,
        outline_vertices: outline_start..outline_end,
    });
}

fn append_transform_trigger_target_outline_instances(
    objects: &[LevelObject],
    trigger_indices: impl IntoIterator<Item = usize>,
    mask_vertices: &mut Vec<Vertex>,
    outline_vertices: &mut Vec<Vertex>,
    instances: &mut Vec<EditorOutlineInstance>,
) {
    let mut target_indices = trigger_indices
        .into_iter()
        .flat_map(|index| transform_trigger_target_indices_for_block(objects, index))
        .collect::<Vec<_>>();
    target_indices.sort_unstable();
    target_indices.dedup();
    for target_index in target_indices {
        if let Some(object) = objects.get(target_index) {
            append_outline_instance(
                mask_vertices,
                outline_vertices,
                instances,
                object,
                build_editor_transform_trigger_target_outline_vertices(
                    object.position,
                    object.size,
                    object.rotation,
                    2.5,
                ),
            );
        }
    }
}

fn selected_editor_group_bounds(
    objects: &[LevelObject],
    selected_indices: &[usize],
) -> Option<([f32; 3], [f32; 3])> {
    let first = *selected_indices.first()?;
    let first_object = objects.get(first)?;
    let mut min = first_object.position;
    let mut max = [
        first_object.position[0] + first_object.size[0],
        first_object.position[1] + first_object.size[1],
        first_object.position[2] + first_object.size[2],
    ];
    for index in selected_indices.iter().copied().skip(1) {
        if let Some(object) = objects.get(index) {
            min[0] = min[0].min(object.position[0]);
            min[1] = min[1].min(object.position[1]);
            min[2] = min[2].min(object.position[2]);
            max[0] = max[0].max(object.position[0] + object.size[0]);
            max[1] = max[1].max(object.position[1] + object.size[1]);
            max[2] = max[2].max(object.position[2] + object.size[2]);
        }
    }
    Some((min, [max[0] - min[0], max[1] - min[1], max[2] - min[2]]))
}

/// Extract editor selection and hover outlines from App-owned objects/input.
/// The scheduled upload system writes the derived mesh data to GPU slots.
pub(crate) fn extract_editor_selection_overlays_system(
    phase: Res<State<AppPhase>>,
    input: Res<EditorInputRes>,
    authored_objects: Query<
        (
            &super::super::editor_objects::EditorObjectOrder,
            &eg_game::ObjectData,
        ),
        With<super::super::editor_objects::EditorAuthoredObject>,
    >,
    mut selection: ResMut<RenderEditorSelectionOverlayRes>,
    mut hover: ResMut<RenderEditorHoverOverlayRes>,
) {
    selection.stencil_vertices.clear();
    selection.outline_vertices.clear();
    selection.instances.clear();
    hover.stencil_vertices.clear();
    hover.outline_vertices.clear();

    if *phase.get() != AppPhase::Editor || !input.0.mode.is_transform_mode() {
        return;
    }

    let RenderEditorSelectionOverlayRes {
        stencil_vertices: selection_stencil,
        outline_vertices: selection_outline,
        instances: selection_instances,
    } = &mut *selection;
    let RenderEditorHoverOverlayRes {
        stencil_vertices: hover_stencil,
        outline_vertices: hover_outline,
    } = &mut *hover;

    let authored_objects =
        super::super::editor_objects::collect_editor_object_data(&authored_objects);
    let selected_indices = normalized_editor_selection_indices(&input, authored_objects.len());
    if selected_indices.len() <= SIMPLE_SELECTION_OUTLINE_BLOCK_THRESHOLD {
        for index in selected_indices.iter().copied() {
            if let Some(object) = authored_objects.get(index) {
                append_outline_instance(
                    selection_stencil,
                    selection_outline,
                    selection_instances,
                    object,
                    build_editor_object_outline_vertices(object, true, 2.0),
                );
            }
        }
    } else if let Some((position, size)) =
        selected_editor_group_bounds(&authored_objects, &selected_indices)
    {
        let bounds_object = LevelObject {
            position,
            size,
            ..LevelObject::default()
        };
        append_outline_instance(
            selection_stencil,
            selection_outline,
            selection_instances,
            &bounds_object,
            build_editor_selection_outline_vertices(position, size, [0.0, 0.0, 0.0], 2.0),
        );
    }
    append_transform_trigger_target_outline_instances(
        &authored_objects,
        selected_indices.iter().copied(),
        selection_stencil,
        selection_outline,
        selection_instances,
    );

    let hovered_index = input
        .0
        .hovered_block_index
        .filter(|index| *index < authored_objects.len());
    let selected_mask = selected_indices
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    if let Some(index) = hovered_index
        && !selected_mask.contains(&index)
        && let Some(object) = authored_objects.get(index)
    {
        let mut hover_instances = Vec::new();
        append_outline_instance(
            hover_stencil,
            hover_outline,
            &mut hover_instances,
            object,
            build_editor_object_outline_vertices(object, false, 3.0),
        );
    }

    let mut target_indices = hovered_index
        .into_iter()
        .flat_map(|index| transform_trigger_target_indices_for_block(&authored_objects, index))
        .collect::<Vec<_>>();
    target_indices.sort_unstable();
    target_indices.dedup();
    let mut target_instances = Vec::new();
    for target_index in target_indices {
        if let Some(object) = authored_objects.get(target_index) {
            append_outline_instance(
                hover_stencil,
                hover_outline,
                &mut target_instances,
                object,
                build_editor_transform_trigger_target_outline_vertices(
                    object.position,
                    object.size,
                    object.rotation,
                    2.5,
                ),
            );
        }
    }
}

fn transform_trigger_markers_for_triggers(
    objects: &[LevelObject],
    trigger_objects: &[LevelObject],
    selected_trigger_index: Option<usize>,
) -> Vec<TransformTriggerMarker> {
    let mut markers = Vec::new();
    for (trigger_index, trigger_object) in trigger_objects.iter().enumerate() {
        let Some(trigger) = trigger_object.trigger.as_ref() else {
            continue;
        };
        let TriggerTarget::Objects { object_ids } = &trigger.target else {
            continue;
        };
        if !trigger.transform {
            continue;
        }

        let source_objects = if trigger_index == 0 {
            std::borrow::Cow::Borrowed(objects)
        } else {
            std::borrow::Cow::Owned(trigger_transformed_objects_at_time(
                objects,
                &trigger_objects[..trigger_index],
                trigger.time_seconds,
            ))
        };
        if object_ids.is_empty() {
            markers.push(TransformTriggerMarker {
                source_position: None,
                source_size: None,
                target_position: trigger_object.position,
                target_rotation_degrees: trigger_object.rotation,
                target_size: trigger_object.size,
                time_seconds: trigger.time_seconds,
                duration_seconds: trigger.duration_seconds,
                is_selected: selected_trigger_index == Some(trigger_index),
            });
        } else {
            for object_id in object_ids {
                let Some(source_object) =
                    source_objects.iter().find(|object| object.id == *object_id)
                else {
                    continue;
                };
                markers.push(TransformTriggerMarker {
                    source_position: Some(source_object.position),
                    source_size: Some(source_object.size),
                    target_position: trigger_object.position,
                    target_rotation_degrees: trigger_object.rotation,
                    target_size: trigger_object.size,
                    time_seconds: trigger.time_seconds,
                    duration_seconds: trigger.duration_seconds,
                    is_selected: selected_trigger_index == Some(trigger_index),
                });
            }
        }
    }
    markers
}

/// Extract editor transform-trigger markers from App-owned objects and
/// timeline state. The scheduled upload system writes the derived vertices to
/// the GPU mesh.
pub(crate) fn extract_editor_transform_trigger_marker_vertices_system(
    phase: Res<State<AppPhase>>,
    authored_objects: Query<
        (
            &super::super::editor_objects::EditorObjectOrder,
            &eg_game::ObjectData,
        ),
        With<super::super::editor_objects::EditorAuthoredObject>,
    >,
    triggers: Res<EditorTriggerRes>,
    timeline: Res<EditorTimelineRes>,
    mut marker_vertices: ResMut<RenderEditorTransformTriggerMarkerVerticesRes>,
) {
    if *phase.get() != AppPhase::Editor {
        marker_vertices.0.clear();
        return;
    }

    let authored_objects =
        super::super::editor_objects::collect_editor_object_data(&authored_objects);
    let trigger_objects = authored_objects
        .iter()
        .filter(|object| object.trigger.is_some())
        .cloned()
        .collect::<Vec<_>>();
    let markers = transform_trigger_markers_for_triggers(
        &authored_objects,
        &trigger_objects,
        triggers.0.selected_index,
    );
    if markers.is_empty() {
        marker_vertices.0.clear();
        return;
    }
    marker_vertices.0 =
        build_transform_trigger_marker_vertices(&markers, timeline.0.clock.time_seconds);
}

fn positions_match(left: [f32; 3], right: [f32; 3]) -> bool {
    (left[0] - right[0]).abs() < 0.001
        && (left[1] - right[1]).abs() < 0.001
        && (left[2] - right[2]).abs() < 0.001
}

#[derive(SystemParam)]
pub(crate) struct RenderUniformsParams<'w, 's> {
    phase: Res<'w, State<AppPhase>>,
    camera: Res<'w, EditorCameraRes>,
    authored_objects: Query<
        'w,
        's,
        (
            &'static super::super::editor_objects::EditorObjectOrder,
            &'static eg_game::ObjectData,
        ),
        With<super::super::editor_objects::EditorAuthoredObject>,
    >,
    timeline: Res<'w, EditorTimelineRes>,
    frame_time: Res<'w, FrameTimeRes>,
    clock: Option<Res<'w, eg_game::GameClock>>,
    menu: Option<Res<'w, MenuStateRes>>,
    config: Res<'w, RenderConfigRes>,
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
/// The GPU upload is owned by this scheduled system.
pub(crate) fn update_render_uniforms_system(mut params: RenderUniformsParams) {
    let (width, height, apply_gamma_correction) = (
        params.config.width.max(1),
        params.config.height.max(1),
        params.config.apply_gamma_correction,
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
    let authored_objects =
        super::super::editor_objects::collect_editor_object_data(&params.authored_objects);

    let (eye, target) = match *params.phase.get() {
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
                &authored_objects,
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
                &authored_objects,
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

    let view = glam::camera::rh::view::look_at_mat4(eye, target, Vec3::Y);
    let aspect = width as f32 / height as f32;
    let proj =
        glam::camera::rh::proj::opengl::perspective(45f32.to_radians(), aspect, 0.1, 10000.0);
    let mut line = params.uniforms.line;
    if matches!(*params.phase.get(), AppPhase::Playing | AppPhase::GameOver) {
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

/// Upload the extracted camera and line POD values before presentation.
/// Color-space upload remains in its dedicated system.
pub(crate) fn upload_render_uniforms_system(
    upload: Option<NonSend<RenderUniformUploadRes>>,
    uniforms: Res<RenderUniformsRes>,
) {
    let Some(upload) = upload else {
        return;
    };
    upload.queue.write_buffer(
        &upload.camera_uniform_buffer,
        0,
        bytemuck::bytes_of(&uniforms.camera),
    );
    upload.queue.write_buffer(
        &upload.line_uniform_buffer,
        0,
        bytemuck::bytes_of(&uniforms.line),
    );
}

/// Update the scheduled color-space POD uniform from App-world frame
/// timing. The scheduled render path consumes this value
/// to upload the GPU buffer until render presentation moves fully into
/// systems.
pub fn update_color_space_uniform_system(
    frame_time: Res<FrameTimeRes>,
    mut uniforms: ResMut<RenderUniformsRes>,
) {
    uniforms.color_space.time_seconds = frame_time.global_time_seconds;
}

/// Upload the scheduled color-space POD value before presentation.
pub fn upload_color_space_uniform_system(
    upload: Option<NonSend<RenderUniformUploadRes>>,
    uniforms: Res<RenderUniformsRes>,
) {
    let Some(upload) = upload else {
        return;
    };
    upload.queue.write_buffer(
        &upload.color_space_uniform_buffer,
        0,
        bytemuck::bytes_of(&uniforms.color_space),
    );
}

pub fn apply_resize_surface_system(
    mut request: ResMut<RenderResizeRequest>,
    mut config: ResMut<RenderConfigRes>,
    mut gpu: Option<NonSendMut<RenderGpuRes>>,
    surface_host: Option<NonSend<RenderSurfaceHostRes>>,
) {
    let Some(size) = request.take() else {
        return;
    };
    if size.width == 0 || size.height == 0 {
        return;
    }
    config.width = size.width;
    config.height = size.height;

    if let Some(surface_host) = surface_host
        && let Some(host) = surface_host.0.as_ref()
    {
        host.prepare_resize(size);
    }
    if let Some(gpu) = gpu.as_deref_mut() {
        gpu.0.apply_resize(size);
    }
}

/// Upload the CPU meshes extracted by the App world into the App-owned GPU
/// mesh storage.
#[derive(SystemParam)]
pub(crate) struct RenderMeshUploadParams<'w> {
    gpu: Option<NonSend<'w, RenderGpuRes>>,
    meshes: Option<NonSendMut<'w, RenderMeshesRes>>,
    phase: Res<'w, State<AppPhase>>,
    block_effects: Res<'w, RenderBlockEffectVerticesRes>,
    playing: Res<'w, RenderPlayingBlockMeshesRes>,
    menu_preview: Res<'w, RenderMenuPreviewBlockMeshesRes>,
    editor_blocks: Res<'w, RenderEditorBlockMeshesRes>,
    spawn_marker: Res<'w, RenderEditorSpawnMarkerVerticesRes>,
    gizmo: Res<'w, RenderEditorGizmoVerticesRes>,
    trail: Res<'w, RenderTrailVerticesRes>,
    practice_checkpoints: Res<'w, RenderPracticeCheckpointVerticesRes>,
    camera_arrow: Res<'w, RenderEditorCameraArrowVerticesRes>,
    cursor: Res<'w, RenderEditorCursorVerticesRes>,
    hitbox: Res<'w, RenderEditorHitboxVerticesRes>,
    tap_indicator: Res<'w, RenderEditorTapIndicatorVerticesRes>,
    selection: Res<'w, RenderEditorSelectionOverlayRes>,
    hover: Res<'w, RenderEditorHoverOverlayRes>,
    transform_markers: Res<'w, RenderEditorTransformTriggerMarkerVerticesRes>,
}

pub(crate) fn upload_extracted_render_meshes_system(params: RenderMeshUploadParams) {
    let RenderMeshUploadParams {
        gpu,
        meshes,
        phase,
        block_effects,
        playing,
        menu_preview,
        editor_blocks,
        spawn_marker,
        gizmo,
        trail,
        practice_checkpoints,
        camera_arrow,
        cursor,
        hitbox,
        tap_indicator,
        selection,
        hover,
        transform_markers,
    } = params;
    let Some(gpu) = gpu else {
        return;
    };
    let Some(mut meshes) = meshes else {
        return;
    };
    let device = &gpu.0.device;
    let queue = &gpu.0.queue;

    meshes
        .0
        .gem_shatter_effects
        .replace_with_streaming_vertices(
            device,
            queue,
            "Gem Shatter Vertex Buffer",
            &block_effects.0,
        );
    if let Some(result) = playing.result.as_ref() {
        meshes
            .0
            .blocks_instanced
            .replace_with_batches(device, "Block Instance Buffer", result);
        meshes.0.blocks.replace_with_geometry(
            device,
            "Block Vertex Buffer",
            &result.fallback_geometry,
        );
        meshes.0.blocks_chunked.clear();
    } else if let Some(chunked) = playing.chunked.as_ref() {
        meshes
            .0
            .blocks_chunked
            .replace_with_chunked_mesh(device, "Chunked Block Buffer", chunked);
        meshes.0.blocks.clear();
        meshes.0.blocks_instanced.clear();
        meshes.0.blocks_static.clear();
        meshes.0.blocks_static_chunked.clear();
        meshes.0.blocks_static_instanced.clear();
        meshes.0.blocks_selected.clear();
    }
    if *phase.get() == AppPhase::Menu
        && let Some(chunked) = menu_preview.chunked.as_ref()
    {
        meshes.0.blocks_chunked.replace_with_chunked_mesh(
            device,
            "Menu Preview Chunked Block Buffer",
            chunked,
        );
        meshes.0.blocks.clear();
        meshes.0.blocks_instanced.clear();
        meshes.0.blocks_static.clear();
        meshes.0.blocks_static_chunked.clear();
        meshes.0.blocks_static_instanced.clear();
        meshes.0.blocks_selected.clear();
    }
    if *phase.get() == AppPhase::Editor {
        meshes.0.blocks_static_chunked.replace_with_chunked_mesh(
            device,
            "Chunked Block Static Buffer",
            &editor_blocks.static_mesh,
        );
        meshes.0.blocks_selected.replace_with_geometry(
            device,
            "Block Selected Vertex Buffer",
            &editor_blocks.selected_geometry,
        );
        meshes.0.blocks_static.clear();
        meshes.0.blocks_static_instanced.clear();
        meshes.0.blocks.clear();
        meshes.0.blocks_chunked.clear();
    }

    meshes.0.spawn_marker.replace_with_streaming_vertices(
        device,
        queue,
        "Spawn Marker Vertex Buffer",
        &spawn_marker.0,
    );
    meshes.0.editor_gizmo.replace_with_streaming_vertices(
        device,
        queue,
        "Editor Gizmo Vertex Buffer",
        &gizmo.0,
    );
    meshes.0.trail.write_streaming_vertices(queue, &trail.0);
    meshes
        .0
        .practice_checkpoints
        .replace_with_streaming_vertices(
            device,
            queue,
            "Practice Checkpoint Flag Vertex Buffer",
            &practice_checkpoints.0,
        );
    meshes.0.camera_arrow.replace_with_streaming_vertices(
        device,
        queue,
        "Camera Arrow Vertex Buffer",
        &camera_arrow.0,
    );
    meshes.0.editor_cursor.replace_with_streaming_vertices(
        device,
        queue,
        "Editor Cursor Vertex Buffer",
        &cursor.0,
    );
    meshes
        .0
        .editor_hitbox_visualization
        .replace_with_streaming_vertices(
            device,
            queue,
            "Editor Hitbox Visualization Vertex Buffer",
            &hitbox.0,
        );
    meshes.0.tap_indicators.replace_with_streaming_vertices(
        device,
        queue,
        "Tap Indicator Vertex Buffer",
        &tap_indicator.0,
    );
    meshes
        .0
        .editor_selection_stencil
        .replace_with_streaming_vertices(
            device,
            queue,
            "Editor Selection Stencil Vertex Buffer",
            &selection.stencil_vertices,
        );
    meshes
        .0
        .editor_selection_outline
        .replace_with_streaming_vertices(
            device,
            queue,
            "Editor Selection Outline Vertex Buffer",
            &selection.outline_vertices,
        );
    meshes.0.editor_selection_outline_instances = selection.instances.clone();
    meshes
        .0
        .editor_hover_stencil
        .replace_with_streaming_vertices(
            device,
            queue,
            "Editor Hover Stencil Vertex Buffer",
            &hover.stencil_vertices,
        );
    meshes
        .0
        .editor_hover_outline
        .replace_with_streaming_vertices(
            device,
            queue,
            "Editor Hover Outline Vertex Buffer",
            &hover.outline_vertices,
        );
    meshes
        .0
        .transform_trigger_markers
        .replace_with_streaming_vertices(
            device,
            queue,
            "Transform Trigger Marker Vertex Buffer",
            &transform_markers.0,
        );
}

/// Add the App-world render bookkeeping systems to a [`Schedule`].
///
/// The schedule itself is added by the
/// [`super::super::plugins::register_state_schedule`] helper
/// or by the Phase 7 `App` work; this function only adds the systems
/// that populate the POD render resources.
///
/// Callers are expected to add a `RenderSet::Present` system set
/// afterwards (e.g. `add_command_systems` chains an `add_systems` for
/// the present boundary).
pub fn add_state_render_systems(schedule: &mut Schedule) {
    schedule.add_systems((
        update_render_frame_input_system.after(crate::state::async_bridge::ApplyAsyncResultsSet),
        update_color_space_uniform_system
            .after(crate::state::frame_time::advance_frame_time_system),
        upload_color_space_uniform_system
            .after(update_color_space_uniform_system)
            .after(apply_resize_surface_system)
            .in_set(RenderSet::Present),
        apply_resize_surface_system
            .after(crate::events::auth::handle_resize_surface)
            .in_set(RenderSet::Present),
    ));
}

#[cfg(test)]
mod tests {
    use bevy_ecs::world::World;
    use bevy_state::prelude::State;
    use eg_types::{AppPhase, EditorMode, LevelObject, PhysicalSize};

    use super::*;
    use crate::state::{
        EditorCameraRes, EditorInputRes, SessionStateRes,
        render::resources::render_pod_resources_present,
    };

    #[test]
    fn register_render_pod_resources_inserts_defaults() {
        let mut world = World::new();
        register_render_pod_resources(&mut world);
        assert!(world.contains_resource::<RenderUniformsRes>());
        assert!(world.contains_resource::<RenderFrameInputRes>());
        assert!(world.contains_resource::<RenderConfigRes>());
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
        world.init_resource::<State<AppPhase>>();
        world.init_resource::<EditorInputRes>();
        world.init_resource::<SessionStateRes>();
        world.init_resource::<EditorCameraRes>();
        world.init_resource::<EditorTimelineRes>();
        world.init_resource::<RenderFrameInputRes>();
        world.init_resource::<RenderConfigRes>();
        world.insert_resource(State::new(AppPhase::Editor));
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
    /// is inserted by `AppBootstrap` because it has no cheap default.
    #[test]
    fn register_inserts_phase4c_heavy_resources() {
        let mut world = World::new();
        register_render_pod_resources(&mut world);
        assert!(world.contains_non_send::<RenderMeshesRes>());
        assert!(world.contains_non_send::<RenderSurfaceHostRes>());
        assert!(!world.contains_non_send::<RenderGpuRes>());
    }

    /// Phase 4e: the present system reads the authoritative render
    /// resources. Bare test worlds do not have a real GPU, so the GPU
    /// resource is optional until constructed by `AppBootstrap`.
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

        assert_eq!(
            *world.resource::<RenderConfigRes>(),
            RenderConfigRes {
                width: 320,
                height: 240,
                apply_gamma_correction: false,
            }
        );
    }
    fn extract_selection_overlays(
        input: eg_types::EditorInputState,
        objects: Vec<LevelObject>,
    ) -> (RenderEditorSelectionOverlayRes, RenderEditorHoverOverlayRes) {
        let mut world = World::new();
        world.insert_resource(State::new(AppPhase::Editor));
        world.insert_resource(EditorInputRes(input));
        crate::state::editor_objects::replace_editor_objects(&mut world, objects);
        world.init_resource::<RenderEditorSelectionOverlayRes>();
        world.init_resource::<RenderEditorHoverOverlayRes>();

        let mut schedule = Schedule::default();
        schedule.add_systems(extract_editor_selection_overlays_system);
        schedule.run(&mut world);

        (
            world.resource::<RenderEditorSelectionOverlayRes>().clone(),
            world.resource::<RenderEditorHoverOverlayRes>().clone(),
        )
    }

    #[test]
    fn selection_overlay_keeps_hover_outline_before_marquee_activation() {
        let input = eg_types::EditorInputState {
            mode: EditorMode::Select,
            hovered_block_index: Some(0),
            left_mouse_down: true,
            marquee_start_screen: Some([100.0, 100.0]),
            marquee_current_screen: Some([101.0, 101.0]),
            ..Default::default()
        };
        let (_, hover) = extract_selection_overlays(
            input,
            vec![
                LevelObject::default(),
                LevelObject {
                    position: [5.0, 0.0, 0.0],
                    ..LevelObject::default()
                },
            ],
        );

        assert_eq!(hover.outline_vertices.len(), 36);
        assert_eq!(hover.stencil_vertices.len(), 36);
    }

    #[test]
    fn selection_overlay_builds_one_instance_per_selected_object() {
        let input = eg_types::EditorInputState {
            mode: EditorMode::Select,
            selected_block_index: Some(0),
            selected_block_indices: vec![0, 1],
            ..Default::default()
        };
        let (selection, _) = extract_selection_overlays(
            input,
            vec![
                LevelObject::default(),
                LevelObject {
                    position: [2.0, 0.0, 0.0],
                    ..LevelObject::default()
                },
            ],
        );

        assert_eq!(selection.instances.len(), 2);
        assert_eq!(selection.outline_vertices.len(), 72);
        assert_eq!(selection.stencil_vertices.len(), 72);
    }

    #[test]
    fn selection_overlay_uses_group_bounds_for_large_selections() {
        let objects = (0..701)
            .map(|index| LevelObject {
                position: [index as f32, 0.0, 0.0],
                ..LevelObject::default()
            })
            .collect::<Vec<_>>();
        let input = eg_types::EditorInputState {
            mode: EditorMode::Select,
            selected_block_index: Some(0),
            selected_block_indices: (0..701).collect(),
            ..Default::default()
        };
        let (selection, _) = extract_selection_overlays(input, objects);

        assert_eq!(selection.instances.len(), 1);
        assert_eq!(selection.outline_vertices.len(), 36);
        assert_eq!(selection.stencil_vertices.len(), 36);
    }
    #[test]
    fn editor_cursor_extraction_hides_cursor_over_selected_tap() {
        let mut app = crate::state::test_support::TestApp::minimal();
        app.world_mut()
            .insert_resource(State::new(AppPhase::Editor));
        {
            let mut input = app.world_mut().resource_mut::<EditorInputRes>();
            input.0.mode = EditorMode::Tapping;
            input.0.cursor = [1.0, 0.0, 1.0];
        }
        app.world_mut()
            .resource_mut::<EditorRuntimeRes>()
            .0
            .interaction
            .hovered_tap_index = Some(0);
        app.world_mut()
            .resource_mut::<EditorTimelineRes>()
            .0
            .taps
            .tap_indicator_positions = vec![[1.0, 0.0, 1.0]];

        let mut schedule = Schedule::default();
        schedule.add_systems(extract_editor_cursor_vertices_system);
        schedule.run(app.world_mut());
        assert!(
            app.world()
                .resource::<RenderEditorCursorVerticesRes>()
                .0
                .is_empty()
        );

        app.world_mut()
            .resource_mut::<EditorRuntimeRes>()
            .0
            .interaction
            .hovered_tap_index = None;
        app.world_mut().resource_mut::<EditorInputRes>().0.cursor = [2.0, 0.0, 2.0];
        schedule.run(app.world_mut());
        assert!(
            !app.world()
                .resource::<RenderEditorCursorVerticesRes>()
                .0
                .is_empty()
        );
    }

    #[test]
    fn editor_tap_indicator_extraction_respects_phase_and_mode() {
        let mut app = crate::state::test_support::TestApp::minimal();
        app.world_mut()
            .insert_resource(State::new(AppPhase::Editor));
        {
            let mut input = app.world_mut().resource_mut::<EditorInputRes>();
            input.0.mode = EditorMode::Tapping;
        }
        {
            let mut timeline = app.world_mut().resource_mut::<EditorTimelineRes>();
            timeline.0.taps.tap_times = vec![0.5];
            timeline.0.taps.tap_indicator_positions = vec![[1.0, 0.0, 1.0]];
        }

        let mut schedule = Schedule::default();
        schedule.add_systems(extract_editor_tap_indicator_vertices_system);
        schedule.run(app.world_mut());
        assert!(
            !app.world()
                .resource::<RenderEditorTapIndicatorVerticesRes>()
                .0
                .is_empty()
        );

        app.world_mut().resource_mut::<EditorInputRes>().0.mode = EditorMode::Timing;
        schedule.run(app.world_mut());
        assert!(
            app.world()
                .resource::<RenderEditorTapIndicatorVerticesRes>()
                .0
                .is_empty()
        );
    }

    #[test]
    fn editor_transform_marker_extraction_is_app_owned_and_phase_gated() {
        let mut app = crate::state::test_support::TestApp::minimal();
        app.world_mut()
            .insert_resource(State::new(AppPhase::Editor));
        crate::state::editor_objects::replace_editor_objects(
            app.world_mut(),
            vec![
                LevelObject {
                    id: "object_0_0_0".to_string(),
                    ..LevelObject::new("core/stone")
                },
                LevelObject {
                    position: [2.0, 0.0, 0.0],
                    trigger: Some(eg_types::TimedTrigger {
                        time_seconds: 0.0,
                        duration_seconds: 0.0,
                        easing: eg_types::EasingDirection::Linear,
                        target: eg_types::TriggerTarget::Objects {
                            object_ids: vec!["object_0_0_0".to_string()],
                        },
                        transform: true,
                        camera: None,
                    }),
                    ..LevelObject::new(eg_types::TRANSFORM_TRIGGER_BLOCK_ID)
                },
            ],
        );

        let mut schedule = Schedule::default();
        schedule.add_systems(extract_editor_transform_trigger_marker_vertices_system);
        schedule.run(app.world_mut());
        assert!(
            !app.world()
                .resource::<RenderEditorTransformTriggerMarkerVerticesRes>()
                .0
                .is_empty()
        );

        app.world_mut().insert_resource(State::new(AppPhase::Menu));
        schedule.run(app.world_mut());
        assert!(
            app.world()
                .resource::<RenderEditorTransformTriggerMarkerVerticesRes>()
                .0
                .is_empty()
        );
    }

    #[test]
    fn editor_block_mesh_extraction_reads_app_objects_and_selection() {
        let mut app = crate::state::test_support::TestApp::minimal();
        app.world_mut()
            .insert_resource(State::new(AppPhase::Editor));
        app.world_mut().resource_mut::<EditorInputRes>().0.mode = EditorMode::Place;
        {
            let mut input = app.world_mut().resource_mut::<EditorInputRes>();
            input.0.selected_block_index = Some(0);
            input.0.selected_block_indices = vec![0];
        }
        crate::state::editor_objects::replace_editor_objects(
            app.world_mut(),
            vec![
                LevelObject::new("core/stone"),
                LevelObject {
                    position: [2.0, 0.0, 0.0],
                    ..LevelObject::new("core/grass")
                },
            ],
        );

        let mut schedule = Schedule::default();
        schedule.add_systems(extract_editor_block_meshes_system);
        schedule.run(app.world_mut());

        let meshes = app.world().resource::<RenderEditorBlockMeshesRes>();
        assert!(!meshes.static_mesh.chunks.is_empty());
        assert!(!meshes.selected_geometry.is_empty());
    }
    #[test]
    fn editor_render_objects_apply_transform_triggers_without_mutating_authored_objects() {
        let authored = LevelObject {
            id: "object_0_0_0".to_string(),
            ..LevelObject::default()
        };
        let trigger = LevelObject {
            position: [2.0, 0.0, 0.0],
            trigger: Some(eg_types::TimedTrigger {
                time_seconds: 0.0,
                duration_seconds: 0.0,
                easing: eg_types::EasingDirection::Linear,
                target: eg_types::TriggerTarget::Objects {
                    object_ids: vec![authored.id.clone()],
                },
                transform: true,
                camera: None,
            }),
            ..LevelObject::new(eg_types::TRANSFORM_TRIGGER_BLOCK_ID)
        };
        let objects = vec![authored.clone()];
        let mut timeline = EditorTimelineRes::default();
        timeline.0.clock.time_seconds = 0.5;

        let rendered = editor_objects_for_render(&objects, &timeline, &[trigger]);

        assert_eq!(rendered[0].position, [2.0, 0.0, 0.0]);
        assert_eq!(objects[0].position, authored.position);
    }
}
