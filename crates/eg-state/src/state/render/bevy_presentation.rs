/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Retained Bevy presentation entities for generated block chunks.
//!
//! The current custom presenter still consumes CPU extraction resources. The
//! full-Bevy root instead installs this plugin after the asset and material
//! plugins. It retains one entity per generated editor chunk and updates only
//! affected chunk meshes when extraction changes.

use std::{
    collections::{HashMap, HashSet},
    hash::Hasher,
};

use bevy::{
    asset::{Assets, Handle},
    camera::ClearColor,
    ecs::prelude::*,
    mesh::{Mesh, Mesh3d},
    pbr::MeshMaterial3d,
    prelude::{Camera3d, Color, Transform, Vec3},
    state::prelude::State,
};
use eg_mesh::{
    MeshGeometry, RenderChunkKey, expand_block_instance_batch, mesh_geometry_to_bevy_mesh,
};
use eg_types::AppPhase;

use super::{
    bevy_material::{BlockMaterial, BlockMaterialAssetsPlugin, BlockMaterialPlugin},
    resources::{
        RenderCameraPoseRes, RenderEditorBlockMeshesRes, RenderFrameInputRes,
        RenderMenuPreviewBlockMeshesRes, RenderPlayingBlockMeshesRes,
    },
};

/// Strong material handle supplied by the root loading/presentation setup.
///
/// `None` keeps the presentation dormant until the block texture array has
/// loaded and the material asset has been created.
#[derive(Resource, Default)]
pub struct BlockPresentationMaterial(pub Option<Handle<BlockMaterial>>);

/// Marker identifying a retained editor chunk presentation entity.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EditorBlockChunkPresentation {
    /// The world-grid chunk represented by this entity.
    pub key: RenderChunkKey,
}

/// Marker identifying a retained menu-preview chunk presentation entity.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct MenuPreviewBlockChunkPresentation {
    /// The world-grid chunk represented by this entity.
    pub key: RenderChunkKey,
}

/// Marker identifying a retained static-gameplay chunk presentation entity.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PlayingBlockChunkPresentation {
    /// The world-grid chunk represented by this entity.
    pub key: RenderChunkKey,
}

/// Fingerprint of the geometry assigned to a retained chunk entity.
///
/// Extraction still produces a complete chunk list, but this lets the Bevy
/// presentation layer preserve the existing mesh asset for every chunk whose
/// generated geometry is unchanged. That makes an editor change local to the
/// affected chunks instead of reallocating the entire visible level.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
struct BlockChunkGeometryFingerprint(u64);

/// Marker for the retained expanded gameplay mesh used by animated and
/// trigger-driven block batches.
#[derive(Component, Default)]
pub struct PlayingDynamicBlockPresentation;

/// Marker for the retained scene camera driven by App-world gameplay/editor
/// camera evaluation.
#[derive(Component, Default)]
pub struct BlockPresentationCamera;

/// Owns the retained Bevy camera for block presentation.
pub struct BlockPresentationCameraPlugin;

impl bevy::app::Plugin for BlockPresentationCameraPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_systems(bevy::app::Startup, spawn_block_presentation_camera_system)
            .add_systems(
                bevy::app::PostUpdate,
                sync_block_presentation_camera_system
                    .after(super::plugin::update_render_uniforms_system),
            );
    }
}

/// Owns scene-wide environment values derived from App-world render input.
pub struct BlockPresentationEnvironmentPlugin;

impl bevy::app::Plugin for BlockPresentationEnvironmentPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.init_resource::<ClearColor>().add_systems(
            bevy::app::PostUpdate,
            sync_block_presentation_clear_color_system
                .after(super::plugin::update_render_frame_input_system),
        );
    }
}

/// Owns retained block-chunk entities for the full-Bevy renderer.
pub struct BlockPresentationPlugin;

impl bevy::app::Plugin for BlockPresentationPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.init_resource::<BlockPresentationMaterial>()
            .init_resource::<RenderEditorBlockMeshesRes>()
            .init_resource::<RenderMenuPreviewBlockMeshesRes>()
            .init_resource::<RenderPlayingBlockMeshesRes>()
            .add_systems(
                bevy::app::PostUpdate,
                (
                    sync_menu_preview_block_chunk_entities_system
                        .after(super::plugin::extract_menu_preview_block_mesh_system),
                    sync_playing_block_chunk_entities_system
                        .after(super::plugin::extract_playing_block_meshes_system),
                    sync_playing_dynamic_block_entity_system
                        .after(super::plugin::extract_playing_block_meshes_system),
                    sync_editor_block_chunk_entities_system
                        .after(super::plugin::extract_editor_block_meshes_system),
                ),
            );
    }
}

/// Complete block-render composition for the full-Bevy root application.
///
/// This deliberately does not join the transitional custom-wgpu
/// [`super::plugin::RenderPlugin`]. The atomic runner cutover installs this
/// plugin after `DefaultPlugins` instead.
pub struct BevyBlockRenderPlugin;

impl bevy::app::Plugin for BevyBlockRenderPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        app.add_plugins((
            BlockMaterialPlugin,
            BlockPresentationPlugin,
            OverlayPresentationPlugin,
            BlockPresentationCameraPlugin,
            BlockPresentationEnvironmentPlugin,
            BlockMaterialAssetsPlugin,
        ));
    }
}

fn spawn_block_presentation_camera_system(mut commands: Commands) {
    let pose = RenderCameraPoseRes::default();
    commands.spawn((
        BlockPresentationCamera,
        Camera3d::default(),
        Transform::from_translation(Vec3::from_array(pose.eye))
            .looking_at(Vec3::from_array(pose.target), Vec3::Y),
    ));
}

fn sync_block_presentation_camera_system(
    pose: Res<RenderCameraPoseRes>,
    mut cameras: Query<&mut Transform, With<BlockPresentationCamera>>,
) {
    if !pose.is_changed() {
        return;
    }
    let eye = Vec3::from_array(pose.eye);
    let target = Vec3::from_array(pose.target);
    for mut transform in &mut cameras {
        *transform = Transform::from_translation(eye).looking_at(target, Vec3::Y);
    }
}

fn sync_block_presentation_clear_color_system(
    frame_input: Res<RenderFrameInputRes>,
    mut clear_color: ResMut<ClearColor>,
) {
    if !frame_input.is_changed() {
        return;
    }
    let [red, green, blue] = frame_input.0.sky_color;
    clear_color.0 = Color::srgb(red, green, blue);
}

/// Synchronize editor chunk extraction into retained `Mesh3d` entities.
///
/// The extractor only changes its resource when authored blocks, selection,
/// or timeline-derived geometry changes. Consequently, an idle editor does
/// not allocate meshes or touch entities each frame.
fn sync_editor_block_chunk_entities_system(
    phase: Res<State<AppPhase>>,
    editor_meshes: Res<RenderEditorBlockMeshesRes>,
    material: Res<BlockPresentationMaterial>,
    mut commands: Commands,
    mut mesh_assets: ResMut<Assets<Mesh>>,
    existing: Query<(
        Entity,
        &EditorBlockChunkPresentation,
        Option<&Mesh3d>,
        Option<&BlockChunkGeometryFingerprint>,
    )>,
) {
    if !editor_meshes.is_changed() && !material.is_changed() && !phase.is_changed() {
        return;
    }
    let chunks = (*phase.get() == AppPhase::Editor).then_some(&editor_meshes.static_mesh);
    sync_block_chunk_entities(
        chunks,
        &material,
        material.is_changed(),
        &mut commands,
        &mut mesh_assets,
        &existing,
        |key| EditorBlockChunkPresentation { key },
    );
}

fn sync_menu_preview_block_chunk_entities_system(
    phase: Res<State<AppPhase>>,
    menu_meshes: Res<RenderMenuPreviewBlockMeshesRes>,
    material: Res<BlockPresentationMaterial>,
    mut commands: Commands,
    mut mesh_assets: ResMut<Assets<Mesh>>,
    existing: Query<(
        Entity,
        &MenuPreviewBlockChunkPresentation,
        Option<&Mesh3d>,
        Option<&BlockChunkGeometryFingerprint>,
    )>,
) {
    if !menu_meshes.is_changed() && !material.is_changed() && !phase.is_changed() {
        return;
    }
    let chunks = (*phase.get() == AppPhase::Menu)
        .then_some(menu_meshes.chunked.as_ref())
        .flatten();
    sync_block_chunk_entities(
        chunks,
        &material,
        material.is_changed(),
        &mut commands,
        &mut mesh_assets,
        &existing,
        |key| MenuPreviewBlockChunkPresentation { key },
    );
}

fn sync_playing_block_chunk_entities_system(
    phase: Res<State<AppPhase>>,
    playing_meshes: Res<RenderPlayingBlockMeshesRes>,
    material: Res<BlockPresentationMaterial>,
    mut commands: Commands,
    mut mesh_assets: ResMut<Assets<Mesh>>,
    existing: Query<(
        Entity,
        &PlayingBlockChunkPresentation,
        Option<&Mesh3d>,
        Option<&BlockChunkGeometryFingerprint>,
    )>,
) {
    if !playing_meshes.is_changed() && !material.is_changed() && !phase.is_changed() {
        return;
    }
    let chunks = matches!(*phase.get(), AppPhase::Playing | AppPhase::GameOver)
        .then_some(playing_meshes.chunked.as_ref())
        .flatten();
    sync_block_chunk_entities(
        chunks,
        &material,
        material.is_changed(),
        &mut commands,
        &mut mesh_assets,
        &existing,
        |key| PlayingBlockChunkPresentation { key },
    );
}

fn sync_playing_dynamic_block_entity_system(
    phase: Res<State<AppPhase>>,
    playing_meshes: Res<RenderPlayingBlockMeshesRes>,
    material: Res<BlockPresentationMaterial>,
    mut commands: Commands,
    mut mesh_assets: ResMut<Assets<Mesh>>,
    existing: Query<(Entity, Option<&Mesh3d>), With<PlayingDynamicBlockPresentation>>,
) {
    if !playing_meshes.is_changed() && !material.is_changed() && !phase.is_changed() {
        return;
    }
    let result = matches!(*phase.get(), AppPhase::Playing | AppPhase::GameOver)
        .then_some(playing_meshes.result.as_ref())
        .flatten();
    let Some(result) = result else {
        despawn_dynamic_block_entity(&mut commands, &mut mesh_assets, &existing);
        return;
    };
    let Some(material) = material.0.clone() else {
        return;
    };

    let geometry = block_instance_result_geometry(result);
    if geometry.vertices.is_empty() {
        despawn_dynamic_block_entity(&mut commands, &mut mesh_assets, &existing);
        return;
    }
    let mesh = mesh_assets.add(mesh_geometry_to_bevy_mesh(&geometry));
    if let Some((entity, old_mesh)) = existing.iter().next() {
        commands
            .entity(entity)
            .insert((Mesh3d(mesh), MeshMaterial3d(material)));
        if let Some(old_mesh) = old_mesh {
            mesh_assets.remove(&old_mesh.0);
        }
    } else {
        commands.spawn((
            PlayingDynamicBlockPresentation,
            Mesh3d(mesh),
            MeshMaterial3d(material),
        ));
    }
}

fn despawn_dynamic_block_entity(
    commands: &mut Commands,
    mesh_assets: &mut Assets<Mesh>,
    existing: &Query<(Entity, Option<&Mesh3d>), With<PlayingDynamicBlockPresentation>>,
) {
    for (entity, mesh) in existing.iter() {
        commands.entity(entity).despawn();
        if let Some(mesh) = mesh {
            mesh_assets.remove(&mesh.0);
        }
    }
}

fn sync_block_chunk_entities<T: Component + BlockChunkPresentation>(
    chunks: Option<&eg_mesh::ChunkedBlockMesh>,
    material: &BlockPresentationMaterial,
    material_changed: bool,
    commands: &mut Commands,
    mesh_assets: &mut Assets<Mesh>,
    existing: &Query<(
        Entity,
        &T,
        Option<&Mesh3d>,
        Option<&BlockChunkGeometryFingerprint>,
    )>,
    marker: impl Fn(RenderChunkKey) -> T,
) {
    let Some(material_handle) = material.0.clone() else {
        return;
    };
    let chunks = chunks.map_or(&[][..], |chunks| chunks.chunks.as_slice());
    let existing_by_key = existing
        .iter()
        .map(|(entity, presentation, mesh, fingerprint)| {
            (
                presentation.key(),
                (entity, mesh.cloned(), fingerprint.copied()),
            )
        })
        .collect::<HashMap<_, _>>();
    let current_keys = chunks.iter().map(|chunk| chunk.key).collect::<HashSet<_>>();

    for (&key, &(entity, ref mesh, _)) in &existing_by_key {
        if !current_keys.contains(&key) {
            commands.entity(entity).despawn();
            if let Some(mesh) = mesh {
                mesh_assets.remove(mesh);
            }
        }
    }

    for chunk in chunks {
        let geometry = chunk_geometry(chunk);
        let fingerprint = BlockChunkGeometryFingerprint(mesh_geometry_fingerprint(&geometry));
        if let Some((entity, old_mesh, old_fingerprint)) = existing_by_key.get(&chunk.key) {
            if old_fingerprint.as_ref() == Some(&fingerprint) && old_mesh.is_some() {
                if material_changed {
                    commands
                        .entity(*entity)
                        .insert(MeshMaterial3d(material_handle.clone()));
                }
                continue;
            }

            let mesh = mesh_assets.add(mesh_geometry_to_bevy_mesh(&geometry));
            commands.entity(*entity).insert((
                Mesh3d(mesh),
                MeshMaterial3d(material_handle.clone()),
                fingerprint,
            ));
            if let Some(old_mesh) = old_mesh {
                mesh_assets.remove(old_mesh);
            }
        } else {
            let mesh = mesh_assets.add(mesh_geometry_to_bevy_mesh(&geometry));
            commands.spawn((
                marker(chunk.key),
                Mesh3d(mesh),
                MeshMaterial3d(material_handle.clone()),
                fingerprint,
            ));
        }
    }
}

trait BlockChunkPresentation {
    fn key(&self) -> RenderChunkKey;
}

impl BlockChunkPresentation for EditorBlockChunkPresentation {
    fn key(&self) -> RenderChunkKey {
        self.key
    }
}

impl BlockChunkPresentation for MenuPreviewBlockChunkPresentation {
    fn key(&self) -> RenderChunkKey {
        self.key
    }
}

impl BlockChunkPresentation for PlayingBlockChunkPresentation {
    fn key(&self) -> RenderChunkKey {
        self.key
    }
}

fn chunk_geometry(chunk: &eg_mesh::BlockMeshChunk) -> MeshGeometry {
    let mut geometry = chunk.greedy_geometry.clone();
    geometry.append_geometry(chunk.fallback.fallback_geometry.clone());
    for batch in &chunk.fallback.batches {
        geometry.append_geometry(expand_block_instance_batch(batch));
    }
    geometry
}

fn mesh_geometry_fingerprint(geometry: &MeshGeometry) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    hasher.write(bytemuck::cast_slice(&geometry.vertices));
    match &geometry.indices {
        Some(indices) => {
            hasher.write_u8(1);
            hasher.write(bytemuck::cast_slice(indices));
        }
        None => hasher.write_u8(0),
    }
    hasher.finish()
}

fn block_instance_result_geometry(result: &eg_mesh::BlockInstanceBuildResult) -> MeshGeometry {
    puffin::profile_scope!("RenderPresentationDynamicBlocks");
    let mut geometry = result.fallback_geometry.clone();
    for batch in &result.batches {
        geometry.append_geometry(expand_block_instance_batch(batch));
    }
    geometry
}

// ── Overlay presentation entities ────────────────────────────────────
//
// These systems convert extracted CPU overlay vertices into retained Bevy
// `Mesh3d` + `MeshMaterial3d<BlockMaterial>` entities so that editor and
// gameplay overlays render through the same material pipeline as block
// chunks. Each system follows the pattern established by the block-chunk
// sync: despawn when empty, skip when unchanged, update mesh asset on
// content change.

/// Marker for the retained trail overlay entity.
#[derive(Component, Default)]
pub struct TrailOverlayPresentation;

/// Marker for the retained editor camera-arrow overlay entity.
#[derive(Component, Default)]
pub struct CameraArrowOverlayPresentation;

/// Marker for the retained editor cursor overlay entity.
#[derive(Component, Default)]
pub struct CursorOverlayPresentation;

/// Marker for the retained hitbox-visualization overlay entity.
#[derive(Component, Default)]
pub struct HitboxOverlayPresentation;

/// Marker for the retained tap-indicator overlay entity.
#[derive(Component, Default)]
pub struct TapIndicatorOverlayPresentation;

/// Marker for the retained selection-stencil overlay entity.
#[derive(Component, Default)]
pub struct SelectionStencilPresentation;

/// Marker for the retained selection-outline overlay entity.
#[derive(Component, Default)]
pub struct SelectionOutlinePresentation;

/// Marker for the retained hover-stencil overlay entity.
#[derive(Component, Default)]
pub struct HoverStencilPresentation;

/// Marker for the retained hover-outline overlay entity.
#[derive(Component, Default)]
pub struct HoverOutlinePresentation;

/// Marker for the retained transform-trigger-marker overlay entity.
#[derive(Component, Default)]
pub struct TransformTriggerMarkerPresentation;

/// Marker for the retained block-effect overlay entity.
#[derive(Component, Default)]
pub struct BlockEffectOverlayPresentation;

/// Marker for the retained practice-checkpoint overlay entity.
#[derive(Component, Default)]
pub struct PracticeCheckpointOverlayPresentation;

/// Marker for the retained spawn-marker overlay entity.
#[derive(Component, Default)]
pub struct SpawnMarkerOverlayPresentation;

/// Marker for the retained gizmo overlay entity.
#[derive(Component, Default)]
pub struct GizmoOverlayPresentation;

/// Trait implemented by overlay vertex resources to provide vertex access.
///
/// This allows the generic sync system to operate over any resource that
/// wraps a `Vec<Vertex>`, avoiding closure lifetime issues in Bevy system
/// parameters.
trait OverlayVertices {
    fn overlay_vertices(&self) -> &[eg_mesh::Vertex];
}

impl OverlayVertices for super::resources::RenderTrailVerticesRes {
    fn overlay_vertices(&self) -> &[eg_mesh::Vertex] {
        &self.0
    }
}
impl OverlayVertices for super::resources::RenderEditorCameraArrowVerticesRes {
    fn overlay_vertices(&self) -> &[eg_mesh::Vertex] {
        &self.0
    }
}
impl OverlayVertices for super::resources::RenderEditorCursorVerticesRes {
    fn overlay_vertices(&self) -> &[eg_mesh::Vertex] {
        &self.0
    }
}
impl OverlayVertices for super::resources::RenderEditorHitboxVerticesRes {
    fn overlay_vertices(&self) -> &[eg_mesh::Vertex] {
        &self.0
    }
}
impl OverlayVertices for super::resources::RenderEditorTapIndicatorVerticesRes {
    fn overlay_vertices(&self) -> &[eg_mesh::Vertex] {
        &self.0
    }
}
impl OverlayVertices for super::resources::RenderEditorTransformTriggerMarkerVerticesRes {
    fn overlay_vertices(&self) -> &[eg_mesh::Vertex] {
        &self.0
    }
}
impl OverlayVertices for super::resources::RenderBlockEffectVerticesRes {
    fn overlay_vertices(&self) -> &[eg_mesh::Vertex] {
        &self.0
    }
}
impl OverlayVertices for super::resources::RenderPracticeCheckpointVerticesRes {
    fn overlay_vertices(&self) -> &[eg_mesh::Vertex] {
        &self.0
    }
}
impl OverlayVertices for super::resources::RenderEditorSpawnMarkerVerticesRes {
    fn overlay_vertices(&self) -> &[eg_mesh::Vertex] {
        &self.0
    }
}
impl OverlayVertices for super::resources::RenderEditorGizmoVerticesRes {
    fn overlay_vertices(&self) -> &[eg_mesh::Vertex] {
        &self.0
    }
}

/// Generate a sync system for a simple vertex-list overlay resource.
///
/// The macro expands to a Bevy system that:
/// - Skips when the resource, material, and phase are unchanged.
/// - Despawns the existing entity when vertices are empty.
/// - Creates or updates a `Mesh3d` + `MeshMaterial3d<BlockMaterial>`
///   entity when vertices are present.
macro_rules! impl_overlay_sync_system {
    ($name:ident, $resource:ty, $marker:ty) => {
        fn $name(
            phase: Res<State<AppPhase>>,
            resource: Res<$resource>,
            material: Res<BlockPresentationMaterial>,
            mut commands: Commands,
            mut mesh_assets: ResMut<Assets<Mesh>>,
            existing: Query<(Entity, Option<&Mesh3d>), With<$marker>>,
        ) {
            if !resource.is_changed() && !material.is_changed() && !phase.is_changed() {
                return;
            }
            let vertices = resource.overlay_vertices();
            let Some(material_handle) = material.0.clone() else {
                return;
            };
            if vertices.is_empty() {
                for (entity, mesh) in existing.iter() {
                    commands.entity(entity).despawn();
                    if let Some(mesh) = mesh {
                        mesh_assets.remove(&mesh.0);
                    }
                }
                return;
            }
            let mesh = mesh_assets.add(eg_mesh::vertices_to_bevy_mesh(vertices));
            if let Some((entity, old_mesh)) = existing.iter().next() {
                commands
                    .entity(entity)
                    .insert((Mesh3d(mesh), MeshMaterial3d(material_handle)));
                if let Some(old_mesh) = old_mesh {
                    mesh_assets.remove(&old_mesh.0);
                }
            } else {
                commands.spawn((
                    <$marker>::default(),
                    Mesh3d(mesh),
                    MeshMaterial3d(material_handle),
                ));
            }
        }
    };
}

impl_overlay_sync_system!(
    sync_trail_overlay_system,
    super::resources::RenderTrailVerticesRes,
    TrailOverlayPresentation
);
impl_overlay_sync_system!(
    sync_camera_arrow_overlay_system,
    super::resources::RenderEditorCameraArrowVerticesRes,
    CameraArrowOverlayPresentation
);
impl_overlay_sync_system!(
    sync_cursor_overlay_system,
    super::resources::RenderEditorCursorVerticesRes,
    CursorOverlayPresentation
);
impl_overlay_sync_system!(
    sync_hitbox_overlay_system,
    super::resources::RenderEditorHitboxVerticesRes,
    HitboxOverlayPresentation
);
impl_overlay_sync_system!(
    sync_tap_indicator_overlay_system,
    super::resources::RenderEditorTapIndicatorVerticesRes,
    TapIndicatorOverlayPresentation
);
impl_overlay_sync_system!(
    sync_transform_trigger_marker_overlay_system,
    super::resources::RenderEditorTransformTriggerMarkerVerticesRes,
    TransformTriggerMarkerPresentation
);
impl_overlay_sync_system!(
    sync_block_effect_overlay_system,
    super::resources::RenderBlockEffectVerticesRes,
    BlockEffectOverlayPresentation
);
impl_overlay_sync_system!(
    sync_practice_checkpoint_overlay_system,
    super::resources::RenderPracticeCheckpointVerticesRes,
    PracticeCheckpointOverlayPresentation
);
impl_overlay_sync_system!(
    sync_spawn_marker_overlay_system,
    super::resources::RenderEditorSpawnMarkerVerticesRes,
    SpawnMarkerOverlayPresentation
);
impl_overlay_sync_system!(
    sync_gizmo_overlay_system,
    super::resources::RenderEditorGizmoVerticesRes,
    GizmoOverlayPresentation
);

/// Sync selection overlay stencil and outline as separate retained entities.
///
/// The selection overlay has two geometry sets: stencil vertices (solid
/// fill behind the outline) and outline vertices. Both are rendered as
/// ordinary `Mesh3d` entities using the shared block material.
fn sync_selection_overlay_system(
    phase: Res<State<AppPhase>>,
    overlay: Res<super::resources::RenderEditorSelectionOverlayRes>,
    material: Res<BlockPresentationMaterial>,
    mut commands: Commands,
    mut mesh_assets: ResMut<Assets<Mesh>>,
    stencil_entities: Query<(Entity, Option<&Mesh3d>), With<SelectionStencilPresentation>>,
    outline_entities: Query<(Entity, Option<&Mesh3d>), With<SelectionOutlinePresentation>>,
) {
    if !overlay.is_changed() && !material.is_changed() && !phase.is_changed() {
        return;
    }
    let is_editor = *phase.get() == AppPhase::Editor;
    let Some(material_handle) = material.0.clone() else {
        return;
    };

    for (entity, mesh) in stencil_entities.iter() {
        commands.entity(entity).despawn();
        if let Some(mesh) = mesh {
            mesh_assets.remove(&mesh.0);
        }
    }

    // Outline.
    if is_editor && !overlay.outline_vertices.is_empty() {
        let mesh = mesh_assets.add(eg_mesh::vertices_to_bevy_mesh(&overlay.outline_vertices));
        if let Some((entity, old_mesh)) = outline_entities.iter().next() {
            commands
                .entity(entity)
                .insert((Mesh3d(mesh), MeshMaterial3d(material_handle)));
            if let Some(old) = old_mesh {
                mesh_assets.remove(&old.0);
            }
        } else {
            commands.spawn((
                SelectionOutlinePresentation,
                Mesh3d(mesh),
                MeshMaterial3d(material_handle),
            ));
        }
    } else {
        for (entity, mesh) in outline_entities.iter() {
            commands.entity(entity).despawn();
            if let Some(mesh) = mesh {
                mesh_assets.remove(&mesh.0);
            }
        }
    }
}

/// Sync hover overlay stencil and outline as separate retained entities.
fn sync_hover_overlay_system(
    phase: Res<State<AppPhase>>,
    overlay: Res<super::resources::RenderEditorHoverOverlayRes>,
    material: Res<BlockPresentationMaterial>,
    mut commands: Commands,
    mut mesh_assets: ResMut<Assets<Mesh>>,
    stencil_entities: Query<(Entity, Option<&Mesh3d>), With<HoverStencilPresentation>>,
    outline_entities: Query<(Entity, Option<&Mesh3d>), With<HoverOutlinePresentation>>,
) {
    if !overlay.is_changed() && !material.is_changed() && !phase.is_changed() {
        return;
    }
    let is_editor = *phase.get() == AppPhase::Editor;
    let Some(material_handle) = material.0.clone() else {
        return;
    };

    for (entity, mesh) in stencil_entities.iter() {
        commands.entity(entity).despawn();
        if let Some(mesh) = mesh {
            mesh_assets.remove(&mesh.0);
        }
    }

    // Outline.
    if is_editor && !overlay.outline_vertices.is_empty() {
        let mesh = mesh_assets.add(eg_mesh::vertices_to_bevy_mesh(&overlay.outline_vertices));
        if let Some((entity, old_mesh)) = outline_entities.iter().next() {
            commands
                .entity(entity)
                .insert((Mesh3d(mesh), MeshMaterial3d(material_handle)));
            if let Some(old) = old_mesh {
                mesh_assets.remove(&old.0);
            }
        } else {
            commands.spawn((
                HoverOutlinePresentation,
                Mesh3d(mesh),
                MeshMaterial3d(material_handle),
            ));
        }
    } else {
        for (entity, mesh) in outline_entities.iter() {
            commands.entity(entity).despawn();
            if let Some(mesh) = mesh {
                mesh_assets.remove(&mesh.0);
            }
        }
    }
}

/// Plugin that owns all overlay presentation entities.
///
/// This extends `BlockPresentationPlugin` to also cover trails, cursor,
/// gizmo, selection/hover outlines, tap indicators, transform triggers,
/// camera arrow, spawn marker, practice checkpoints, block effects,
/// and hitbox visualisation — every overlay type that was previously
/// rendered through the custom wgpu mesh-slot path.
pub struct OverlayPresentationPlugin;

impl bevy::app::Plugin for OverlayPresentationPlugin {
    fn build(&self, app: &mut bevy::app::App) {
        use super::plugin as render_plugin;
        app.add_systems(
            bevy::app::PostUpdate,
            (
                sync_trail_overlay_system.after(render_plugin::extract_trail_vertices_system),
                sync_camera_arrow_overlay_system
                    .after(render_plugin::extract_editor_camera_arrow_vertices_system),
                sync_cursor_overlay_system
                    .after(render_plugin::extract_editor_cursor_vertices_system),
                sync_hitbox_overlay_system
                    .after(render_plugin::extract_editor_hitbox_vertices_system),
                sync_tap_indicator_overlay_system
                    .after(render_plugin::extract_editor_tap_indicator_vertices_system),
                sync_selection_overlay_system
                    .after(render_plugin::extract_editor_selection_overlays_system),
                sync_hover_overlay_system
                    .after(render_plugin::extract_editor_selection_overlays_system),
                sync_transform_trigger_marker_overlay_system
                    .after(render_plugin::extract_editor_transform_trigger_marker_vertices_system),
                sync_block_effect_overlay_system
                    .after(render_plugin::extract_block_effect_vertices_system),
                sync_practice_checkpoint_overlay_system
                    .after(render_plugin::extract_practice_checkpoint_vertices_system),
                sync_spawn_marker_overlay_system
                    .after(render_plugin::extract_editor_spawn_marker_vertices_system),
                sync_gizmo_overlay_system
                    .after(render_plugin::extract_editor_gizmo_vertices_system),
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use bevy::{
        app::App,
        asset::{AssetApp, AssetPlugin, Assets},
        camera::ClearColor,
        mesh::Mesh3d,
        prelude::{Color, Transform, Vec3},
        state::prelude::State,
    };
    use eg_mesh::{
        BlockInstanceBuildResult, BlockMeshChunk, ChunkAabb, ChunkBuildStats, ChunkedBlockMesh,
        MeshGeometry, RenderChunkKey, Vertex,
    };
    use eg_types::AppPhase;

    use super::{
        BlockMaterial, BlockPresentationCamera, BlockPresentationCameraPlugin,
        BlockPresentationEnvironmentPlugin, BlockPresentationMaterial, BlockPresentationPlugin,
        EditorBlockChunkPresentation, MenuPreviewBlockChunkPresentation,
        PlayingBlockChunkPresentation, PlayingDynamicBlockPresentation, RenderCameraPoseRes,
        RenderEditorBlockMeshesRes, RenderFrameInputRes, RenderMenuPreviewBlockMeshesRes,
        RenderPlayingBlockMeshesRes,
    };

    #[test]
    fn changed_editor_chunk_extraction_creates_one_retained_mesh_entity_per_chunk() {
        let mut app = App::new();
        app.add_plugins(AssetPlugin::default())
            .init_asset::<bevy::mesh::Mesh>()
            .init_asset::<BlockMaterial>()
            .add_plugins(BlockPresentationPlugin);
        app.world_mut()
            .insert_resource(State::new(AppPhase::Editor));
        let material = app
            .world_mut()
            .resource_mut::<Assets<BlockMaterial>>()
            .add(BlockMaterial {
                texture_array: Default::default(),
            });
        app.world_mut()
            .resource_mut::<BlockPresentationMaterial>()
            .0 = Some(material);
        app.world_mut().insert_resource(RenderEditorBlockMeshesRes {
            static_mesh: ChunkedBlockMesh {
                chunks: vec![BlockMeshChunk {
                    key: RenderChunkKey { x: 2, y: 0, z: -1 },
                    aabb: ChunkAabb {
                        min: [2.0, 0.0, -1.0],
                        max: [3.0, 1.0, 0.0],
                    },
                    greedy_geometry: MeshGeometry::from_vertices(vec![Vertex::untextured(
                        [2.0, 0.0, -1.0],
                        [1.0, 1.0, 1.0, 1.0],
                    )]),
                    fallback: Default::default(),
                    stats: ChunkBuildStats::default(),
                }],
                stats: ChunkBuildStats::default(),
            },
            selected_geometry: MeshGeometry::default(),
        });

        app.update();

        let mut chunks = app
            .world_mut()
            .query::<(&EditorBlockChunkPresentation, &Mesh3d)>();
        let collected = chunks.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].0.key, RenderChunkKey { x: 2, y: 0, z: -1 });
        let mesh = collected[0].1.0.clone();
        assert!(
            app.world()
                .resource::<Assets<bevy::mesh::Mesh>>()
                .get(&mesh)
                .is_some()
        );

        app.world_mut()
            .insert_resource(RenderEditorBlockMeshesRes::default());
        app.update();

        let chunk_count = app
            .world_mut()
            .query::<&EditorBlockChunkPresentation>()
            .iter(app.world())
            .count();
        assert_eq!(chunk_count, 0);
        assert!(
            app.world()
                .resource::<Assets<bevy::mesh::Mesh>>()
                .get(&mesh)
                .is_none(),
            "despawning a retained chunk must release its unshared mesh asset"
        );
    }

    #[test]
    fn editor_chunk_change_reuses_unchanged_mesh_assets() {
        let first_key = RenderChunkKey { x: 0, y: 0, z: 0 };
        let changed_key = RenderChunkKey { x: 1, y: 0, z: 0 };
        let mut app = App::new();
        app.add_plugins(AssetPlugin::default())
            .init_asset::<bevy::mesh::Mesh>()
            .init_asset::<BlockMaterial>()
            .add_plugins(BlockPresentationPlugin);
        app.world_mut()
            .insert_resource(State::new(AppPhase::Editor));
        let material = app
            .world_mut()
            .resource_mut::<Assets<BlockMaterial>>()
            .add(BlockMaterial {
                texture_array: Default::default(),
            });
        app.world_mut()
            .resource_mut::<BlockPresentationMaterial>()
            .0 = Some(material);
        app.world_mut().insert_resource(RenderEditorBlockMeshesRes {
            static_mesh: ChunkedBlockMesh {
                chunks: vec![test_chunk(first_key, 0.0), test_chunk(changed_key, 0.0)],
                stats: ChunkBuildStats::default(),
            },
            selected_geometry: MeshGeometry::default(),
        });
        app.update();

        let original_meshes = editor_chunk_meshes(&mut app);
        let original_unchanged = mesh_for_key(&original_meshes, first_key);
        let original_changed = mesh_for_key(&original_meshes, changed_key);

        app.world_mut().insert_resource(RenderEditorBlockMeshesRes {
            static_mesh: ChunkedBlockMesh {
                chunks: vec![test_chunk(first_key, 0.0), test_chunk(changed_key, 0.25)],
                stats: ChunkBuildStats::default(),
            },
            selected_geometry: MeshGeometry::default(),
        });
        app.update();

        let updated_meshes = editor_chunk_meshes(&mut app);
        assert_eq!(mesh_for_key(&updated_meshes, first_key), original_unchanged);
        assert_ne!(mesh_for_key(&updated_meshes, changed_key), original_changed);
        assert!(
            app.world()
                .resource::<Assets<bevy::mesh::Mesh>>()
                .get(&original_changed)
                .is_none(),
            "rebuilding an affected chunk must release only its previous mesh"
        );
    }

    #[test]
    fn static_menu_and_gameplay_chunks_are_retained_only_for_the_active_phase() {
        let mut app = App::new();
        app.add_plugins(AssetPlugin::default())
            .init_asset::<bevy::mesh::Mesh>()
            .init_asset::<BlockMaterial>()
            .add_plugins(BlockPresentationPlugin);
        let material = app
            .world_mut()
            .resource_mut::<Assets<BlockMaterial>>()
            .add(BlockMaterial {
                texture_array: Default::default(),
            });
        app.world_mut()
            .resource_mut::<BlockPresentationMaterial>()
            .0 = Some(material);
        app.world_mut().insert_resource(State::new(AppPhase::Menu));
        app.world_mut()
            .insert_resource(RenderMenuPreviewBlockMeshesRes {
                preview_level_index: Some(0),
                chunked: Some(single_chunked_mesh(RenderChunkKey { x: 0, y: 0, z: 0 })),
            });

        app.update();
        let menu_chunks = app
            .world_mut()
            .query::<&MenuPreviewBlockChunkPresentation>()
            .iter(app.world())
            .count();
        assert_eq!(menu_chunks, 1);

        app.world_mut()
            .insert_resource(State::new(AppPhase::Playing));
        app.world_mut()
            .insert_resource(RenderPlayingBlockMeshesRes {
                result: None,
                chunked: Some(single_chunked_mesh(RenderChunkKey { x: 1, y: 0, z: 0 })),
            });
        app.update();

        let menu_chunks = app
            .world_mut()
            .query::<&MenuPreviewBlockChunkPresentation>()
            .iter(app.world())
            .count();
        let playing_chunks = app
            .world_mut()
            .query::<&PlayingBlockChunkPresentation>()
            .iter(app.world())
            .count();
        assert_eq!(menu_chunks, 0);
        assert_eq!(playing_chunks, 1);

        app.world_mut()
            .insert_resource(RenderPlayingBlockMeshesRes {
                result: Some(BlockInstanceBuildResult {
                    batches: Vec::new(),
                    fallback_geometry: MeshGeometry::from_vertices(vec![Vertex::untextured(
                        [2.0, 0.0, 0.0],
                        [1.0, 1.0, 1.0, 1.0],
                    )]),
                }),
                chunked: None,
            });
        app.update();

        let mut dynamic_blocks = app
            .world_mut()
            .query::<(&PlayingDynamicBlockPresentation, &Mesh3d)>();
        let dynamic_blocks = dynamic_blocks.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(dynamic_blocks.len(), 1);
        let dynamic_mesh = dynamic_blocks[0].1.0.clone();

        app.world_mut().insert_resource(State::new(AppPhase::Menu));
        app.update();

        let dynamic_blocks = app
            .world_mut()
            .query::<&PlayingDynamicBlockPresentation>()
            .iter(app.world())
            .count();
        assert_eq!(dynamic_blocks, 0);
        assert!(
            app.world()
                .resource::<Assets<bevy::mesh::Mesh>>()
                .get(&dynamic_mesh)
                .is_none(),
            "leaving gameplay must release the dynamic mesh asset"
        );
    }

    #[test]
    fn retained_camera_follows_extracted_app_world_pose() {
        let mut app = App::new();
        app.init_resource::<RenderCameraPoseRes>()
            .add_plugins(BlockPresentationCameraPlugin);
        app.update();

        let eye = Vec3::new(4.0, 8.0, -12.0);
        let target = Vec3::new(1.0, 2.0, 3.0);
        app.world_mut().insert_resource(RenderCameraPoseRes {
            eye: eye.to_array(),
            target: target.to_array(),
        });
        app.update();

        let mut cameras = app
            .world_mut()
            .query::<(&BlockPresentationCamera, &Transform)>();
        let (_, transform) = cameras.single(app.world()).expect("one retained camera");
        assert_eq!(transform.translation, eye);
        let expected = Transform::from_translation(eye).looking_at(target, Vec3::Y);
        assert!(transform.rotation.angle_between(expected.rotation) < 1e-6);
    }

    #[test]
    fn retained_scene_clear_color_follows_extracted_sky_color() {
        let mut app = App::new();
        app.init_resource::<RenderFrameInputRes>()
            .add_plugins(BlockPresentationEnvironmentPlugin);
        app.world_mut()
            .resource_mut::<RenderFrameInputRes>()
            .0
            .sky_color = [0.2, 0.4, 0.6];

        app.update();

        assert_eq!(
            app.world().resource::<ClearColor>().0,
            Color::srgb(0.2, 0.4, 0.6)
        );
    }

    fn single_chunked_mesh(key: RenderChunkKey) -> ChunkedBlockMesh {
        ChunkedBlockMesh {
            chunks: vec![test_chunk(key, 0.0)],
            stats: ChunkBuildStats::default(),
        }
    }

    fn test_chunk(key: RenderChunkKey, x_offset: f32) -> BlockMeshChunk {
        BlockMeshChunk {
            key,
            aabb: ChunkAabb {
                min: [key.x as f32, 0.0, key.z as f32],
                max: [key.x as f32 + 1.0, 1.0, key.z as f32 + 1.0],
            },
            greedy_geometry: MeshGeometry::from_vertices(vec![Vertex::untextured(
                [key.x as f32 + x_offset, 0.0, key.z as f32],
                [1.0, 1.0, 1.0, 1.0],
            )]),
            fallback: Default::default(),
            stats: ChunkBuildStats::default(),
        }
    }

    fn editor_chunk_meshes(
        app: &mut App,
    ) -> Vec<(RenderChunkKey, bevy::asset::Handle<bevy::mesh::Mesh>)> {
        let world = app.world_mut();
        let mut query = world.query::<(&EditorBlockChunkPresentation, &Mesh3d)>();
        query
            .iter(world)
            .map(|(presentation, mesh)| (presentation.key, mesh.0.clone()))
            .collect()
    }

    fn mesh_for_key(
        meshes: &[(RenderChunkKey, bevy::asset::Handle<bevy::mesh::Mesh>)],
        key: RenderChunkKey,
    ) -> bevy::asset::Handle<bevy::mesh::Mesh> {
        meshes
            .iter()
            .find_map(|(candidate, mesh)| (*candidate == key).then(|| mesh.clone()))
            .expect("chunk mesh must exist")
    }
}
