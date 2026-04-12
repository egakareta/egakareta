/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/

#![cfg(not(target_arch = "wasm32"))]

use std::path::PathBuf;
use std::process::{Command, Stdio};

fn get_egakareta_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_egakareta"))
}

#[test]
#[ignore = "launches a real native window; run explicitly for E2E smoke validation"]
fn native_binary_launches_and_exits_after_first_frame() {
    let output = Command::new(get_egakareta_path())
        .env("EGAKARETA_NATIVE_SMOKE_EXIT_AFTER_FIRST_REDRAW", "1")
        .env("RUST_LOG", "warn")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .expect("Failed to execute egakareta binary");

    assert!(
        output.status.success(),
        "Native launch smoke test failed with status {:?}. Stderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}
