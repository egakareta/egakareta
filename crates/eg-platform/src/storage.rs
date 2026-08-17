/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

#[cfg(not(target_arch = "wasm32"))]
use async_fs as fs;
use eg_types::{AppSettings, AuthSession, PersistentWaveformCacheEntry};
use futures::StreamExt;
#[cfg(target_arch = "wasm32")]
use web_fs as fs;

const AUDIO_DIR_NAME: &str = "audio";
const WAVEFORM_DIR_NAME: &str = "waveforms";
const SETTINGS_FILE_NAME: &str = "settings.json";
const AUTH_SESSION_FILE_NAME: &str = "auth_session.json";
const APP_STORAGE_DIR_NAME: &str = "egakareta";

/// Save the given audio data to persistent storage with the specified filename.
pub async fn save_audio(filename: &str, data: &[u8]) -> Result<(), String> {
    let audio_dir = storage_subdir(AUDIO_DIR_NAME).await?;
    storage_write_file(&audio_dir, filename, data).await
}

/// Load all audio files from persistent storage, returning a map of filenames to their byte contents.
pub async fn load_all_audio() -> HashMap<String, Vec<u8>> {
    let mut audio_map = HashMap::new();
    let Ok(audio_dir) = storage_subdir(AUDIO_DIR_NAME).await else {
        return audio_map;
    };
    let Ok(entries) = fs::read_dir(&audio_dir).await else {
        return audio_map;
    };

    futures::pin_mut!(entries);
    while let Some(entry) = entries.next().await {
        let Ok(entry) = entry else {
            continue;
        };
        let Ok(file_type) = entry.file_type().await else {
            continue;
        };
        if !file_type.is_file() {
            continue;
        }
        if let Ok(bytes) = fs::read(entry.path()).await {
            audio_map.insert(entry.file_name().to_string_lossy().into_owned(), bytes);
        }
    }

    audio_map
}

/// Read the specified music file from persistent storage, returning None if it doesn't exist.
pub async fn read_editor_music_bytes(music_source: &str) -> Result<Option<Vec<u8>>, String> {
    let audio_dir = storage_subdir(AUDIO_DIR_NAME).await?;
    storage_read_file(&audio_dir, music_source).await
}

/// Save the given waveform cache entry to persistent storage with the specified cache key.
pub async fn save_waveform(
    cache_key: &str,
    entry: &PersistentWaveformCacheEntry,
) -> Result<(), String> {
    let encoded = encode_waveform_entry(entry)?;
    let waveform_dir = storage_subdir(WAVEFORM_DIR_NAME).await?;
    storage_write_file(&waveform_dir, &format!("{cache_key}.cbor"), &encoded).await
}

/// Load the waveform cache entry from persistent storage with the specified cache key.
pub async fn load_waveform(cache_key: &str) -> Option<PersistentWaveformCacheEntry> {
    let waveform_dir = storage_subdir(WAVEFORM_DIR_NAME).await.ok()?;
    let bytes = storage_read_file(&waveform_dir, &format!("{cache_key}.cbor"))
        .await
        .ok()??;
    decode_waveform_entry(&bytes).ok()
}

fn encode_waveform_entry(entry: &PersistentWaveformCacheEntry) -> Result<Vec<u8>, String> {
    serde_cbor::to_vec(entry).map_err(|error| format!("Waveform cache encode failed: {error}"))
}

fn decode_waveform_entry(bytes: &[u8]) -> Result<PersistentWaveformCacheEntry, String> {
    serde_cbor::from_slice(bytes).map_err(|error| format!("Waveform cache decode failed: {error}"))
}

/// Load the app settings from persistent storage, returning default settings if not found.
pub async fn load_app_settings() -> Result<AppSettings, String> {
    let root = app_specific_dir().await?;
    let Some(settings_bytes) = storage_read_file(&root, SETTINGS_FILE_NAME).await? else {
        return Ok(AppSettings::default());
    };
    let settings_json = String::from_utf8(settings_bytes)
        .map_err(|err| format!("Settings JSON decode failed: {err}"))?;

    serde_json::from_str(&settings_json).map_err(|err| format!("Settings JSON parse failed: {err}"))
}

/// Save the app settings to persistent storage.
pub async fn save_app_settings(settings: &AppSettings) -> Result<(), String> {
    let settings_json = serde_json::to_string_pretty(settings)
        .map_err(|err| format!("Settings JSON encode failed: {err}"))?;
    let root = app_specific_dir().await?;
    storage_write_file(&root, SETTINGS_FILE_NAME, settings_json.as_bytes()).await
}

/// Load the auth session from persistent storage, returning None if not found.
pub async fn load_auth_session() -> Result<Option<AuthSession>, String> {
    let root = app_specific_dir().await?;
    let Some(session_bytes) = storage_read_file(&root, AUTH_SESSION_FILE_NAME).await? else {
        return Ok(None);
    };
    let session_json = String::from_utf8(session_bytes)
        .map_err(|err| format!("Auth session JSON decode failed: {err}"))?;

    serde_json::from_str(&session_json)
        .map(Some)
        .map_err(|err| format!("Auth session JSON parse failed: {err}"))
}

/// Save the auth session to persistent storage.
pub async fn save_auth_session(session: &AuthSession) -> Result<(), String> {
    let session_json = serde_json::to_string_pretty(session)
        .map_err(|err| format!("Auth session JSON encode failed: {err}"))?;
    storage_write_file(
        &app_specific_dir().await?,
        AUTH_SESSION_FILE_NAME,
        session_json.as_bytes(),
    )
    .await
}

/// Clear the auth session from persistent storage.
pub async fn clear_auth_session() -> Result<(), String> {
    let path = app_specific_dir().await?.join(AUTH_SESSION_FILE_NAME);
    match fs::remove_file(path).await {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("Failed to clear auth session: {error}")),
    }
    Ok(())
}

/// The path for app-specific data storage.
/// Uses the `EGAKARETA_STORAGE_ROOT` environment variable in tests.
async fn app_specific_dir() -> Result<PathBuf, String> {
    #[cfg(test)]
    if let Ok(root) = std::env::var("EGAKARETA_STORAGE_ROOT") {
        let path = std::path::PathBuf::from(root);
        fs::create_dir_all(&path)
            .await
            .map_err(|err| format!("Failed to create storage root: {err}"))?;
        return Ok(path);
    }

    #[cfg(not(target_arch = "wasm32"))]
    let path = dirs::data_local_dir()
        .ok_or_else(|| "Failed to resolve the native data directory".to_string())?
        .join(APP_STORAGE_DIR_NAME);
    #[cfg(target_arch = "wasm32")]
    let path = PathBuf::from(APP_STORAGE_DIR_NAME);

    fs::create_dir_all(&path)
        .await
        .map_err(|err| format!("Failed to create app storage directory: {err}"))?;
    Ok(path)
}

/// Resolve a named subdirectory of the app storage, creating it if needed.
async fn storage_subdir(name: &str) -> Result<PathBuf, String> {
    let path = app_specific_dir().await?.join(name);
    fs::create_dir_all(&path)
        .await
        .map_err(|err| format!("Failed to create subdirectory '{name}': {err}"))?;
    Ok(path)
}

/// Read the specified file from the given directory, returning None if it doesn't exist.
async fn storage_read_file(dir: &Path, filename: &str) -> Result<Option<Vec<u8>>, String> {
    match fs::read(dir.join(filename)).await {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("File read failed: {error}")),
    }
}

/// Write the specified data to the given file in the directory, creating it if it doesn't exist.
async fn storage_write_file(dir: &Path, filename: &str, data: &[u8]) -> Result<(), String> {
    let path = dir.join(filename);
    // `web-fs::write` has create-new semantics, while `async-fs::write`
    // replaces existing files. Removing the old file first gives this shared
    // adapter a consistent overwrite contract on both platforms.
    let _ = fs::remove_file(&path).await;
    fs::write(&path, data)
        .await
        .map_err(|err| format!("Failed to write file '{filename}': {err}"))
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use std::sync::{Mutex, OnceLock};

    use super::*;

    fn test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct TestEnv {
        _temp_dir: tempfile::TempDir,
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    impl TestEnv {
        fn new() -> Self {
            let lock = test_lock().lock().unwrap();
            let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
            unsafe { std::env::set_var("EGAKARETA_STORAGE_ROOT", temp_dir.path()) };
            Self {
                _temp_dir: temp_dir,
                _lock: lock,
            }
        }
    }

    impl Drop for TestEnv {
        fn drop(&mut self) {
            unsafe { std::env::remove_var("EGAKARETA_STORAGE_ROOT") };
        }
    }

    #[tokio::test]
    async fn storage_root_uses_storage_override() {
        let _env = TestEnv::new();
        let root = app_specific_dir()
            .await
            .expect("failed to get app specific dir");

        storage_write_file(&root, "probe.txt", b"probe")
            .await
            .expect("failed to write probe");
        let loaded = storage_read_file(&root, "probe.txt")
            .await
            .expect("failed to read probe")
            .expect("probe should exist");

        assert_eq!(loaded, b"probe");
    }

    #[tokio::test]
    async fn storage_subdirectories_are_created() {
        let _env = TestEnv::new();
        storage_subdir(AUDIO_DIR_NAME)
            .await
            .expect("audio directory should resolve");
        storage_subdir(WAVEFORM_DIR_NAME)
            .await
            .expect("waveform directory should resolve");
    }

    #[tokio::test]
    async fn test_settings_roundtrip() {
        let _env = TestEnv::new();

        let mut settings = AppSettings::default();
        settings.editor_snap_to_grid = !settings.editor_snap_to_grid;
        settings.editor_snap_step = 1.23;

        save_app_settings(&settings)
            .await
            .expect("failed to save settings");
        let loaded = load_app_settings().await.expect("failed to load settings");

        assert_eq!(loaded.editor_snap_to_grid, settings.editor_snap_to_grid);
        assert_eq!(loaded.editor_snap_step, settings.editor_snap_step);
    }

    #[tokio::test]
    async fn test_load_settings_default_when_missing() {
        let _env = TestEnv::new();
        let loaded = load_app_settings().await.expect("failed to load settings");
        assert_eq!(loaded, AppSettings::default());
    }

    #[tokio::test]
    async fn test_auth_session_roundtrip() {
        let _env = TestEnv::new();
        let session = eg_types::AuthSession {
            session: eg_types::AuthSessionTokens {
                access_token: "access".to_string(),
                refresh_token: "refresh".to_string(),
                expires_at: Some(123),
                token_type: "bearer".to_string(),
            },
            user: eg_types::AuthUser {
                id: "user-id".to_string(),
                email: Some("player@example.com".to_string()),
            },
            profile: Some(eg_types::AuthProfile {
                id: "user-id".to_string(),
                username: Some("player".to_string()),
                avatar_url: None,
                country: "UN".to_string(),
            }),
        };

        save_auth_session(&session)
            .await
            .expect("failed to save auth session");
        let loaded = load_auth_session()
            .await
            .expect("failed to load auth session");
        assert_eq!(loaded, Some(session));

        clear_auth_session()
            .await
            .expect("failed to clear auth session");
        let cleared = load_auth_session()
            .await
            .expect("failed to load cleared auth session");
        assert_eq!(cleared, None);
    }

    #[tokio::test]
    async fn test_audio_roundtrip() {
        let _env = TestEnv::new();

        let data1 = b"audio1-content";
        let data2 = b"audio2-content";

        save_audio("test1.ogg", data1)
            .await
            .expect("failed to save audio 1");
        save_audio("test2.ogg", data2)
            .await
            .expect("failed to save audio 2");

        let all = load_all_audio().await;
        assert_eq!(all.len(), 2);
        assert_eq!(all.get("test1.ogg").unwrap(), data1);
        assert_eq!(all.get("test2.ogg").unwrap(), data2);

        let read = read_editor_music_bytes("test1.ogg")
            .await
            .expect("failed to read audio");

        assert_eq!(read, Some(data1.to_vec()));
    }

    #[tokio::test]
    async fn test_read_music_bytes_builtin_fallback() {
        let _env = TestEnv::new();

        // Should return None for unknown builtin and unknown user audio
        let read = read_editor_music_bytes("missing.ogg")
            .await
            .expect("unexpected error");

        assert!(read.is_none());

        // Save to user audio and check fallback
        let data = b"user-audio";
        save_audio("user.ogg", data).await.unwrap();
        let read = read_editor_music_bytes("user.ogg").await.unwrap();
        assert_eq!(read, Some(data.to_vec()));
    }

    #[tokio::test]
    async fn test_waveform_cache_roundtrip() {
        let _env = TestEnv::new();
        let entry = PersistentWaveformCacheEntry::new(vec![0.1, 0.5, 1.0], 44_100, 256);

        save_waveform("waveform-v1-window-256-sha256-test", &entry)
            .await
            .expect("failed to save waveform cache");

        let loaded = load_waveform("waveform-v1-window-256-sha256-test")
            .await
            .expect("failed to load waveform cache");
        assert_eq!(loaded, entry);
        assert!(load_waveform("missing-waveform").await.is_none());
    }
}
