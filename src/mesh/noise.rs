/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
pub(crate) fn pseudo_random_noise(x: f32, y: f32, z: f32) -> f32 {
    let seed = ((x as i32).wrapping_mul(73856093)
        ^ (y as i32).wrapping_mul(19349663)
        ^ (z as i32).wrapping_mul(83492791)) as u32;
    let mut h = seed;
    h = (h ^ (h >> 13)).wrapping_mul(0x5bd1e995);
    (h ^ (h >> 15)) as f32 / 4294967295.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determinism() {
        let x = 1.23;
        let y = 4.56;
        let z = 7.89;
        let val1 = pseudo_random_noise(x, y, z);
        let val2 = pseudo_random_noise(x, y, z);
        assert_eq!(val1, val2);
    }

    #[test]
    fn test_range() {
        for i in -100..100 {
            for j in -100..100 {
                let val = pseudo_random_noise(i as f32, j as f32, 0.0);
                assert!(
                    (0.0..=1.0).contains(&val),
                    "Value {} at ({}, {}, {}) is out of range",
                    val,
                    i,
                    j,
                    0.0
                );
            }
        }
    }

    #[test]
    fn test_different_values() {
        let val1 = pseudo_random_noise(0.0, 0.0, 0.0);
        let val2 = pseudo_random_noise(1.0, 0.0, 0.0);
        let val3 = pseudo_random_noise(0.0, 1.0, 0.0);
        let val4 = pseudo_random_noise(0.0, 0.0, 1.0);

        assert_ne!(val1, val2);
        assert_ne!(val1, val3);
        assert_ne!(val1, val4);
    }

    #[test]
    fn test_negative_values() {
        let val = pseudo_random_noise(-1.0, -2.0, -3.0);
        assert!((0.0..=1.0).contains(&val));
    }

    #[test]
    fn test_grid_behavior() {
        // Since it casts to i32, values within the same integer unit should be the same
        let val1 = pseudo_random_noise(1.1, 2.2, 3.3);
        let val2 = pseudo_random_noise(1.9, 2.9, 3.9);
        assert_eq!(val1, val2);

        let val3 = pseudo_random_noise(-1.1, -2.2, -3.3);
        let val4 = pseudo_random_noise(-1.9, -2.9, -3.9);
        assert_eq!(val3, val4);
    }
}
