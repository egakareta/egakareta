/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    let root = PathBuf::from(
        std::env::var("EGAKARETA_WORKSPACE_ROOT")
            .expect("EGAKARETA_WORKSPACE_ROOT should be set by .cargo/config.toml"),
    );
    root.canonicalize().unwrap_or(root)
}

fn main() {
    println!("cargo:rerun-if-env-changed=EGAKARETA_WORKSPACE_ROOT");
    println!(
        "cargo:rerun-if-changed={}",
        &workspace_root().join("assets/blocks").display()
    );
}
