use std::collections::HashSet;

// Whitelist of keys to bake into the binary
const BAKE_KEYS: &[&str] = &["API_URL", "PUBLISHABLE_KEY"];

fn main() {
    println!("cargo:rerun-if-changed=.env.local");
    println!("cargo:rerun-if-changed=.env.preview");
    println!("cargo:rerun-if-changed=.env.production");
    println!("cargo:rerun-if-env-changed=BUILD_ENV");

    let build_env = std::env::var("BUILD_ENV").unwrap_or_else(|_| "local".to_string());

    let env_file = match build_env.as_str() {
        "preview" => ".env.preview",
        "production" => ".env.production",
        _ => ".env.local",
    };

    println!("cargo:warning=Loading env from: {}", env_file);

    let allowed: HashSet<&str> = BAKE_KEYS.iter().copied().collect();

    let contents =
        std::fs::read_to_string(env_file).unwrap_or_else(|_| panic!("Could not read {}", env_file));

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
}
