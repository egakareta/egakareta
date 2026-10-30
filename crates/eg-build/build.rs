/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::{
    collections::BTreeMap,
    env, fs,
    path::{Path, PathBuf},
};

// This is the only schema for generated environment fields and their tracking.
// Never export arbitrary Wrangler, dotenv, or process keys into the client.
const PUBLIC_ENV_KEYS: &[&str] = &["API_URL", "PUBLISHABLE_KEY", "AUTH_BASE_URL"];
// Each entry generates an embedded directory and its Cargo file-change tracking.
const ASSET_DIRECTORIES: &[(&str, &str)] =
    &[("blocks", "assets/blocks"), ("levels", "assets/levels")];

fn workspace_root() -> PathBuf {
    let root = PathBuf::from(
        env::var("EGAKARETA_WORKSPACE_ROOT")
            .expect("EGAKARETA_WORKSPACE_ROOT should be set by .cargo/config.toml"),
    );
    root.canonicalize().unwrap_or(root)
}

fn env_file(workspace_root: &Path, build_env: &str) -> PathBuf {
    workspace_root.join(match build_env {
        "preview" => ".env.preview",
        "production" => ".env.production",
        _ => ".env.local",
    })
}

fn input_paths(
    workspace_root: &Path,
    build_env: &str,
    feature_enabled: impl Fn(&str) -> bool,
) -> Vec<PathBuf> {
    let mut paths = vec![
        workspace_root.join("wrangler.jsonc"),
        env_file(workspace_root, build_env),
    ];
    for &(feature, directory) in ASSET_DIRECTORIES {
        if feature_enabled(feature) {
            paths.push(workspace_root.join(directory));
        }
    }
    paths
}

fn load_wrangler_vars(workspace_root: &Path, build_env: &str) -> Vec<(String, String)> {
    let wrangler_path = workspace_root.join("wrangler.jsonc");
    let content = match fs::read_to_string(&wrangler_path) {
        Ok(content) => content,
        Err(error) => {
            println!("cargo:warning=Failed to read wrangler.jsonc: {error}");
            return vec![];
        }
    };
    let json: serde_json::Value = match serde_json::from_str(&strip_jsonc_comments(&content)) {
        Ok(json) => json,
        Err(error) => {
            println!("cargo:warning=Failed to parse wrangler.jsonc: {error}");
            return vec![];
        }
    };
    let vars = json
        .pointer(&format!("/env/{build_env}/vars"))
        .or_else(|| json.pointer("/vars"));
    match vars {
        Some(serde_json::Value::Object(map)) => map
            .iter()
            .filter_map(|(key, value)| value.as_str().map(|value| (key.clone(), value.to_string())))
            .collect(),
        _ => vec![],
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

fn parse_env_vars(content: &str) -> impl Iterator<Item = (String, String)> + '_ {
    content.lines().filter_map(|line| {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return None;
        }
        let (key, value) = line
            .strip_prefix("export ")
            .unwrap_or(line)
            .split_once('=')?;
        let value = value.trim();
        let value = value
            .strip_prefix('"')
            .and_then(|value| value.strip_suffix('"'))
            .or_else(|| {
                value
                    .strip_prefix('\'')
                    .and_then(|value| value.strip_suffix('\''))
            })
            .unwrap_or(value);
        Some((key.trim().to_string(), value.to_string()))
    })
}

fn baked_vars(
    workspace_root: &Path,
    build_env: &str,
    process_vars: impl IntoIterator<Item = (String, String)>,
) -> BTreeMap<String, String> {
    let env_file = env_file(workspace_root, build_env);
    let content = match fs::read_to_string(&env_file) {
        Ok(content) => content,
        Err(error) => {
            if error.kind() != std::io::ErrorKind::NotFound {
                println!(
                    "cargo:warning=Failed to read {}: {error}",
                    env_file.display()
                );
            }
            String::new()
        }
    };
    // Explicit process environment > selected .env file > Wrangler variables.
    load_wrangler_vars(workspace_root, build_env)
        .into_iter()
        .chain(parse_env_vars(&content))
        .chain(process_vars)
        .filter(|(key, _)| PUBLIC_ENV_KEYS.contains(&key.as_str()))
        .collect()
}

fn generate_metadata(
    workspace_root: &Path,
    build_env: &str,
    vars: &BTreeMap<String, String>,
) -> String {
    let mut source = String::from(
        "// @generated by eg-build/build.rs. Do not edit.\n\
         /// Public client metadata resolved at compile time.\n\
         #[derive(Clone, Copy, Debug, PartialEq, Eq)]\n\
         pub struct BuildMetadata {\n\
         pub build_env: &'static str,\n",
    );
    for key in PUBLIC_ENV_KEYS {
        source.push_str(&format!(
            "pub {}: Option<&'static str>,\n",
            key.to_ascii_lowercase()
        ));
    }
    source.push_str("}\n/// Build metadata shared by every consumer.\npub const BUILD: BuildMetadata = BuildMetadata {\n");
    // Rust string literals, not raw interpolation, keep quotes/newlines and paths safe.
    source.push_str(&format!("build_env: {build_env:?},\n"));
    for &key in PUBLIC_ENV_KEYS {
        source.push_str(&format!(
            "{}: {:?},\n",
            key.to_ascii_lowercase(),
            vars.get(key).map(String::as_str)
        ));
    }
    source.push_str("};\n");
    for &(feature, directory) in ASSET_DIRECTORIES {
        let path = workspace_root.join(directory);
        let path = path
            .to_str()
            .expect("workspace asset paths should be valid UTF-8");
        source.push_str(&format!(
            "/// Embedded {feature} assets.\n\
             #[cfg(feature = {feature:?})]\n\
             pub static {}_DIR: include_dir::Dir<'static> = include_dir::include_dir!({path:?});\n",
            feature.to_ascii_uppercase(),
        ));
    }
    source
}

fn main() {
    let workspace_root = workspace_root();
    let build_env = env::var("BUILD_ENV").unwrap_or_else(|_| "local".to_string());
    println!("cargo:rerun-if-env-changed=EGAKARETA_WORKSPACE_ROOT");
    println!("cargo:rerun-if-env-changed=BUILD_ENV");
    for key in PUBLIC_ENV_KEYS {
        println!("cargo:rerun-if-env-changed={key}");
    }
    for path in input_paths(&workspace_root, &build_env, |feature| {
        env::var_os(format!("CARGO_FEATURE_{}", feature.to_ascii_uppercase())).is_some()
    }) {
        // Cargo recursively tracks watched directories, including additions/removals.
        println!("cargo:rerun-if-changed={}", path.display());
    }
    let process_vars = PUBLIC_ENV_KEYS
        .iter()
        .filter_map(|&key| env::var(key).ok().map(|value| (key.to_string(), value)));
    let vars = baked_vars(&workspace_root, &build_env, process_vars);
    let source = generate_metadata(&workspace_root, &build_env, &vars);
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo should set OUT_DIR"));
    fs::write(out_dir.join("metadata.rs"), source).expect("build metadata should be generated");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> tempfile::TempDir {
        let root = tempfile::tempdir().expect("temporary workspace should be created");
        fs::write(
            root.path().join("wrangler.jsonc"),
            r#"{
                // URL slashes must survive comment stripping.
                "vars": {"API_URL": "https://local.test", "SECRET_KEY": "private"},
                "env": {
                    "preview": {"vars": {"API_URL": "https://preview.test"}},
                    "production": {"vars": {
                        "API_URL": "https://production.test",
                        "AUTH_BASE_URL": "https://auth.test",
                        "PUBLISHABLE_KEY": "public",
                        "SECRET_KEY": "private"
                    }}
                }
            }"#,
        )
        .unwrap();
        root
    }

    #[test]
    fn selects_and_tracks_each_environment_file() {
        let root = Path::new("workspace");
        for (build_env, file) in [
            ("local", ".env.local"),
            ("preview", ".env.preview"),
            ("production", ".env.production"),
            ("other", ".env.local"),
        ] {
            assert_eq!(env_file(root, build_env), root.join(file));
            let paths = input_paths(root, build_env, |_| false);
            assert!(paths.contains(&root.join(file)));
            assert!(paths.contains(&root.join("wrangler.jsonc")));
            assert_eq!(paths.len(), 2);
        }
    }

    #[test]
    fn tracks_only_enabled_asset_directories() {
        let root = Path::new("workspace");
        for &(feature, directory) in ASSET_DIRECTORIES {
            let paths = input_paths(root, "local", |enabled| enabled == feature);
            assert!(paths.contains(&root.join(directory)));
            assert_eq!(paths.len(), 3);
        }
        assert_eq!(input_paths(root, "local", |_| true).len(), 4);
    }

    #[test]
    fn selects_wrangler_environment_with_top_level_fallback() {
        let root = fixture();
        for (build_env, url) in [
            ("local", "https://local.test"),
            ("preview", "https://preview.test"),
            ("production", "https://production.test"),
        ] {
            let vars = baked_vars(root.path(), build_env, []);
            assert_eq!(vars["API_URL"], url);
            assert!(!vars.contains_key("SECRET_KEY"));
        }
    }

    #[test]
    fn environment_precedence_and_allowlist_apply_to_every_source() {
        let root = fixture();
        fs::write(
            root.path().join(".env.production"),
            "API_URL=https://file.test\nPUBLISHABLE_KEY=file-key\nSECRET_KEY=file-secret\n",
        )
        .unwrap();
        let vars = baked_vars(
            root.path(),
            "production",
            [
                ("API_URL".to_string(), "https://process.test".to_string()),
                ("SECRET_KEY".to_string(), "process-secret".to_string()),
            ],
        );
        assert_eq!(vars["API_URL"], "https://process.test");
        assert_eq!(vars["PUBLISHABLE_KEY"], "file-key");
        assert_eq!(vars["AUTH_BASE_URL"], "https://auth.test");
        assert_eq!(vars.len(), 3);
    }

    #[test]
    fn ignores_unselected_environment_files() {
        let root = fixture();
        fs::write(root.path().join(".env.local"), "API_URL=https://wrong.test").unwrap();
        let vars = baked_vars(root.path(), "production", []);
        assert_eq!(vars["API_URL"], "https://production.test");
    }

    #[test]
    fn parses_quoted_exported_and_plain_env_values() {
        let vars: BTreeMap<_, _> = parse_env_vars(
            "# comment\n\ninvalid\n API_URL = https://test/?a=b#fragment \n\
             export PUBLISHABLE_KEY=\"public=key\"\nAUTH_BASE_URL='https://auth.test'\n",
        )
        .collect();
        assert_eq!(vars["API_URL"], "https://test/?a=b#fragment");
        assert_eq!(vars["PUBLISHABLE_KEY"], "public=key");
        assert_eq!(vars["AUTH_BASE_URL"], "https://auth.test");
        assert_eq!(vars.len(), 3);
    }

    #[test]
    fn strips_jsonc_comments_without_changing_strings() {
        let input = r#"{"url":"https://test/a//b",/* comment */"value":"a\"/*b*/"}// end"#;
        let json: serde_json::Value = serde_json::from_str(&strip_jsonc_comments(input)).unwrap();
        assert_eq!(json["url"], "https://test/a//b");
        assert_eq!(json["value"], "a\"/*b*/");
    }

    #[test]
    fn generates_every_allowlisted_field_even_when_values_are_missing() {
        let source = generate_metadata(Path::new("workspace"), "local", &BTreeMap::new());
        for key in PUBLIC_ENV_KEYS {
            let field = key.to_ascii_lowercase();
            assert!(source.contains(&format!("pub {field}: Option<&'static str>")));
            assert!(source.contains(&format!("{field}: None,")));
        }
        assert!(source.contains("build_env: \"local\""));
        assert!(!source.contains("secret_key"));
    }

    #[test]
    fn escapes_generated_values_and_never_generates_unknown_fields() {
        let value = "https://test/\"quoted\"\\path\nnext\r\0🎵";
        let vars = BTreeMap::from([
            ("API_URL".to_string(), value.to_string()),
            ("SECRET_KEY".to_string(), "never-embed-this".to_string()),
        ]);
        let source = generate_metadata(Path::new("workspace"), "preview", &vars);
        assert!(source.contains(&format!("api_url: Some({value:?})")));
        assert!(!source.contains("never-embed-this"));
        assert!(!source.contains("secret_key"));
    }

    #[test]
    fn generates_feature_gated_asset_fields_from_the_same_tracking_schema() {
        let root = Path::new("workspace");
        let source = generate_metadata(root, "local", &BTreeMap::new());
        for &(feature, directory) in ASSET_DIRECTORIES {
            assert!(source.contains(&format!("#[cfg(feature = {feature:?})]")));
            assert!(source.contains(&format!("pub static {}_DIR:", feature.to_ascii_uppercase())));
            let path = root.join(directory);
            assert!(source.contains(&format!("include_dir!({:?})", path.to_str().unwrap())));
        }
    }
}
