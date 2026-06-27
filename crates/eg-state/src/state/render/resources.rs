/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! ECS resources for the rendering extraction plugin (Phase 4a).
//!
//! The full plan calls for `GpuContextRes`, `SceneMeshesRes`,
//! `SurfaceHostRes`, `CameraUniform`, `LineUniform`, and
//! `ColorSpaceUniform` to be lifted out of the legacy `State` struct
//! and become `bevy_ecs::Resource`s. This file defines the resource
//! types. The POD resources ([`RenderUniformsRes`],
//! [`RenderFrameInputRes`]) are still mirrored while the legacy frame
//! update computes uniforms, and the heavy resources
//! ([`RenderGpuRes`], [`RenderMeshesRes`], [`RenderSurfaceHostRes`])
//! own the authoritative values inserted during `State` construction.
//!
//! ## Why split POD vs heavy?
//!
//! - `wgpu::Device`, `wgpu::Queue`, `wgpu::Buffer`, `wgpu::Surface`,
//!   `wgpu::BindGroup` and friends are all `Send + Sync` and can be
//!   stored in Bevy `Resource`s directly. Phase 4 moved these heavy
//!   values behind `State` accessors backed by World resources.
//! - Mirroring just the POD types first establishes the resource
//!   boundary, the system-set graph, and the extraction system
//!   contract. Subsequent phases replace the POD mirror with direct
//!   resource writes as `State::update` moves into Bevy systems.
//!
//! REFACTOR POINT(full-bevy): the full-Bevy migration replaces this
//! entire module with `bevy_render` render-world resources
//! (`bevy_render::render_resource::Buffer`) plus
//! `bevy_render::camera::Camera` etc. The Bevy `Message`-driven
//! `extract` system here is replaced by the `RenderApp` extract
//! schedule in the `bevy_render` graph.

use bevy_ecs::prelude::*;
use eg_platform::state_host::SurfaceHost;
use eg_render::{RenderFrameInput, SceneMeshes};
use eg_types::{CameraUniform, ColorSpaceUniform, LineUniform};

/// Wrapper resource for the full [`eg_render::GpuContext`].
///
/// Inserted by `State` construction with the authoritative wgpu
/// context. Bare test worlds that do not initialize wgpu may omit it.
#[allow(dead_code)]
#[derive(Resource)]
pub struct RenderGpuRes(pub eg_render::GpuContext);

/// Wrapper resource for the [`eg_render::SceneMeshes`] storage.
///
/// Phase 4c moved the former `State::render.meshes` field into this
/// resource so scene mesh storage is now held by the Bevy world while
/// legacy callers go through `State::render_meshes()`.
#[allow(dead_code)]
#[derive(Resource, Default)]
pub struct RenderMeshesRes(pub SceneMeshes);

/// Wrapper resource for the optional platform [`SurfaceHost`].
///
/// Phase 4c moved the former `State::render.surface_host` field into
/// this resource so surface ownership is now held by the Bevy world
/// while legacy callers go through `State::surface_host()`.
#[allow(dead_code)]
#[derive(Resource, Default)]
pub struct RenderSurfaceHostRes(pub Option<SurfaceHost>);

/// Bundle of the three `Copy` uniforms that drive the per-frame
/// shader constants (`CameraUniform`, `LineUniform`,
/// `ColorSpaceUniform`).
///
/// Mirrored from the legacy frame-update uniform computation during
/// the [`RenderExtractSet`](super::plugin) phase. The mirror
/// is a free operation: the uniform values are tiny POD types
/// (a `Mat4` and a handful of `f32`s), and the actual GPU buffers
/// stay in `RenderGpuRes`. Bevy systems can use this
/// resource to read the per-frame uniform values without touching
/// the legacy state.
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

/// Resize request resource.
///
/// The platform layer (winit / wasm canvas) pushes the latest surface
/// size into this resource on `WindowEvent::Resized` / canvas resize.
/// The [`RenderPlugin`](super::plugin) drains it during
/// `RenderPresentSet` and calls `State::resize_surface` (Phase 4b: a
/// dedicated `apply_resize_surface_system`).
///
/// Phase 4a only **defines** the resource; the platform-side wiring
/// lands with the Phase 7 `App` work. The resource is inserted with
/// an empty default so Bevy schedules can already reach it.
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
        && world.contains_resource::<RenderResizeRequest>()
}

/// Returns `true` if the world has every heavy render resource. This
/// is true for constructed `State` worlds after wgpu initialization;
/// bare test worlds may omit `RenderGpuRes` because it has no default.
#[allow(dead_code)]
pub fn render_heavy_resources_present(world: &World) -> bool {
    world.contains_resource::<RenderGpuRes>()
        && world.contains_resource::<RenderMeshesRes>()
        && world.contains_resource::<RenderSurfaceHostRes>()
}

/// Returns `true` if the world has every Phase 4c heavy resource.
/// Today this is `true` for every eg-state `World`; the resources are
/// inserted with authoritative construction values.
#[allow(dead_code)]
pub fn render_phase4c_resources_present(world: &World) -> bool {
    world.contains_resource::<RenderMeshesRes>()
        && world.contains_resource::<RenderSurfaceHostRes>()
}

/// Render POD mirror helper.
///
/// Reads the current uniform values + frame input exposed by the
/// closure and writes them into
/// [`RenderUniformsRes`] / [`RenderFrameInputRes`].
///
/// The closure lets us reach legacy frame-computed values without
/// making `State` a system arg. The closure must be cheap
/// (it runs once per frame) and must not retain references past the
/// call.
#[allow(dead_code)]
pub fn mirror_render_pod_state(
    uniforms: &mut RenderUniformsRes,
    frame_input: &mut RenderFrameInputRes,
    accessor: impl FnMut(&mut RenderUniformsRes, &mut RenderFrameInputRes),
) {
    let mut accessor = accessor;
    accessor(uniforms, frame_input);
}

#[cfg(test)]
mod tests {
    use super::*;
    use eg_types::PhysicalSize;

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
