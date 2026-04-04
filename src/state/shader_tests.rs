/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
use naga::front::wgsl;
use naga::valid::{Capabilities, ValidationFlags, Validator};

fn fs_main_source(shader_source: &str) -> &str {
    let marker = "fn fs_main";
    let start = shader_source
        .find(marker)
        .expect("shader must contain fs_main");
    let tail = &shader_source[start..];

    let open_brace = tail.find('{').expect("fs_main must have body") + 1;
    let mut depth = 1usize;

    for (offset, ch) in tail[open_brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &tail[open_brace..open_brace + offset];
                }
            }
            _ => {}
        }
    }

    panic!("unterminated fs_main body");
}

#[test]
fn shader_wgsl_parses_and_validates_with_naga() {
    let shader_source = include_str!("../shader.wgsl");
    let module = wgsl::parse_str(shader_source).expect("shader.wgsl should parse");

    Validator::new(ValidationFlags::all(), Capabilities::all())
        .validate(&module)
        .expect("shader.wgsl should pass naga validation");
}

#[test]
fn texture_sample_stays_outside_fragment_conditionals() {
    let shader_source = include_str!("../shader.wgsl");
    let fs_source = fs_main_source(shader_source);
    let sample_pos = fs_source
        .find("textureSample(")
        .expect("fs_main must sample block textures");

    let before_sample = &fs_source[..sample_pos];
    assert!(
        !before_sample.contains("if ("),
        "textureSample must remain outside conditional control flow to satisfy strict WebGPU validators"
    );
}
