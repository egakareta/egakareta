/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
pub(crate) fn apply_lighting(base_color: [f32; 4], normal: [f32; 3]) -> [f32; 4] {
    let light_dir = [0.57735, 0.57735, 0.57735];
    let ambient = 0.6;
    let diffuse = 0.4;

    let dot = normal[0] * light_dir[0] + normal[1] * light_dir[1] + normal[2] * light_dir[2];
    let intensity = ambient + diffuse * dot.max(0.0);

    [
        (base_color[0] * intensity).min(1.0),
        (base_color[1] * intensity).min(1.0),
        (base_color[2] * intensity).min(1.0),
        base_color[3],
    ]
}
