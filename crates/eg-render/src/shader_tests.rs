/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use naga::front::wgsl;
use naga::valid::{Capabilities, ValidationFlags, Validator};
use naga::{Block, Statement};

use super::{GRID_WGSL, SHADER_WGSL};

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
    let module = wgsl::parse_str(SHADER_WGSL).expect("shader.wgsl should parse");

    Validator::new(ValidationFlags::all(), Capabilities::all())
        .validate(&module)
        .expect("shader.wgsl should pass naga validation");
}

#[test]
fn grid_shader_wgsl_parses_and_validates_with_naga() {
    let module = wgsl::parse_str(GRID_WGSL).expect("grid.wgsl should parse");

    Validator::new(ValidationFlags::all(), Capabilities::all())
        .validate(&module)
        .expect("grid.wgsl should pass naga validation");
}

#[test]
fn texture_sample_stays_outside_fragment_conditionals() {
    let fs_source = fs_main_source(SHADER_WGSL);
    let sample_pos = fs_source
        .find("textureSample(")
        .expect("fs_main must sample block textures");

    let before_sample = &fs_source[..sample_pos];
    assert!(
        !before_sample.contains("if ("),
        "textureSample must remain outside conditional control flow to satisfy strict WebGPU validators"
    );
}

fn block_contains_discard(block: &Block) -> bool {
    for stmt in block.iter() {
        match stmt {
            Statement::Kill => return true,
            Statement::Block(inner) => {
                if block_contains_discard(inner) {
                    return true;
                }
            }
            Statement::If { accept, reject, .. } => {
                if block_contains_discard(accept) || block_contains_discard(reject) {
                    return true;
                }
            }
            Statement::Switch { cases, .. } => {
                for case in cases.iter() {
                    if block_contains_discard(&case.body) {
                        return true;
                    }
                }
            }
            Statement::Loop {
                body, continuing, ..
            } => {
                if block_contains_discard(body) || block_contains_discard(continuing) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

#[test]
fn fragment_shader_discards_zero_alpha_fragments() {
    let module = wgsl::parse_str(SHADER_WGSL).expect("shader.wgsl should parse");

    Validator::new(ValidationFlags::all(), Capabilities::all())
        .validate(&module)
        .expect("shader.wgsl should pass naga validation");

    let fs_main = module
        .entry_points
        .iter()
        .find(|ep| ep.name == "fs_main")
        .expect("shader must have fs_main entry point");

    assert!(
        block_contains_discard(&fs_main.function.body),
        "fs_main must contain a discard statement to prevent zero-alpha fragments from writing to the depth buffer"
    );
}
