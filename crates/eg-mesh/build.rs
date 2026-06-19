/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::fs;
use std::path::{Path, PathBuf};

fn workspace_root() -> PathBuf {
    let root = PathBuf::from(
        std::env::var("EGAKARETA_WORKSPACE_ROOT")
            .expect("EGAKARETA_WORKSPACE_ROOT should be set by .cargo/config.toml"),
    );
    root.canonicalize().unwrap_or(root)
}

fn emit_rerun_if_changed_recursive(path: &Path) {
    println!("cargo:rerun-if-changed={}", path.display());

    let Ok(entries) = fs::read_dir(path) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            emit_rerun_if_changed_recursive(&path);
        } else {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }
}

fn main() {
    println!("cargo:rerun-if-env-changed=EGAKARETA_WORKSPACE_ROOT");
    emit_rerun_if_changed_recursive(&workspace_root().join("assets/blocks"));
}
