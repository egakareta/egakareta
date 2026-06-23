/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/

// ── Test fixture helpers ────────────────────────────────────────────────

use eg_types::LevelObject;

/// A stone block at `(x, y, z)` with default size and no rotation.
pub fn stone(x: f32, y: f32, z: f32) -> LevelObject {
    LevelObject {
        block_id: "core/stone".to_string(),
        id: format!("object_{x:.0}_{y:.0}_{z:.0}"),
        position: [x, y, z],
        ..LevelObject::default()
    }
}

/// A block with the given `block_id` at `(x, y, z)`.
pub fn block(block_id: &str, x: f32, y: f32, z: f32) -> LevelObject {
    LevelObject {
        id: format!("object_{x:.0}_{y:.0}_{z:.0}"),
        block_id: block_id.to_string(),
        position: [x, y, z],
        ..LevelObject::default()
    }
}

/// A gem block at `(x, y, z)`.
pub fn gem(x: f32, y: f32, z: f32) -> LevelObject {
    block("core/gem", x, y, z)
}

/// A block at `(x, y, z)` with custom `size`.
pub fn sized(id: &str, x: f32, y: f32, z: f32, sx: f32, sy: f32, sz: f32) -> LevelObject {
    LevelObject {
        id: format!("object_{x:.0}_{y:.0}_{z:.0}"),
        block_id: id.to_string(),
        position: [x, y, z],
        size: [sx, sy, sz],
        ..LevelObject::default()
    }
}

/// A block at `(x, y, z)` with custom rotation (degrees).
pub fn rotated(id: &str, x: f32, y: f32, z: f32, rx: f32, ry: f32, rz: f32) -> LevelObject {
    LevelObject {
        id: format!("object_{x:.0}_{y:.0}_{z:.0}"),
        block_id: id.to_string(),
        position: [x, y, z],
        rotation_degrees: [rx, ry, rz],
        ..LevelObject::default()
    }
}

pub fn tiny_wav_bytes() -> Vec<u8> {
    vec![
        b'R', b'I', b'F', b'F', 38, 0, 0, 0, b'W', b'A', b'V', b'E', b'f', b'm', b't', b' ', 16, 0,
        0, 0, 1, 0, 1, 0, 68, 172, 0, 0, 136, 88, 1, 0, 2, 0, 16, 0, b'd', b'a', b't', b'a', 2, 0,
        0, 0, 0, 0,
    ]
}

/// Ergonomic macro for editor tests. Eliminates `tokio::test` + `async fn` +
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
#[allow(unused_macros)]
macro_rules! editor_test {
    ($name:ident, $body:expr) => {
        #[tokio::test]
        async fn $name() {
            let mut state = crate::state::State::new_test().await;
            state.phase = eg_types::AppPhase::Editor;
            state.editor.ui.mode = eg_types::EditorMode::Select;
            let body: &dyn Fn(&mut crate::state::State) = &$body;
            body(&mut state);
        }
    };
}

#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use editor_test;

// ── Existing tests ──────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::{block, gem, rotated, sized, stone, tiny_wav_bytes};

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

    #[test]
    fn tiny_wav_fixture_is_valid_riff_data() {
        let bytes = tiny_wav_bytes();
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(&bytes[8..12], b"WAVE");
    }
}
