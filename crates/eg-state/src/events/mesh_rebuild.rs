/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Mesh rebuild events (Phase 4d).
//!
//! The plan calls for replacing the legacy `EditorDirtyFlags`
//! bitfield with Bevy `Changed<Position|Rotation|Size>` change
//! detection. A Bevy system in the `RebuildMeshSet` reads the
//! change-set each frame and emits a [`RebuildMeshEvent`] per dirty
//! entity. The actual mesh rebuild logic stays on the legacy
//! `State::process_editor_dirty` for now — the event bus establishes
//! the boundary so a future commit can move the rebuild into a
//! Bevy system that reads the world resources directly.
//!
//! ## Why messages and not direct resource writes?
//!
//! - The event is a **signal** to the rebuild consumer, not a
//!   command. The consumer (today: legacy
//!   `State::process_editor_dirty`; future: a Bevy system) decides
//!   what to rebuild based on the current world state.
//! - The event is **decoupled** from the change detection. The
//!   Bevy system that detects `Changed<>` may run before or after
//!   the rebuild consumer; the event queue decouples them.
//! - The event is **observable** for tests + profiling. A test
//!   can write a synthetic `RebuildMeshEvent` and assert the
//!   consumer behavior without going through the editor
//!   dirty-flag path.
//!
//! REFACTOR POINT(full-bevy): the change-detection system is
//! replaced by `bevy_render`'s render-world `extract` step in the
//! full-Bevy migration. The `RebuildMeshEvent` event itself stays
//! but the source changes (Bevy's render graph emits it from
//! `Mesh` / `Material` asset events).

use bevy_ecs::prelude::*;

/// Reason a mesh needs to be rebuilt.
///
/// Maps to the categories of work that the legacy
/// `EditorDirtyFlags` bitfield tracks. A `RebuildMeshEvent` carries
/// one or more of these reasons so the consumer can choose what
/// to rebuild.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RebuildMeshEvent {
    /// Which mesh category needs to be rebuilt.
    pub kind: RebuildMeshKind,
}

/// The categories of mesh rebuild work.
///
/// Each variant corresponds to a bit on the legacy
/// `EditorDirtyFlags`. A Bevy system in the `RebuildMeshSet`
/// translates `Changed<Position|Rotation|Size>` into one or more
/// of these variants per dirty entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RebuildMeshKind {
    /// Sync the gameplay object list (no GPU work; the legacy
    /// `sync_game_objects` bit).
    SyncGameObjects,
    /// Rebuild the entire block mesh (the legacy
    /// `rebuild_block_mesh` bit).
    RebuildBlockMesh,
    /// Append a new chunk to the block mesh (the legacy
    /// `append_block_mesh` bit).
    AppendBlockMesh,
    /// Rebuild the selection outline + gizmo (the legacy
    /// `rebuild_selection_overlays` bit).
    RebuildSelectionOverlays,
    /// Rebuild the tap indicators (the legacy
    /// `rebuild_tap_indicators` bit).
    RebuildTapIndicators,
    /// Rebuild the transform trigger markers (the legacy
    /// `rebuild_transform_trigger_markers` bit).
    RebuildTransformTriggerMarkers,
    /// Rebuild the editor preview player (the legacy
    /// `rebuild_preview_player` bit).
    RebuildPreviewPlayer,
    /// Rebuild the editor cursor (the legacy `rebuild_cursor` bit).
    RebuildCursor,
    /// Rebuild the hitbox visualization (the legacy
    /// `rebuild_hitbox_visualization` bit).
    RebuildHitboxVisualization,
}

impl RebuildMeshEvent {
    /// Construct a new event.
    pub fn new(kind: RebuildMeshKind) -> Self {
        Self { kind }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rebuild_mesh_event_carries_kind() {
        let event = RebuildMeshEvent::new(RebuildMeshKind::RebuildBlockMesh);
        assert_eq!(event.kind, RebuildMeshKind::RebuildBlockMesh);
    }

    #[test]
    fn rebuild_mesh_kinds_distinct() {
        // Pin that the variants are all distinct so a
        // match-on-kind switch stays exhaustive.
        let kinds = [
            RebuildMeshKind::SyncGameObjects,
            RebuildMeshKind::RebuildBlockMesh,
            RebuildMeshKind::AppendBlockMesh,
            RebuildMeshKind::RebuildSelectionOverlays,
            RebuildMeshKind::RebuildTapIndicators,
            RebuildMeshKind::RebuildTransformTriggerMarkers,
            RebuildMeshKind::RebuildPreviewPlayer,
            RebuildMeshKind::RebuildCursor,
            RebuildMeshKind::RebuildHitboxVisualization,
        ];
        for i in 0..kinds.len() {
            for j in (i + 1)..kinds.len() {
                assert_ne!(kinds[i], kinds[j], "kinds[{}] == kinds[{}]", i, j);
            }
        }
    }
}
