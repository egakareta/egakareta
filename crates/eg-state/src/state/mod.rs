/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
macro_rules! editor_context_methods {
    (@collect [$($context:item)*]) => {
        impl $crate::state::EditorInteractionContext<'_> {
            $($context)*
        }
    };
    (@collect [$($context:item)*] #[cfg(test)] $test:item $($rest:tt)*) => {
        editor_context_methods!(@collect [$($context)*] $($rest)*);
    };
    (@collect [$($context:item)*] $item:item $($rest:tt)*) => {
        editor_context_methods!(@collect [$($context)* $item] $($rest)*);
    };
    ($($tokens:tt)*) => {
        editor_context_methods!(@collect [] $($tokens)*);
    };
}
pub(crate) use editor_context_methods;

mod async_bridge;
mod audio_state;
mod auth_state;
mod editor_actions;
mod editor_camera;
pub mod editor_command;
mod editor_config_state;
mod editor_interaction;
mod editor_objects;
pub(crate) use editor_objects::{
    EditorAuthoredObject, EditorObjectOrder, EditorObjectRevision, collect_editor_object_data,
    replace_editor_objects_commands, spawn_editor_object_commands,
};
pub use editor_objects::{collect_editor_objects_from_world, replace_editor_objects};
mod editor_playback;
mod editor_scene;
mod editor_state;
mod editor_timeline;
mod editor_timeline_cache;
mod editor_timing;
mod editor_trail;
mod editor_triggers;
mod frame_time;
pub(crate) mod gameplay_world;
mod history;
mod input_bridge;
mod level_management;
mod lifecycle;
mod menu_preview;
mod perf;
mod plugins;
mod render;
mod runtime;
mod state_helpers;
mod state_resources;
mod transitions;
mod visual_effects;

pub use state_events::{CommandHandlerSet, add_command_systems, register_command_events};

use crate::events as state_events;

#[cfg(test)]
mod editor_domain_tests;
#[cfg(test)]
mod editor_snap_tests;
#[cfg(test)]
mod tests;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

pub const TAP_CLICK_SCREEN_EPSILON_PIXELS: f64 = 0.5;

use std::ops::{Deref, DerefMut};

pub(crate) use async_bridge::apply_imported_level_to_world;
pub use audio_state::{
    AppSound, AudioSceneParam, AudioSceneRes, WAVEFORM_WINDOW, WaveformLoadMessage,
    WaveformLoadSenderRes, WaveformResult, available_host_labels, canonical_backend_name,
    decode_audio_to_waveform_streaming, ensure_audio_output, music_is_playing, music_time_seconds,
    pause_music, play_sfx, resume_music, runtime_asset_source_key, set_preferred_backend_name,
    start_music_builtin, start_music_with_bytes, start_waveform_loading, stop_music,
    warmup_music_builtin, warmup_music_with_bytes,
};
pub(crate) use audio_state::{
    AudioImportChannel, WaveformLoadChannel, load_waveform_for_level_resources,
    start_audio_source_at_seconds_for_resources, with_audio_scene,
};
#[cfg(test)]
use bevy_ecs::prelude::ResMut;
#[cfg(test)]
use bevy_ecs::system::SystemState;
pub(crate) use editor_camera::{CameraViewSample, evaluate_camera_track_for_objects};
pub use editor_camera::{EditorCameraState, editor_camera_preview_for_objects};
pub use editor_config_state::EditorConfigState;
pub use editor_interaction::{
    EditorBlockDrag, EditorClipboard, EditorDragBlockStart, EditorGizmoDrag, EditorHistorySnapshot,
    EditorInteractionState, EditorPendingTapClick,
};
pub(crate) use editor_interaction::{EditorHistoryObjectChange, EditorPickSpatialIndex};
#[cfg(test)]
pub(crate) use editor_timeline::EditorTimelineSnapshot;
pub use editor_timeline::EditorTimelineState;
pub use editor_timing::EditorTimingState;
pub use editor_triggers::EditorTriggerState;
pub(crate) use editor_triggers::{
    camera_trigger_object_pose, sync_camera_follow_trigger_object_pose,
};
use eg_game::GameCheckpointState;
use eg_mesh::MeshGeometry;
use eg_types::{EditorInputState, LevelObject, SpawnMetadata};
pub use history::EditorHistoryState;
pub use input_bridge::{
    PlatformInputQueue, enqueue_platform_input, register_platform_input_resources,
};
pub use lifecycle::AppBootstrap;
pub use perf::PerfState;
pub use plugins::{
    EditorPlugin, GameplayPlugin, MenuPlugin, SessionPlugin, StatePlugins, register_state_schedule,
    run_state_schedule_system, setup_state_app,
};
pub use render::resources::{
    RenderBlockEffectVerticesRes, RenderConfigRes, RenderEditorBlockMeshesRes,
    RenderEditorCameraArrowVerticesRes, RenderEditorCursorVerticesRes,
    RenderEditorGizmoVerticesRes, RenderEditorHitboxVerticesRes, RenderEditorHoverOverlayRes,
    RenderEditorSelectionOverlayRes, RenderEditorSpawnMarkerVerticesRes,
    RenderEditorTapIndicatorVerticesRes, RenderEditorTransformTriggerMarkerVerticesRes,
    RenderEguiRes, RenderFrameInputRes, RenderGpuRes, RenderMenuPreviewBlockMeshesRes,
    RenderMeshesRes, RenderPlayingBlockMeshesRes, RenderPracticeCheckpointVerticesRes,
    RenderResizeRequest, RenderSurfaceHostRes, RenderTrailVerticesRes, RenderUniformUploadRes,
    RenderUniformsRes,
};
pub use runtime::{BlockVisualEffect, EditorRuntimeState};
pub(crate) use state_resources::EditorPointerCache;
pub use state_resources::{
    AudioImportSenderRes, AuthOpenSignupRequest, AuthSignOutRequest, AuthStateRes,
    AuthSubmitSignInRequest, BlockEffectsRes, BuiltinLevelCatalogRes, EditorCameraRes,
    EditorConfigRes, EditorHistoryRes, EditorInputRes, EditorRuntimeRes, EditorSimulationInputsRes,
    EditorSpawnRes, EditorTimelineRes, EditorTimingRes, EditorTriggerRes, FrameTimeRes,
    GameplayResetRes, GameplayResetSeed, LevelExportServiceRes, LevelImportSenderRes,
    MenuPreviewObjectsRes, MenuStateRes, PerfFrameCopyRequest, PerfRes, SessionStateRes,
};

#[derive(Clone)]
pub struct PracticeCheckpoint {
    pub gameplay: GameCheckpointState,
    pub trigger_base_objects: Option<Vec<LevelObject>>,
}
pub(crate) struct EditorObjectsBorrow<'a>(pub(crate) &'a mut Vec<LevelObject>);

impl Deref for EditorObjectsBorrow<'_> {
    type Target = Vec<LevelObject>;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl DerefMut for EditorObjectsBorrow<'_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0
    }
}

/// Borrowing editor interaction view used by systems that update several
/// App-world resources atomically. It owns no editor state.
pub(crate) struct EditorInteractionContext<'a> {
    pub input: &'a mut EditorInputState,
    pub config: &'a mut EditorConfigState,
    pub objects: EditorObjectsBorrow<'a>,
    pub spawn: &'a mut SpawnMetadata,
    pub camera: &'a mut EditorCameraState,
    pub triggers: &'a mut EditorTriggerState,
    pub timeline: &'a mut EditorTimelineState,
    pub runtime: &'a mut EditorRuntimeState,
    pub timing: &'a mut EditorTimingState,
    pub history: &'a mut EditorHistoryState,
    pub selected_mask_cache: Option<Vec<bool>>,
    pub block_static_vertex_cache: MeshGeometry,
    pub block_static_vertex_cache_complete_len: Option<usize>,
    pub(crate) pick_spatial_index: &'a mut EditorPickSpatialIndex,
}

#[cfg(test)]
pub(crate) fn with_editor_interaction_context<R>(
    world: &mut bevy_ecs::world::World,
    f: impl FnOnce(&mut EditorInteractionContext<'_>) -> R,
) -> R {
    let mut objects = editor_objects::collect_editor_objects(world);
    type EditorInteractionSystemState<'w> = SystemState<(
        ResMut<'w, EditorInputRes>,
        ResMut<'w, EditorConfigRes>,
        ResMut<'w, EditorCameraRes>,
        ResMut<'w, EditorTriggerRes>,
        ResMut<'w, EditorTimelineRes>,
        ResMut<'w, EditorRuntimeRes>,
        ResMut<'w, EditorTimingRes>,
        ResMut<'w, EditorSpawnRes>,
        ResMut<'w, EditorHistoryRes>,
    )>;
    let mut system_state: EditorInteractionSystemState<'_> = SystemState::new(world);
    let result = {
        let (
            mut input,
            mut config,
            mut camera,
            mut triggers,
            mut timeline,
            mut runtime,
            mut timing,
            mut spawn,
            mut history,
        ) = system_state
            .get_mut(world)
            .expect("editor test context resources must be registered");
        let mut pick_spatial_index = EditorPickSpatialIndex::default();
        f(&mut EditorInteractionContext {
            input: &mut input.0,
            config: &mut config.0,
            objects: EditorObjectsBorrow(&mut objects),
            spawn: &mut spawn.0,
            camera: &mut camera.0,
            triggers: &mut triggers.0,
            timeline: &mut timeline.0,
            runtime: &mut runtime.0,
            timing: &mut timing.0,
            history: &mut history.0,
            selected_mask_cache: None,
            block_static_vertex_cache: Default::default(),
            block_static_vertex_cache_complete_len: None,
            pick_spatial_index: &mut pick_spatial_index,
        })
    };
    system_state.apply(world);
    replace_editor_objects(world, objects);
    result
}

/// Schedule label for the per-frame state update. Runtime runs this
/// after App update and before the remaining compatibility update/render
/// path.
#[derive(bevy_ecs::schedule::ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StateSchedule;

/// Schedule label for the render extraction pass that runs after the
/// App-world gameplay tick and refreshes render POD resources for present.
#[derive(bevy_ecs::schedule::ScheduleLabel, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderExtractSchedule;

pub struct RenderBootstrapResources {
    gpu: eg_render::GpuContext,
    meshes: eg_render::SceneMeshes,
    surface_host: Option<eg_platform::state_host::SurfaceHost>,
}

impl RenderBootstrapResources {
    /// Move the platform render resources into their App-owned wrappers.
    pub fn into_parts(
        self,
    ) -> (
        eg_render::GpuContext,
        eg_render::SceneMeshes,
        Option<eg_platform::state_host::SurfaceHost>,
    ) {
        (self.gpu, self.meshes, self.surface_host)
    }
}
