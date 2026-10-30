/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::{env, path::PathBuf};

/// Locate LLVM's resource compiler for MSVC cross-compilation.
fn find_llvm_rc() -> Option<PathBuf> {
    let exe = if cfg!(windows) {
        "llvm-rc.exe"
    } else {
        "llvm-rc"
    };
    if let Some(paths) = env::var_os("PATH") {
        for dir in env::split_paths(&paths) {
            let candidate = dir.join(exe);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    // Debian/Ubuntu keep versioned LLVM toolchains out of PATH.
    let mut versioned: Vec<(u32, PathBuf)> = std::fs::read_dir("/usr/lib")
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let dir = entry.path();
            let version: u32 = dir
                .file_name()?
                .to_str()?
                .strip_prefix("llvm-")?
                .parse()
                .ok()?;
            let candidate = dir.join("bin").join(exe);
            candidate.is_file().then_some((version, candidate))
        })
        .collect();
    versioned.sort_unstable_by_key(|(version, _)| *version);
    versioned.pop().map(|(_, candidate)| candidate)
}

fn main() {
    // Non-Windows builds have no external inputs; avoid Cargo's package-wide scan.
    println!("cargo:rerun-if-changed=build.rs");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    // winresource reads package metadata from the application manifest.
    println!("cargo:rerun-if-changed=Cargo.toml");
    for key in [
        "EGAKARETA_WORKSPACE_ROOT",
        "RC_PATH",
        "WINDRES",
        "AR",
        "CROSS_COMPILE",
        "PATH",
    ] {
        println!("cargo:rerun-if-env-changed={key}");
    }
    let workspace_root = PathBuf::from(
        env::var("EGAKARETA_WORKSPACE_ROOT")
            .expect("EGAKARETA_WORKSPACE_ROOT should be set by .cargo/config.toml"),
    );
    let workspace_root = workspace_root.canonicalize().unwrap_or(workspace_root);
    let favicon_path = workspace_root.join("assets/favicon.ico");
    println!("cargo:rerun-if-changed={}", favicon_path.display());

    let cross_compiling = env::var("HOST").ok() != env::var("TARGET").ok();
    if cross_compiling
        && env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
        && env::var_os("RC_PATH").is_none()
    {
        match find_llvm_rc() {
            Some(llvm_rc) => unsafe { env::set_var("RC_PATH", &llvm_rc) },
            None => {
                println!(
                    "cargo:warning=llvm-rc not found; skipping Windows icon/metadata resource. \
                     Install LLVM or set RC_PATH to enable it."
                );
                return;
            }
        }
    }

    winresource::WindowsResource::new()
        .set_icon(
            favicon_path
                .to_str()
                .expect("workspace asset paths should be valid UTF-8"),
        )
        .compile()
        .expect("Failed to compile Windows resource");
}
