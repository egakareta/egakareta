/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
mod async_bridge;
mod audio_state;
mod auth_state;
mod editor_actions;
mod editor_camera;
pub mod editor_command;
mod editor_config_state;
mod editor_interaction;
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
mod sets;
mod state_helpers;
mod state_resources;
mod transitions;
mod visual_effects;

use crate::events as state_events;
pub use state_events::{CommandHandlerSet, add_command_systems, register_command_events};

#[cfg(test)]
mod editor_domain_tests;
#[cfg(test)]
mod editor_snap_tests;
#[cfg(test)]
mod tests;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;

pub const TAP_CLICK_SCREEN_EPSILON_PIXELS: f64 = 0.5;

pub(crate) use async_bridge::apply_imported_level_to_world;
pub use audio_state::{
    AudioBackendRes, AudioBackendSystemParam, AudioState, EditorAudioCacheRes,
    RuntimeAudioPreloadRes, RuntimeAudioPreloadSenderRes, WaveformLoadSenderRes,
};
pub(crate) use audio_state::{
    load_waveform_for_level_resources, start_audio_source_at_seconds_for_resources,
    with_audio_backend,
};

pub use editor_camera::EditorCameraState;
pub use editor_camera::editor_camera_preview_for_objects;
pub(crate) use editor_camera::{CameraViewSample, evaluate_camera_track_for_objects};
pub use editor_config_state::EditorConfigState;
use editor_interaction::EditorPickSpatialIndex;
pub use editor_interaction::{
    EditorBlockDrag, EditorClipboard, EditorDragBlockStart, EditorGizmoDrag, EditorHistorySnapshot,
    EditorInteractionState, EditorPendingTapClick,
};
#[cfg(test)]
pub(crate) use editor_timeline::EditorTimelineSnapshot;
pub use editor_timeline::EditorTimelineState;
pub use editor_timing::EditorTimingState;
pub use editor_triggers::EditorTriggerState;
pub(crate) use editor_triggers::camera_trigger_object_pose;
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
pub use runtime::{BlockVisualEffect, EditorDirtyFlags, EditorRuntimeState, FrameRuntimeState};
pub use sets::StateSet;
pub use state_resources::{
    AudioImportSenderRes, AuthOpenSignupRequest, AuthSignOutRequest, AuthStateRes,
    AuthSubmitSignInRequest, BlockEffectsRes, BuiltinLevelCatalogRes, EditorCameraRes,
    EditorConfigRes, EditorHistoryRes, EditorInputRes, EditorObjectsRes, EditorRuntimeRes,
    EditorSpawnRes, EditorTimelineRes, EditorTimingRes, EditorTriggerRes, FrameTimeRes,
    GameplayResetRes, GameplayResetSeed, LevelExportServiceRes, LevelImportSenderRes,
    MenuPreviewObjectsRes, MenuStateRes, PerfFrameCopyRequest, PerfRes, SessionStateRes,
};

use eg_game::GameCheckpointState;
use eg_mesh::MeshGeometry;
use eg_types::{EditorInputState, LevelObject, SpawnMetadata};

#[derive(Clone)]
pub struct PracticeCheckpoint {
    pub gameplay: GameCheckpointState,
    pub trigger_base_objects: Option<Vec<LevelObject>>,
}
/// Bundles all editor-related state into a single subsystem.
/// Separates editor concern from the top-level application state.
#[derive(Default)]
pub struct EditorSubsystem {
    pub input: EditorInputState,
    pub config: EditorConfigState,
    pub objects: Vec<LevelObject>,
    pub spawn: SpawnMetadata,
    pub camera: EditorCameraState,
    pub triggers: EditorTriggerState,
    pub timeline: EditorTimelineState,
    pub runtime: EditorRuntimeState,
    pub timing: EditorTimingState,
    pub selected_mask_cache: Option<Vec<bool>>,
    pub block_static_vertex_cache: MeshGeometry,
    pub block_static_vertex_cache_complete_len: Option<usize>,
    pub(crate) pick_spatial_index: EditorPickSpatialIndex,
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
