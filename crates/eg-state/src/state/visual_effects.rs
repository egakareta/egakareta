/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! App-world lifecycle for block visual effects.

use bevy_ecs::prelude::*;
use eg_types::AppPhase;

use super::{AppPhaseRes, BlockEffectsRes, EditorRuntimeRes, FrameTimeRes};

/// Consume newly produced effects and own their frame lifecycle.
pub(crate) fn update_block_effects_system(
    phase: Res<AppPhaseRes>,
    frame_time: Res<FrameTimeRes>,
    mut editor_runtime: ResMut<EditorRuntimeRes>,
    mut effects: ResMut<BlockEffectsRes>,
) {
    if phase.current == AppPhase::Menu {
        effects.effects.clear();
        editor_runtime.0.pending_block_effects.clear();
        return;
    }

    effects
        .effects
        .extend(std::mem::take(&mut editor_runtime.0.pending_block_effects));
    let frame_dt = frame_time.delta_seconds.max(0.0);
    for effect in &mut effects.effects {
        effect.age_seconds += frame_dt;
    }
    effects
        .effects
        .retain(|effect| effect.age_seconds < effect.effect.duration_seconds);
}
