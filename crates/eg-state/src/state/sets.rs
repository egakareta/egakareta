/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! System set definitions for the eg-state `App`.
//!
//! These sets are the contract for ordering systems in the editor + play
//! pipeline. Today they are only declared; the actual systems that use them
//! will be wired up in subsequent phases (Phase 3+). The set graph mirrors
//! the per-frame flow in `update.rs`:
//!
//! 1. `InputSet` — drain platform `InputEvent`s into the `EditorInput` resource.
//! 2. `CommandSet` — process queued `AppCommand` / `EditorCommand` events.
//! 3. `PreCommandHistorySet` — snapshot editor state for undo/redo before
//!    mutating commands commit.
//! 4. `ModeChangeSet` — apply phase/mode transitions (`AppPhase`, `EditorMode`).
//! 5. `GameplaySet` — gameplay simulation (delegated to [`crate::state::plugins::GameplayPlugin`]).
//! 6. `EditorLogicSet` — editor mutation, selection, history, dirty flags.
//! 7. `TimelineSet` — timeline scrubbing, playback, tap/timing updates.
//! 8. `TriggerSet` — transform/camera trigger evaluation.
//! 9. `AudioBridgeSet` — drain audio channel events into resources.
//! 10. `RenderExtractSet` — snapshot world state into render resources.
//! 11. `RenderPresentSet` — issue the wgpu present (no-op for Bevy).
//! 12. `UiSet` — egui systems that read the world and write commands back.
//! 13. `RebuildMeshSet` — react to dirty flags (or `Changed<>` components
//!     in the full-Bevy migration) and rebuild mesh resources.

use bevy_ecs::schedule::SystemSet;

/// Ordered system sets for the eg-state `App`.
///
/// See module docs for the per-frame ordering. Add new sets at the end of
/// the chain — do not renumber existing variants; their positions encode
/// the current ordering contract used by the schedule graph.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StateSet {
    /// Drains platform `InputEvent`s into `EditorInput` resource. Runs at
    /// the top of the frame so all downstream systems see the latest input.
    Input,
    /// Drains queued semantic messages into direct handlers or the legacy
    /// command buffer while the compatibility bridge is still active.
    Command,
    /// Snapshots editor state for undo/redo before mutating commands commit.
    /// Runs before the editor mutation set so undo history reflects the
    /// pre-mutation state.
    PreCommandHistory,
    /// Applies phase/mode transitions (`AppPhase`, `EditorMode`). Runs
    /// after command processing so commands can change the phase, and
    /// before the editor mutation set so the new mode is in effect.
    ModeChange,
    /// Gameplay simulation step (delegates to `eg_game::GamePlugin`).
    /// No-op in the editor phase.
    Gameplay,
    /// Editor-side mutation (block placement, selection, history, dirty
    /// flags). Runs after the command set so the latest input is applied.
    EditorLogic,
    /// Timeline scrubbing, playback, tap/timing updates. Runs after the
    /// editor logic set so placement can update tap indicators.
    Timeline,
    /// Transform/camera trigger evaluation. Runs after the timeline so it
    /// sees the latest timeline time.
    Trigger,
    /// Drains audio channel events into `AudioStateRes`. Runs near the
    /// bottom of the frame so audio results from this frame can be used.
    AudioBridge,
    /// Snapshots world state into render resources (`SceneMeshesRes`).
    /// Runs after gameplay + timeline + triggers so the visual set is
    /// consistent with the current game time.
    RenderExtract,
    /// Issues the wgpu present. Currently lives outside Bevy; the set
    /// exists for ordering documentation.
    RenderPresent,
    /// egui systems that read the world and write commands back. Runs
    /// after the render extract set so the UI can show fresh data.
    Ui,
    /// Reacts to dirty flags (or `Changed<>` in full-Bevy) and rebuilds
    /// mesh resources. Runs at the end of the frame.
    RebuildMesh,
}
