/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Rendering extraction plugin (Phase 4a).
//!
//! Establishes the resource boundary + system-set graph for the
//! `RenderPlugin` skeleton. The POD resources ([`RenderUniformsRes`],
//! [`RenderFrameInputRes`], [`RenderResizeRequest`]) are inserted as
//! defaults and populated each frame by the
//! [`mirror_render_pod_state_system`]. [`RenderMeshesRes`] and
//! [`RenderSurfaceHostRes`] now own their authoritative construction
//! values. [`RenderGpuRes`] remains deferred because moving
//! `GpuContext` touches the remaining legacy present/update surface.
//!
//! Future phases:
//! - **Phase 4b** moves `State::render.gpu` into [`RenderGpuRes`] and
//!   replaces `state.render.gpu.X` reads with `world.resource::<RenderGpuRes>().0.X`.
//! - **Phase 4c** moved `State::render.meshes` + `surface_host` into
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

use super::resources::{
    RenderFrameInputRes, RenderMeshesRes, RenderResizeRequest, RenderSurfaceHostRes,
    RenderUniformsRes,
};
use crate::events::mesh_rebuild::RebuildMeshEvent;

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
///   RenderPresentSet    // issue the wgpu present (no-op in Phase 4a)
/// ```
///
/// In a future full-Bevy frame the order is:
///   `Simulation → RebuildMesh → Extract → Present` (see the
///   per-frame schedule graph in `state/sets.rs`).
#[allow(dead_code)]
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderSet {
    /// Phase 4d: detect `Changed<Position|Rotation|Size>` on object
    /// entities and emit a [`RebuildMeshEvent`].
    ///
    /// Today (Phase 4d-scaffold) this is a no-op system — there
    /// are no Bevy components yet (the legacy `LevelObject` struct
    /// is not a component). A future commit introduces the
    /// `ObjectBundle` components + entities and flips this set
    /// on. The system set + `RebuildMeshEvent` event type are
    /// in place so the schedule graph is final.
    RebuildMesh,
    /// Snapshot the world into render resources.
    ///
    /// Today this mirrors POD uniform values from the legacy
    /// `State::render.gpu.*_uniform_*` fields into [`RenderUniformsRes`]
    /// and [`RenderFrameInputRes`]. Future
    /// phases add a `rebuild_mesh_batches_system` that walks object
    /// entities (`Query<(&Position, &Rotation, &Size, &BlockId,
    /// &ColorTint, Option<&PlayState>)>` + player + camera + effects)
    /// and fills [`RenderMeshesRes`] via `eg_mesh` builders.
    Extract,
    /// Issue the wgpu present.
    ///
    /// Today (Phase 4e-scaffold) the placeholder
    /// `present_frame_placeholder_system` is registered. It reads
    /// the POD resources and verifies they're populated, but does
    /// not submit a wgpu pass. The actual present is called by
    /// `eg-app/src/platform/pipeline.rs::render_frame` via
    /// `State::render()`. Phase 4e-final replaces the
    /// `State::render()` call with a real `present_frame_system`
    /// that reads from [`RenderGpuRes`] + [`RenderMeshesRes`] +
    /// [`RenderUniformsRes`] + [`RenderFrameInputRes`].
    Present,
}

/// The rendering extraction plugin.
///
/// Registers the POD resources + a mirror system + a placeholder
/// present set. Mesh and surface ownership live in resources; GPU
/// ownership remains on the legacy `State::render` field until
/// Phase 4b-final.
pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RenderUniformsRes>()
            .init_resource::<RenderFrameInputRes>()
            .init_resource::<RenderResizeRequest>()
            .init_resource::<RenderMeshesRes>()
            .init_resource::<RenderSurfaceHostRes>()
            .add_message::<RebuildMeshEvent>();
        // `RenderGpuRes` is still deferred to Phase 4b. It owns the
        // raw `wgpu::Device` + `wgpu::Queue` handles which can't be
        // cheaply mirrored (the legacy `State::render.gpu` field is
        // authoritative; moving ownership is the remaining large
        // render refactor).
    }
}

/// `World`-flavoured registration for callers that build a `World`
/// directly (the `State` constructor). Mirrors
/// [`RenderPlugin::build`] without going through an `App`.
pub fn register_render_pod_resources(world: &mut World) {
    world.init_resource::<RenderUniformsRes>();
    world.init_resource::<RenderFrameInputRes>();
    world.init_resource::<RenderResizeRequest>();
    // Phase 4c: insert default heavy resources so standalone worlds
    // can query them. `State` construction overwrites these defaults
    // with the authoritative `SceneMeshes` / `SurfaceHost` values.
    world.init_resource::<RenderMeshesRes>();
    world.init_resource::<RenderSurfaceHostRes>();
    // Phase 4d: register the RebuildMeshEvent message buffer.
    world.init_resource::<Messages<RebuildMeshEvent>>();
}

/// Empty `RenderSet::Present` placeholder.
///
/// Kept as a named function so future phases can extend it (Phase 4e:
/// add a real `present_frame_system` that drains the render
/// resources and submits the wgpu present). Today it does nothing
/// because the actual present is invoked from `eg-app`.
pub fn present_render_frame_placeholder_system(
    uniforms: Res<RenderUniformsRes>,
    frame_input: Res<RenderFrameInputRes>,
) {
    // Phase 4e-scaffold: read the POD resources to confirm they're
    // populated. The `let _ =` is a deliberate no-op sink that makes
    // the function consume its args so the system stays in the
    // schedule graph. Future phases replace this with the real
    // `present_frame_system` that reads from `RenderGpuRes` +
    // `RenderMeshesRes` + `RenderUniformsRes` + `RenderFrameInputRes`
    // and submits the wgpu pass.
    let _ = (&uniforms, &frame_input);
}

/// Phase 4d-scaffold: detect mesh-rebuild changes and emit events.
///
/// Today this is a **no-op**: there are no Bevy components on object
/// entities yet (the legacy `LevelObject` is not a component). The
/// `RebuildMeshEvent` event type + the `RebuildMeshSet` system set
/// are in place so a future commit can introduce `ObjectBundle`
/// components + a `Query<Changed<Position>, Changed<Rotation>,
/// Changed<Size>>` and emit one event per dirty entity.
///
/// The system takes `MessageWriter<RebuildMeshEvent>` so the future
/// implementation has a place to write events. The `Message` is
/// registered by [`RenderPlugin::build`].
pub fn mark_mesh_dirty_placeholder_system(mut _writer: MessageWriter<RebuildMeshEvent>) {
    // Intentionally empty in Phase 4d-scaffold.
    let _ = &mut _writer;
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
        // Phase 4d-scaffold: a no-op system in `RebuildMeshSet`. The
        // event + system set are in place so a future commit can
        // introduce `Changed<>` detection + event emission.
        mark_mesh_dirty_placeholder_system.in_set(RenderSet::RebuildMesh),
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
        // Phase 4e-scaffold: the present set is a no-op that reads
        // the POD resources. Future phases add the real
        // `present_frame_system` here.
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
    use crate::state::render::resources::{RenderGpuRes, RenderMeshesRes, RenderSurfaceHostRes};
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

    /// Phase 4c-scaffold: `RenderMeshesRes` + `RenderSurfaceHostRes`
    /// are inserted (with default values) by
    /// `register_render_pod_resources`. `RenderGpuRes` is still
    /// deferred (Phase 4b).
    #[test]
    fn register_inserts_phase4c_heavy_resources() {
        let mut world = World::new();
        register_render_pod_resources(&mut world);
        assert!(world.contains_resource::<RenderMeshesRes>());
        assert!(world.contains_resource::<RenderSurfaceHostRes>());
        // Phase 4b deferred: still no RenderGpuRes.
        assert!(!world.contains_resource::<RenderGpuRes>());
    }

    /// Phase 4b guard: `render_heavy_resources_present` returns
    /// `false` because `RenderGpuRes` is still not inserted. When
    /// Phase 4b lands, this assertion will flip to `true`.
    #[test]
    fn render_heavy_resources_present_returns_false_in_phase_4a() {
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

    /// Phase 4d-scaffold: the `mark_mesh_dirty_placeholder_system`
    /// is wired into the `RenderSet::RebuildMesh` set. The system
    /// takes a `MessageWriter<RebuildMeshEvent>` so the future
    /// `Changed<>`-detection implementation has a place to write
    /// events. This test asserts the system compiles + is
    /// registered as a Bevy system (not just a free function).
    #[test]
    fn mark_mesh_dirty_placeholder_system_is_registered() {
        let mut schedule = bevy_ecs::schedule::Schedule::default();
        add_mirror_render_systems(&mut schedule);
        // The schedule compiled + the system was added without
        // panicking. The actual `Changed<>` detection requires
        // Phase 4d-final (ObjectBundle components).
        let _ = schedule;
    }

    /// Phase 4e-scaffold: the `present_render_frame_placeholder_system`
    /// reads the POD render resources to confirm they're populated.
    /// The `let _ =` sink inside the system means the function
    /// doesn't actually use the resources, but it does require
    /// them to be present in the World. This test asserts the
    /// system runs without panicking when the resources are
    /// registered.
    #[test]
    fn present_render_frame_placeholder_system_runs() {
        let mut world = World::new();
        register_render_pod_resources(&mut world);

        // Run the system in isolation. The system takes the two
        // POD resources as system params; if they're missing the
        // system would panic at registration time. Today (Phase
        // 4e-scaffold) the system just consumes the args.
        let mut schedule = bevy_ecs::schedule::Schedule::default();
        schedule.add_systems(present_render_frame_placeholder_system);
        schedule.run(&mut world);
    }
}
