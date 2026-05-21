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

#[cfg(test)]
mod tests {
    use super::{approx_eq, assert_approx_eq};

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
}
