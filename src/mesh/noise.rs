pub(crate) fn pseudo_random_noise(x: f32, y: f32, z: f32) -> f32 {
    let seed = ((x as i32).wrapping_mul(73856093)
        ^ (y as i32).wrapping_mul(19349663)
        ^ (z as i32).wrapping_mul(83492791)) as u32;
    let mut h = seed;
    h = (h ^ (h >> 13)).wrapping_mul(0x5bd1e995);
    (h ^ (h >> 15)) as f32 / 4294967295.0
}
