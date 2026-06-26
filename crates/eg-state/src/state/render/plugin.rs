/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Rendering extraction plugin (Phase 4a).
//!
//! Establishes the resource boundary + system-set graph for the
//! `RenderPlugin` skeleton. Phase 4a only **scaffolds** the plugin:
//! the heavy resources ([`RenderGpuRes`], [`RenderMeshesRes`],
//! [`RenderSurfaceHostRes`]) are defined in [`super::resources`] but
//! not yet inserted into the world — the legacy `State::render` field
//! remains authoritative. The POD resources ([`RenderUniformsRes`],
//! [`RenderFrameInputRes`], [`RenderResizeRequest`]) are inserted as
//! defaults and populated each frame by the
//! [`mirror_render_pod_state_system`].
//!
//! Future phases:
//! - **Phase 4b** moves `State::render.gpu` into [`RenderGpuRes`] and
//!   replaces `state.render.gpu.X` reads with `world.resource::<RenderGpuRes>().0.X`.
//! - **Phase 4c** moves `State::render.meshes` + `surface_host` into
//!   [`RenderMeshesRes`] / [`RenderSurfaceHostRes`].
//! - **Phase 4d** replaces the legacy `rebuild_*_mesh` call sites in
//!   `state/editor_scene.rs` with `RebuildMeshSet` systems that read
//!   `Changed<>` components and emit mesh buffers into
//!   [`RenderMeshesRes`].
//! - **Phase 4e** adds an actual wgpu `present_frame_system` in
//!   `RenderPresentSet` that drains [`RenderFrameInputRes`] +
//!   [`RenderMeshesRes`] + [`RenderGpuRes`] and submits the render
//!   pass. The current `State::render()` call from
//!   `eg-app/src/platform/pipeline.rs::render_frame` is removed in
//!   the same commit.
//!
//! REFACTOR POINT(full-bevy): this plugin + system set graph is the
//! placeholder for `bevy_render`'s render-world + render graph. The
//! `extract_render_state_system` here becomes a
//! `RenderApp::extract` system in the full-Bevy migration; the
//! `RenderPresentSet` is replaced by `bevy_render`'s `Render` set.

use bevy_app::{App, Plugin};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::{Schedule, SystemSet};

use super::resources::{RenderFrameInputRes, RenderResizeRequest, RenderUniformsRes};

/// System sets for the rendering extraction plugin.
///
/// These sets mirror the legacy `render_to_view` /
/// `render_with_overlay` call order so we can slot Bevy systems in
/// without changing the visible behavior.
///
/// Ordering contract (see `docs/bevy-migration.md` Phase 4):
/// ```text
///   RenderExtractSet    // build SceneMeshes + uniform values
///   RenderPresentSet    // issue the wgpu present (no-op in Phase 4a)
/// ```
///
/// In a future full-Bevy frame the order is:
///   `Simulation → RebuildMesh → Extract → Present` (see the
///   per-frame schedule graph in `state/sets.rs`).
#[allow(dead_code)]
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderSet {
    /// Snapshot the world into render resources.
    ///
    /// Today (Phase 4a) this only mirrors POD uniform values from
    /// the legacy `State::render.gpu.*_uniform_*` fields into
    /// [`RenderUniformsRes`] and [`RenderFrameInputRes`]. Future
    /// phases add a `rebuild_mesh_batches_system` that walks object
    /// entities (`Query<(&Position, &Rotation, &Size, &BlockId,
    /// &ColorTint, Option<&PlayState>)>` + player + camera + effects)
    /// and fills [`RenderMeshesRes`] via `eg_mesh` builders.
    Extract,
    /// Issue the wgpu present.
    ///
    /// Today (Phase 4a) this is empty — the actual present is called
    /// by `eg-app/src/platform/pipeline.rs::render_frame` via
    /// `State::render()`. Phase 4e replaces the `State::render()` call
    /// with a `present_frame_system` that reads from
    /// [`RenderGpuRes`] + [`RenderMeshesRes`] + [`RenderUniformsRes`]
    /// + [`RenderFrameInputRes`].
    Present,
}

/// The rendering extraction plugin.
///
/// Registers the POD resources + a mirror system + a placeholder
/// present set. The legacy `State::render` field remains
/// authoritative; the plugin is a non-invasive scaffold for Phase 4a.
pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RenderUniformsRes>()
            .init_resource::<RenderFrameInputRes>()
            .init_resource::<RenderResizeRequest>();
        // The heavy resources (RenderGpuRes, RenderMeshesRes,
        // RenderSurfaceHostRes) are intentionally NOT registered here
        // in Phase 4a. They are defined in `super::resources` for
        // type stability but inserted in Phase 4b/4c once the
        // ownership transfer is implemented.
    }
}

/// `World`-flavoured registration for callers that build a `World`
/// directly (the `State` constructor). Mirrors
/// [`RenderPlugin::build`] without going through an `App`.
pub fn register_render_pod_resources(world: &mut World) {
    world.init_resource::<RenderUniformsRes>();
    world.init_resource::<RenderFrameInputRes>();
    world.init_resource::<RenderResizeRequest>();
}

/// Empty `RenderSet::Present` placeholder.
///
/// Kept as a named function so future phases can extend it (Phase 4e:
/// add a real `present_frame_system` that drains the render
/// resources and submits the wgpu present). Today it does nothing
/// because the actual present is invoked from `eg-app`.
pub fn present_render_frame_placeholder_system() {
    // Intentionally empty. See module docs.
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
/// the placeholder).
pub fn add_mirror_render_systems(schedule: &mut Schedule) {
    schedule.add_systems((
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
        // Phase 4a: the present set is a no-op. Future phases add
        // the real `present_frame_system` here.
        present_render_frame_placeholder_system.in_set(RenderSet::Present),
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
fn mirror_pod_state_via_bridge(
    mut uniforms: ResMut<RenderUniformsRes>,
    mut frame_input: ResMut<RenderFrameInputRes>,
) {
    use crate::state::render::mirror::take_pending_mirror;

    if let Some(mut mirror) = take_pending_mirror() {
        mirror(&mut uniforms, &mut frame_input);
    }
    // If no mirror was queued, the resources stay at their default
    // values. This is the correct behavior for tests that don't go
    // through the full `State::tick_state` path. The `let _ =` is a
    // deliberate no-op sink that makes the function nominally
    // consume its args so the system stays in the schedule graph.
    let _ = (&mut uniforms, &mut frame_input);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::render::resources::{
        mirror_render_pod_state, render_heavy_resources_present, render_pod_resources_present,
    };
    use bevy_ecs::world::World;
    use eg_render::RenderFrameInput;
    use eg_types::{AppPhase, EditorMode, LevelMetadata};

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
    fn render_heavy_resources_present_returns_false_in_phase_4a() {
        // The heavy resources are defined but not inserted in Phase
        // 4a. This test guards the contract: a Phase 4b commit that
        // adds the insert path will flip this to true.
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
}
