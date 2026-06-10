/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/

pub(crate) fn approx_eq(a: f32, b: f32, eps: f32) -> bool {
    (a - b).abs() <= eps
}

pub(crate) fn assert_approx_eq(a: f32, b: f32, eps: f32) {
    assert!(approx_eq(a, b, eps), "expected {a} ~= {b}");
}

// ── Test fixture helpers ────────────────────────────────────────────────

use crate::types::LevelObject;

/// Default `LevelObject` with "core/stone" at the origin.
fn default_level_object() -> LevelObject {
    LevelObject {
        position: [0.0, 0.0, 0.0],
        size: [1.0, 1.0, 1.0],
        rotation_degrees: [0.0, 0.0, 0.0],
        block_id: "core/stone".to_string(),
        color_tint: [1.0, 1.0, 1.0],
        trigger: None,
    }
}

/// A stone block at `(x, y, z)` with default size and no rotation.
pub(crate) fn stone(x: f32, y: f32, z: f32) -> LevelObject {
    LevelObject {
        position: [x, y, z],
        ..default_level_object()
    }
}

/// A block with the given `id` at `(x, y, z)`.
pub(crate) fn block(id: &str, x: f32, y: f32, z: f32) -> LevelObject {
    LevelObject {
        block_id: id.to_string(),
        position: [x, y, z],
        ..default_level_object()
    }
}

/// A gem block at `(x, y, z)`.
pub(crate) fn gem(x: f32, y: f32, z: f32) -> LevelObject {
    block("core/gem", x, y, z)
}

/// A block at `(x, y, z)` with custom `size`.
pub(crate) fn sized(id: &str, x: f32, y: f32, z: f32, sx: f32, sy: f32, sz: f32) -> LevelObject {
    LevelObject {
        block_id: id.to_string(),
        position: [x, y, z],
        size: [sx, sy, sz],
        ..default_level_object()
    }
}

/// A block at `(x, y, z)` with custom rotation (degrees).
pub(crate) fn rotated(id: &str, x: f32, y: f32, z: f32, rx: f32, ry: f32, rz: f32) -> LevelObject {
    LevelObject {
        block_id: id.to_string(),
        position: [x, y, z],
        rotation_degrees: [rx, ry, rz],
        ..default_level_object()
    }
}

/// Ergonomic macro for editor tests. Eliminates `pollster::block_on` and
/// `State::new_test()` boilerplate.
///
/// The body receives `state: &mut State` already in `AppPhase::Editor` with
/// `EditorMode::Select`.
///
/// # Example
/// ```ignore
/// editor_test!(my_test, |state| {
///     state.editor.objects.push(stone(0, 0, 0));
///     assert_eq!(state.editor.objects.len(), 1);
/// });
/// ```
#[cfg(test)]
macro_rules! editor_test {
    ($name:ident, $body:expr) => {
        #[test]
        fn $name() {
            pollster::block_on(async {
                let mut state = crate::state::State::new_test().await;
                state.phase = crate::types::AppPhase::Editor;
                state.editor.ui.mode = crate::types::EditorMode::Select;
                let body: &dyn Fn(&mut crate::state::State) = &$body;
                body(&mut state);
            });
        }
    };
}

#[cfg(test)]
pub(crate) use editor_test;

// ── Existing tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::{approx_eq, assert_approx_eq, block, gem, rotated, sized, stone};

    #[test]
    fn approx_eq_returns_true_within_epsilon() {
        assert!(approx_eq(1.0, 1.000_5, 0.001));
    }

    #[test]
    fn approx_eq_returns_false_outside_epsilon() {
        assert!(!approx_eq(1.0, 1.01, 0.001));
    }

    #[test]
    fn assert_approx_eq_does_not_panic_within_epsilon() {
        assert_approx_eq(-42.0, -42.000_4, 0.001);
    }

    #[test]
    #[should_panic(expected = "expected 3 ~= 4")]
    fn assert_approx_eq_panics_outside_epsilon() {
        assert_approx_eq(3.0, 4.0, 0.1);
    }

    #[test]
    fn level_object_helpers_fill_expected_fields() {
        let stone = stone(1.0, 2.0, 3.0);
        assert_eq!(stone.block_id, "core/stone");
        assert_eq!(stone.position, [1.0, 2.0, 3.0]);
        assert_eq!(stone.size, [1.0, 1.0, 1.0]);

        let lava = block("core/lava", 4.0, 5.0, 6.0);
        assert_eq!(lava.block_id, "core/lava");
        assert_eq!(lava.position, [4.0, 5.0, 6.0]);

        let gem = gem(7.0, 8.0, 9.0);
        assert_eq!(gem.block_id, "core/gem");
        assert_eq!(gem.position, [7.0, 8.0, 9.0]);
    }

    #[test]
    fn sized_and_rotated_helpers_override_only_requested_fields() {
        let sized = sized("core/stone", 1.0, 2.0, 3.0, 4.0, 5.0, 6.0);
        assert_eq!(sized.position, [1.0, 2.0, 3.0]);
        assert_eq!(sized.size, [4.0, 5.0, 6.0]);
        assert_eq!(sized.rotation_degrees, [0.0, 0.0, 0.0]);

        let rotated = rotated("core/grass", 7.0, 8.0, 9.0, 10.0, 20.0, 30.0);
        assert_eq!(rotated.block_id, "core/grass");
        assert_eq!(rotated.position, [7.0, 8.0, 9.0]);
        assert_eq!(rotated.rotation_degrees, [10.0, 20.0, 30.0]);
        assert_eq!(rotated.size, [1.0, 1.0, 1.0]);
    }
}
