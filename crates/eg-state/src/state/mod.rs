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
mod editor_config_state;
mod editor_interaction;
#[cfg(test)]
pub(crate) use editor_interaction::{editor_camera_view_proj, project_world_to_screen};
mod editor_objects;
pub(crate) use editor_objects::{
    EditorAuthoredObject, EditorObjectOrder, collect_editor_object_data,
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
mod file_import_tasks;
mod frame_time;
pub(crate) mod gameplay_world;
mod history;
mod input_bridge;
mod level_management;
mod menu_preview;
mod perf;
mod persistence_tasks;
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

#[cfg(test)]
pub mod test_support;

pub const TAP_CLICK_SCREEN_EPSILON_PIXELS: f64 = 0.5;

use std::ops::{Deref, DerefMut};

pub(crate) use async_bridge::apply_imported_level_to_world;
pub use audio_state::{
    AudioBackendRes, AudioBackendSystemParam, EditorAudioCacheRes, RuntimeAudioPreloadRes,
};
pub(crate) use audio_state::{
    poll_waveform_load_tasks_system, queue_runtime_audio_preload_task,
    queue_runtime_audio_preload_task_world, queue_waveform_load_task,
    queue_waveform_load_task_world, start_audio_source_at_seconds_for_resources,
    with_audio_backend,
};
pub(crate) use editor_camera::{
    CameraViewSample, capture_current_camera_trigger, evaluate_camera_track_for_objects,
};
pub use editor_camera::{EditorCameraState, editor_camera_preview_for_objects};
pub use editor_config_state::EditorConfigState;
use editor_interaction::EditorPickSpatialIndex;
pub use editor_interaction::{
    EditorBlockDrag, EditorClipboard, EditorDragBlockStart, EditorGizmoDrag, EditorHistorySnapshot,
    EditorInteractionState, EditorPendingTapClick,
};
#[cfg(test)]
pub(crate) use editor_state::effective_snap_enabled;
#[cfg(test)]
pub(crate) use editor_timeline::EditorTimelineSnapshot;
pub use editor_timeline::EditorTimelineState;
pub use editor_timing::EditorTimingState;
pub use editor_triggers::EditorTriggerState;
pub(crate) use editor_triggers::sync_camera_follow_trigger_object_pose;
use eg_game::GameCheckpointState;
use eg_mesh::MeshGeometry;
use eg_types::{EditorInputState, LevelObject, SpawnMetadata};
pub(crate) use file_import_tasks::{
    AudioImportTask, LevelImportTask, spawn_audio_import_task, spawn_level_import_task,
};
pub use history::EditorHistoryState;
pub(crate) use history::record_history_snapshot;
pub use input_bridge::{
    BevyInputBridgeEnabled, BevyInputBridgeSet, BevyPointerPositionRes, BevyTouchRoutingRes,
    BevyUiInputCaptureRes, register_platform_input_resources,
};
pub use perf::PerfState;
#[cfg(not(test))]
pub(crate) use persistence_tasks::queue_app_settings_persist_world;
pub(crate) use persistence_tasks::{
    poll_persistence_tasks_system, queue_app_settings_persist, queue_pending_level_exports_system,
    queue_waveform_persist_world,
};
pub use plugins::{
    AudioPlugin, EditorPlugin, GameplayPlugin, MenuPlugin, SessionPlugin, StateDomainPlugins,
    StatePlugins, setup_state_app,
};
pub use render::{
    bevy_material::{
        BlockMaterial, BlockMaterialAssetsPlugin, BlockMaterialPlugin,
        build_block_texture_array_image,
    },
    bevy_presentation::{
        BevyBlockRenderPlugin, BlockEffectOverlayPresentation, BlockPresentationCamera,
        BlockPresentationCameraPlugin, BlockPresentationEnvironmentPlugin,
        BlockPresentationMaterial, BlockPresentationPlugin, CameraArrowOverlayPresentation,
        CursorOverlayPresentation, EditorBlockChunkPresentation, GizmoOverlayPresentation,
        HitboxOverlayPresentation, HoverOutlinePresentation, HoverStencilPresentation,
        MenuPreviewBlockChunkPresentation, OverlayPresentationPlugin,
        PlayingBlockChunkPresentation, PlayingDynamicBlockPresentation,
        PracticeCheckpointOverlayPresentation, SelectionOutlinePresentation,
        SelectionStencilPresentation, SpawnMarkerOverlayPresentation,
        TapIndicatorOverlayPresentation, TrailOverlayPresentation,
        TransformTriggerMarkerPresentation,
    },
    resources::{
        RenderBlockEffectVerticesRes, RenderCameraPoseRes, RenderConfigRes,
        RenderEditorBlockMeshesRes, RenderEditorCameraArrowVerticesRes,
        RenderEditorCursorVerticesRes, RenderEditorGizmoVerticesRes, RenderEditorHitboxVerticesRes,
        RenderEditorHoverOverlayRes, RenderEditorSelectionOverlayRes,
        RenderEditorSpawnMarkerVerticesRes, RenderEditorTapIndicatorVerticesRes,
        RenderEditorTransformTriggerMarkerVerticesRes, RenderFrameInputRes,
        RenderMenuPreviewBlockMeshesRes, RenderPlayingBlockMeshesRes,
        RenderPracticeCheckpointVerticesRes, RenderResizeRequest, RenderTrailVerticesRes,
        RenderUniformsRes,
    },
};
pub use runtime::{BlockVisualEffect, EditorRuntimeState, FrameRuntimeState};
pub use state_resources::{
    AuthOpenSignupRequest, AuthSignOutRequest, AuthStateRes, AuthSubmitSignInRequest,
    BlockEffectsRes, BuiltinLevelCatalogRes, EditorCameraRes, EditorConfigRes, EditorHistoryRes,
    EditorInputRes, EditorRuntimeRes, EditorSpawnRes, EditorTimelineRes, EditorTimingRes,
    EditorTriggerRes, FrameTimeRes, GameplayResetRes, GameplayResetSeed, LevelExportServiceRes,
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
    pub selected_mask_cache: Option<Vec<bool>>,
    pub block_static_vertex_cache: MeshGeometry,
    pub block_static_vertex_cache_complete_len: Option<usize>,
    pub(crate) pick_spatial_index: EditorPickSpatialIndex,
}
