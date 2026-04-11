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
