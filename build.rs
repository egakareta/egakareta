/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::collections::HashSet;
use std::path::Path;

fn load_wrangler_vars(build_env: &str) -> Vec<(String, String)> {
    let content = match std::fs::read_to_string("wrangler.jsonc") {
        Ok(c) => c,
        Err(_) => {
            println!("cargo:warning=wrangler.jsonc not found, skipping var baking");
            return vec![];
        }
    };

    // Strip block and line comments
    let stripped = strip_jsonc_comments(&content);

    let json: serde_json::Value = match serde_json::from_str(&stripped) {
        Ok(v) => v,
        Err(e) => {
            println!("cargo:warning=Failed to parse wrangler.jsonc: {}", e);
            return vec![];
        }
    };

    let vars = json
        .pointer(&format!("/env/{}/vars", build_env))
        .or_else(|| json.pointer("/vars")); // fallback to top-level vars

    match vars {
        Some(serde_json::Value::Object(map)) => map
            .iter()
            .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
            .collect(),
        _ => {
            vec![]
        }
    }
}

fn strip_jsonc_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;

    while let Some(c) = chars.next() {
        if in_string {
            if c == '\\' {
                out.push(c);
                if let Some(next) = chars.next() {
                    out.push(next);
                }
            } else {
                if c == '"' {
                    in_string = false;
                }
                out.push(c);
            }
        } else if c == '"' {
            in_string = true;
            out.push(c);
        } else if c == '/' {
            match chars.peek() {
                Some('/') => {
                    chars.by_ref().take_while(|&c| c != '\n').for_each(drop);
                    out.push('\n');
                }
                Some('*') => {
                    chars.next();
                    loop {
                        match chars.next() {
                            Some('*') if chars.peek() == Some(&'/') => {
                                chars.next();
                                break;
                            }
                            None => break,
                            _ => {}
                        }
                    }
                }
                _ => out.push(c),
            }
        } else {
            out.push(c);
        }
    }
    out
}

// Whitelist of keys to bake into the binary
const BAKE_KEYS: &[&str] = &["API_URL", "PUBLISHABLE_KEY"];

fn main() {
    println!("cargo:rerun-if-changed=.env.local");
    println!("cargo:rerun-if-changed=wrangler.jsonc");
    println!("cargo:rerun-if-env-changed=BUILD_ENV");
    println!("cargo:rerun-if-changed=assets/levels");
    println!("cargo:rerun-if-changed=assets/blocks");

    let build_env = std::env::var("BUILD_ENV").unwrap_or_else(|_| "local".to_string());

    let env_file = match build_env.as_str() {
        "preview" => ".env.preview",
        "production" => ".env.production",
        _ => ".env.local",
    };

    let allowed: HashSet<&str> = BAKE_KEYS.iter().copied().collect();

    let mut wrangler_count = 0;
    // Use wrangler.jsonc first
    for (key, value) in load_wrangler_vars(&build_env) {
        if allowed.contains(key.as_str()) {
            println!("cargo:rustc-env={}={}", key, value);
            wrangler_count += 1;
        }
    }
    println!(
        "cargo:warning=Baking {} keys from wrangler.jsonc",
        wrangler_count
    );

    let mut env_count = 0;
    match std::fs::read_to_string(env_file) {
        Ok(c) => {
            for line in c.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') {
                    continue;
                }
                if let Some((key, value)) = line.split_once('=') {
                    let key = key.trim();
                    let value = value.trim();
                    if allowed.contains(key) {
                        println!("cargo:rustc-env={}={}", key, value);
                        env_count += 1;
                    }
                }
            }
        }
        Err(_) => {
            println!("cargo:warning={} not found", env_file);
        }
    };
    println!("cargo:warning=Baking {} keys from {}", env_count, env_file);

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

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    if target_os == "windows" && target_arch != "wasm32" {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/favicon.ico");
        res.compile().expect("Failed to compile Windows resource");
    }
}
