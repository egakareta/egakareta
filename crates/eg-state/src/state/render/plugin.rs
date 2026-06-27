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

use super::resources::{
    RenderFrameInputRes, RenderGpuRes, RenderMeshesRes, RenderResizeRequest, RenderSurfaceHostRes,
    RenderUniformsRes,
};
use crate::events::mesh_rebuild::{RebuildMeshEvent, RebuildMeshKind};

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
}
