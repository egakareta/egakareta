/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/

use crate::State;

pub(crate) fn approx_eq(a: f32, b: f32, eps: f32) -> bool {
    (a - b).abs() <= eps
}

pub(crate) fn assert_approx_eq(a: f32, b: f32, eps: f32) {
    assert!(approx_eq(a, b, eps), "expected {a} ~= {b}");
}

pub(crate) fn with_test_state(mut test: impl FnMut(&mut State)) {
    pollster::block_on(async {
        let Some(mut state) = State::new_test().await else {
            return;
        };
        test(&mut state);
    });
}

pub(crate) fn with_editor_test_state(mut test: impl FnMut(&mut State)) {
    with_test_state(|state| {
        if state.is_splash() {
            state.turn_right();
        }
        if state.is_menu() {
            state.toggle_editor();
        }
        test(state);
    });
}
