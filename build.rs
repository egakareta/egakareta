/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
use std::collections::HashSet;
use std::path::Path;

// Whitelist of keys to bake into the binary
const BAKE_KEYS: &[&str] = &["API_URL", "PUBLISHABLE_KEY"];

fn main() {
    println!("cargo:rerun-if-changed=.env.local");
    println!("cargo:rerun-if-changed=.env.preview");
    println!("cargo:rerun-if-changed=.env.production");
    println!("cargo:rerun-if-env-changed=BUILD_ENV");
    println!("cargo:rerun-if-changed=assets/levels");

    let build_env = std::env::var("BUILD_ENV").unwrap_or_else(|_| "local".to_string());

    let env_file = match build_env.as_str() {
        "preview" => ".env.preview",
        "production" => ".env.production",
        _ => ".env.local",
    };

    println!("cargo:warning=Loading env from: {}", env_file);

    let allowed: HashSet<&str> = BAKE_KEYS.iter().copied().collect();

    let contents = match std::fs::read_to_string(env_file) {
        Ok(c) => c,
        Err(_) => {
            println!("cargo:warning=Env file not found, skipping environment variable baking");
            return;
        }
    };

    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let key = key.trim();
            let value = value.trim();
            if allowed.contains(key) {
                println!("cargo:rustc-env={}={}", key, value);
            }
        }
    }

    let levels_dir = Path::new("assets/levels");
    if levels_dir.is_dir() {
        let level_count = std::fs::read_dir(levels_dir)
            .map(|entries| {
                entries
                    .filter(|e| e.as_ref().map(|e| e.path().is_dir()).unwrap_or(false))
                    .count()
            })
            .unwrap_or(0);
        println!("cargo:warning=Using {} levels", level_count);
    }
}
