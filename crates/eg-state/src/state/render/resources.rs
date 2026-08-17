/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! ECS resources for the rendering extraction plugin (Phase 4a).
//!
//! Rendering extraction resources owned by the application world. Bevy's
//! render sub-app owns GPU, surface, and egui renderer state; these resources
//! are compact domain snapshots used to derive retained presentation entities.

use bevy::ecs::prelude::*;
use eg_types::{CameraUniform, ColorSpaceUniform, LineUniform};

/// Per-frame presentation state derived from App-world resources.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderFrameInput {
    pub phase: eg_types::AppPhase,
    pub editor_mode: eg_types::EditorMode,
    pub game_over: bool,
    pub sky_color: [f32; 3],
    pub grid_center: [f32; 2],
    pub apply_gamma_correction: bool,
}

/// A pair of mesh ranges used by a selected-object outline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorOutlineInstance {
    pub mask_vertices: std::ops::Range<u32>,
    pub outline_vertices: std::ops::Range<u32>,
}

/// App-owned render configuration needed by extraction without borrowing the
/// full surface/GPU context.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderConfigRes {
    pub width: u32,
    pub height: u32,
    pub apply_gamma_correction: bool,
}

impl RenderConfigRes {}

impl Default for RenderConfigRes {
    fn default() -> Self {
        Self {
            width: 1,
            height: 1,
            apply_gamma_correction: false,
        }
    }
}

/// Bundle of the three `Copy` uniforms that drive the per-frame
/// shader constants (`CameraUniform`, `LineUniform`,
/// `ColorSpaceUniform`).
///
/// Mirrored from the legacy frame-update uniform computation during
/// the [`RenderExtractSet`](super::plugin) phase. The mirror
/// is a free operation: the uniform values are tiny POD types
/// (a `Mat4` and a handful of `f32`s), and the actual GPU buffers
/// stay in Bevy's render world. Bevy systems can use this resource to read
/// the per-frame values without touching renderer internals.
#[allow(dead_code)]
#[derive(Resource, Clone, Copy)]
pub struct RenderUniformsRes {
    /// Current camera (view-projection) uniform.
    pub camera: CameraUniform,
    /// Current line uniform (offset + rotation).
    pub line: LineUniform,
    /// Current color-space uniform (gamma + time + viewport).
    pub color_space: ColorSpaceUniform,
}

impl Default for RenderUniformsRes {
    fn default() -> Self {
        Self {
            camera: CameraUniform {
                view_proj: glam::Mat4::IDENTITY.to_cols_array_2d(),
                camera_position: [0.0, 0.0, 0.0, 0.0],
            },
            line: LineUniform {
                offset: [0.0, 0.0],
                rotation: 0.0,
                _pad: 0.0,
            },
            color_space: ColorSpaceUniform {
                apply_gamma_correction: 0.0,
                time_seconds: 0.0,
                viewport_size: [1.0, 1.0],
            },
        }
    }
}

/// Authoritative camera pose extracted from the App world for Bevy scene
/// presentation.
///
/// This stays separate from [`RenderUniformsRes`] so retained Bevy camera
/// entities do not need to reconstruct a transform from a GPU uniform.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct RenderCameraPoseRes {
    /// World-space camera position.
    pub eye: [f32; 3],
    /// World-space point the camera faces.
    pub target: [f32; 3],
}

impl Default for RenderCameraPoseRes {
    fn default() -> Self {
        Self {
            eye: [0.0, 20.0, -20.0],
            target: [0.0, 0.0, 0.0],
        }
    }
}

/// Mirrored [`RenderFrameInput`] for the current frame.
///
/// The frame input is computed at the top of `State::render_to_view`
/// (phase, editor mode, sky color, grid center, gamma toggle). It
/// drives the clear color, the grid uniform, and several `should_*`
/// predicates that gate editor-only draw calls. Mirroring it into a
/// resource lets Bevy systems (e.g. a future `RenderExtractSet`
/// system that builds mesh batches for editor overlays) read these
/// values without reaching into `State::render`.
#[allow(dead_code)]
#[derive(Resource, Clone, Copy)]
pub struct RenderFrameInputRes(pub RenderFrameInput);

impl Default for RenderFrameInputRes {
    fn default() -> Self {
        use eg_types::{AppPhase, EditorMode};
        Self(RenderFrameInput {
            phase: AppPhase::Menu,
            editor_mode: EditorMode::Place,
            game_over: false,
            sky_color: eg_types::LevelMetadata::default().sky_color,
            grid_center: [0.0, 0.0],
            apply_gamma_correction: false,
        })
    }
}

/// Extracted gameplay trail vertices consumed by the compatibility renderer.
///
/// The vertex list is built from App-owned gameplay components in
/// `PostUpdate`; Runtime only uploads the result while the legacy
/// `SceneMeshes` storage remains in `State`.
#[derive(Resource, Default, Clone)]
pub struct RenderTrailVerticesRes(pub Vec<eg_mesh::Vertex>);

/// Extracted editor camera-arrow vertices consumed by the compatibility
/// renderer until editor overlay mesh storage is App-owned.
#[derive(Resource, Default, Clone)]
pub struct RenderEditorCameraArrowVerticesRes(pub Vec<eg_mesh::Vertex>);

/// Extracted editor cursor vertices consumed by the compatibility renderer.
#[derive(Resource, Default, Clone)]
pub struct RenderEditorCursorVerticesRes(pub Vec<eg_mesh::Vertex>);

/// Extracted editor hitbox visualization vertices consumed by the
/// compatibility renderer.
#[derive(Resource, Default, Clone)]
pub struct RenderEditorHitboxVerticesRes(pub Vec<eg_mesh::Vertex>);

/// Extracted editor tap-indicator vertices consumed by the compatibility
/// renderer until editor overlay mesh storage is App-owned.
#[derive(Resource, Default, Clone)]
pub struct RenderEditorTapIndicatorVerticesRes(pub Vec<eg_mesh::Vertex>);

/// Extracted editor selection overlay data consumed by the compatibility
/// renderer until editor overlay mesh storage is App-owned.
#[derive(Resource, Default, Clone)]
pub struct RenderEditorSelectionOverlayRes {
    pub stencil_vertices: Vec<eg_mesh::Vertex>,
    pub outline_vertices: Vec<eg_mesh::Vertex>,
    pub instances: Vec<EditorOutlineInstance>,
}

/// Extracted editor hover overlay data consumed by the compatibility renderer
/// until editor overlay mesh storage is App-owned.
#[derive(Resource, Default, Clone)]
pub struct RenderEditorHoverOverlayRes {
    pub stencil_vertices: Vec<eg_mesh::Vertex>,
    pub outline_vertices: Vec<eg_mesh::Vertex>,
}

/// Extracted editor transform-trigger marker vertices consumed by the
/// compatibility renderer until editor overlay mesh storage is App-owned.
#[derive(Resource, Default, Clone)]
pub struct RenderEditorTransformTriggerMarkerVerticesRes(pub Vec<eg_mesh::Vertex>);

/// Extracted block-effect vertices consumed by the compatibility renderer
/// until effect mesh storage is App-owned.
#[derive(Resource, Default, Clone)]
pub struct RenderBlockEffectVerticesRes(pub Vec<eg_mesh::Vertex>);

/// Extracted practice-checkpoint flag vertices consumed by the compatibility
/// renderer until the custom scene mesh storage is fully Bevy-owned.
#[derive(Resource, Default, Clone)]
pub struct RenderPracticeCheckpointVerticesRes(pub Vec<eg_mesh::Vertex>);

/// CPU block batches extracted for a gameplay frame that needs time-dependent
/// rendering. Retained Bevy presentation entities consume this output.
#[derive(Resource, Default, Clone)]
pub struct RenderPlayingBlockMeshesRes {
    pub result: Option<eg_mesh::BlockInstanceBuildResult>,
    pub chunked: Option<eg_mesh::ChunkedBlockMesh>,
}

/// Static menu-preview blocks extracted from the App-owned preview objects.
/// Retained Bevy presentation entities consume the cached chunk mesh.
#[derive(Resource, Default, Clone)]
pub struct RenderMenuPreviewBlockMeshesRes {
    pub preview_level_index: Option<usize>,
    pub chunked: Option<eg_mesh::ChunkedBlockMesh>,
}

/// CPU editor block meshes extracted from App-owned editor resources. Runtime
/// uploads the static chunk mesh and selected geometry into compatibility GPU
/// slots until render mesh storage is App-owned.
#[derive(Resource, Default, Clone)]
pub struct RenderEditorBlockMeshesRes {
    pub static_mesh: eg_mesh::ChunkedBlockMesh,
    pub selected_geometry: eg_mesh::MeshGeometry,
}

/// Extracted editor spawn-marker vertices consumed by the compatibility
/// renderer.
#[derive(Resource, Default, Clone)]
pub struct RenderEditorSpawnMarkerVerticesRes(pub Vec<eg_mesh::Vertex>);

/// Extracted editor transform-gizmo vertices consumed by Bevy presentation.
#[derive(Resource, Default, Clone)]
pub struct RenderEditorGizmoVerticesRes(pub Vec<eg_mesh::Vertex>);

/// Resize request resource.
///
/// The platform layer (winit / wasm canvas) pushes the latest surface
/// size into this resource on `WindowEvent::Resized` / canvas resize.
/// The [`RenderPlugin`](super::plugin) drains it during
/// `RenderPresentSet` and applies it to the domain render configuration;
/// Bevy's window/render integration applies the actual surface resize.
///
/// The resource is inserted with an empty default so Bevy schedules can
/// reach it even in bare test worlds.
#[allow(dead_code)]
#[derive(Resource, Clone, Copy)]
pub struct RenderResizeRequest {
    /// Latest physical size reported by the platform layer.
    pub size: eg_types::PhysicalSize<u32>,
    /// True if a resize is pending (consumed by the apply system).
    pub pending: bool,
}

impl Default for RenderResizeRequest {
    fn default() -> Self {
        Self {
            size: eg_types::PhysicalSize {
                width: 0,
                height: 0,
            },
            pending: false,
        }
    }
}

impl RenderResizeRequest {
    /// Queue a resize to the given physical size.
    #[allow(dead_code)]
    pub fn request(&mut self, size: eg_types::PhysicalSize<u32>) {
        self.size = size;
        self.pending = true;
    }

    /// Pop the pending resize, if any. Returns `None` when no resize
    /// is pending.
    #[allow(dead_code)]
    pub fn take(&mut self) -> Option<eg_types::PhysicalSize<u32>> {
        if self.pending {
            self.pending = false;
            Some(self.size)
        } else {
            None
        }
    }
}

/// Returns `true` if the world has every resource the Phase 4a
/// rendering extraction plugin owns. Used by tests to assert the
/// resource plumbing without snapshotting the full world.
#[allow(dead_code)]
pub fn render_pod_resources_present(world: &World) -> bool {
    world.contains_resource::<RenderUniformsRes>()
        && world.contains_resource::<RenderFrameInputRes>()
        && world.contains_resource::<RenderTrailVerticesRes>()
        && world.contains_resource::<RenderEditorCameraArrowVerticesRes>()
        && world.contains_resource::<RenderEditorCursorVerticesRes>()
        && world.contains_resource::<RenderEditorHitboxVerticesRes>()
        && world.contains_resource::<RenderEditorTapIndicatorVerticesRes>()
        && world.contains_resource::<RenderEditorSelectionOverlayRes>()
        && world.contains_resource::<RenderEditorHoverOverlayRes>()
        && world.contains_resource::<RenderEditorTransformTriggerMarkerVerticesRes>()
        && world.contains_resource::<RenderBlockEffectVerticesRes>()
        && world.contains_resource::<RenderPracticeCheckpointVerticesRes>()
        && world.contains_resource::<RenderPlayingBlockMeshesRes>()
        && world.contains_resource::<RenderMenuPreviewBlockMeshesRes>()
        && world.contains_resource::<RenderEditorBlockMeshesRes>()
        && world.contains_resource::<RenderEditorSpawnMarkerVerticesRes>()
        && world.contains_resource::<RenderEditorGizmoVerticesRes>()
        && world.contains_resource::<RenderConfigRes>()
        && world.contains_resource::<RenderResizeRequest>()
}

#[cfg(test)]
mod tests {
    use eg_types::PhysicalSize;

    use super::*;

    #[test]
    fn resize_request_take_returns_none_when_idle() {
        let mut req = RenderResizeRequest::default();
        assert!(req.take().is_none());
    }

    #[test]
    fn resize_request_take_returns_size_once() {
        let mut req = RenderResizeRequest::default();
        req.request(PhysicalSize {
            width: 800,
            height: 600,
        });
        let taken = req.take();
        assert!(matches!(
            taken,
            Some(PhysicalSize {
                width: 800,
                height: 600
            })
        ));
        assert!(req.take().is_none());
    }

    #[test]
    fn render_uniforms_default_is_identity() {
        let uniforms = RenderUniformsRes::default();
        assert_eq!(uniforms.color_space.apply_gamma_correction, 0.0);
        assert_eq!(uniforms.line.offset, [0.0, 0.0]);
    }

    #[test]
    fn render_frame_input_default_uses_menu_phase() {
        let input = RenderFrameInputRes::default();
        assert_eq!(input.0.phase, eg_types::AppPhase::Menu);
    }
}
