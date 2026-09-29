/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::path::PathBuf;

use eg_types::{AppSettings, AuthSession};
use renfs::Directory;

const APP_DIR_NAME: &str = "egakareta";
const AUDIO_DIR_NAME: &str = "audio";
const SETTINGS_FILE_NAME: &str = "settings.json";
const AUTH_SESSION_FILE_NAME: &str = "auth_session.json";

/// Save the given audio data to persistent storage with the specified filename.
pub fn save_audio(filename: &str, data: &[u8]) -> Result<(), String> {
    let audio_dir = app_subdir(AUDIO_DIR_NAME)?;
    write_file(&audio_dir, filename, data)
}

/// Read the specified music file from persistent storage, returning None if it doesn't exist.
pub fn read_editor_music_bytes(music_source: &str) -> Result<Option<Vec<u8>>, String> {
    let audio_dir = app_subdir(AUDIO_DIR_NAME)?;
    read_file(&audio_dir, music_source)
}

/// Load the app settings from persistent storage, returning default settings if not found.
pub fn load_app_settings() -> Result<AppSettings, String> {
    let root = app_specific_dir()?;
    let Some(settings_bytes) = read_file(&root, SETTINGS_FILE_NAME)? else {
        return Ok(AppSettings::default());
    };
    let settings_json = String::from_utf8(settings_bytes)
        .map_err(|err| format!("Settings JSON decode failed: {err}"))?;

    serde_json::from_str(&settings_json).map_err(|err| format!("Settings JSON parse failed: {err}"))
}

/// Save the app settings to persistent storage.
pub fn save_app_settings(settings: &AppSettings) -> Result<(), String> {
    let settings_json = serde_json::to_string_pretty(settings)
        .map_err(|err| format!("Settings JSON encode failed: {err}"))?;
    let root = app_specific_dir()?;
    write_file(&root, SETTINGS_FILE_NAME, settings_json.as_bytes())
}

/// Load the auth session from persistent storage, returning None if not found.
pub fn load_auth_session() -> Result<Option<AuthSession>, String> {
    let root = app_specific_dir()?;
    let Some(session_bytes) = read_file(&root, AUTH_SESSION_FILE_NAME)? else {
        return Ok(None);
    };
    let session_json = String::from_utf8(session_bytes)
        .map_err(|err| format!("Auth session JSON decode failed: {err}"))?;

    serde_json::from_str(&session_json)
        .map(Some)
        .map_err(|err| format!("Auth session JSON parse failed: {err}"))
}

/// Save the auth session to persistent storage.
pub fn save_auth_session(session: &AuthSession) -> Result<(), String> {
    let session_json = serde_json::to_string_pretty(session)
        .map_err(|err| format!("Auth session JSON encode failed: {err}"))?;
    write_file(
        &app_specific_dir()?,
        AUTH_SESSION_FILE_NAME,
        session_json.as_bytes(),
    )
}

/// Clear the auth session from persistent storage.
pub fn clear_auth_session() -> Result<(), String> {
    let _ = app_specific_dir()?
        .remove_file(AUTH_SESSION_FILE_NAME)
        .map_err(|err| format!("Failed to clear auth session: {err}"));
    Ok(())
}

/// The directory for the app-specific data storage.
/// Uses the `EGAKARETA_STORAGE_ROOT` environment variable in tests.
fn app_specific_dir() -> Result<Directory, String> {
    #[cfg(test)]
    if let Ok(root) = std::env::var("EGAKARETA_STORAGE_ROOT") {
        let path = std::path::PathBuf::from(root);
        std::fs::create_dir_all(&path)
            .map_err(|err| format!("Failed to create storage root: {err}"))?;
        return Directory::new(path).map_err(|err| format!("Failed to open storage root: {err}"));
    }

    renfs::app_dir(APP_DIR_NAME).map_err(|err| format!("Failed to get app-specific dir: {err}"))
}

/// Returns the specified subdirectory of the app-specific directory, creating it if needed.
fn app_subdir(name: &str) -> Result<Directory, String> {
    let root = app_specific_dir()?;
    root.create_dir_all(name)
        .map_err(|err| format!("Failed to create subdirectory '{name}': {err}"))?;

    Directory::new(PathBuf::from(root.as_path()).join(name))
        .map_err(|err| format!("Failed to open subdirectory '{name}': {err}"))
}

/// Read the specified file from the given directory, returning None if it doesn't exist.
fn read_file(dir: &Directory, filename: &str) -> Result<Option<Vec<u8>>, String> {
    let exists = dir
        .exists_sync(filename)
        .map_err(|err| format!("Failed to check file '{filename}': {err}"))?;
    if !exists {
        return Ok(None);
    }

    dir.read_file_sync(filename)
        .map(Some)
        .map_err(|err| format!("File read failed for '{filename}': {err}"))
}

/// Write the specified data to the given file in the directory, creating it if it doesn't exist.
fn write_file(dir: &Directory, filename: &str, data: &[u8]) -> Result<(), String> {
    dir.write_file_sync(filename, data)
        .map_err(|err| format!("File write failed for '{filename}': {err}"))
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

    #[test]
    fn storage_root_uses_storage_override() {
        let _env = TestEnv::new();
        let root = app_specific_dir().expect("failed to get app-specific dir");

        write_file(&root, "probe.txt", b"probe").expect("failed to write probe");
        let loaded = read_file(&root, "probe.txt")
            .expect("failed to read probe")
            .expect("probe should exist");

        assert_eq!(loaded, b"probe");
    }

    #[test]
    fn app_subdirectories_are_created() {
        let _env = TestEnv::new();
        let audio_dir = app_subdir(AUDIO_DIR_NAME).expect("audio directory should resolve");
        assert!(std::path::Path::new(audio_dir.as_path()).is_dir());
    }

    #[test]
    fn test_settings_roundtrip() {
        let _env = TestEnv::new();

        let mut settings = AppSettings::default();
        settings.editor_snap_to_grid = !settings.editor_snap_to_grid;
        settings.editor_snap_step = 1.23;

        save_app_settings(&settings).expect("failed to save settings");
        let loaded = load_app_settings().expect("failed to load settings");

        assert_eq!(loaded.editor_snap_to_grid, settings.editor_snap_to_grid);
        assert_eq!(loaded.editor_snap_step, settings.editor_snap_step);
    }

    #[test]
    fn test_load_settings_default_when_missing() {
        let _env = TestEnv::new();
        let loaded = load_app_settings().expect("failed to load settings");
        assert_eq!(loaded, AppSettings::default());
    }

    #[test]
    fn test_auth_session_roundtrip() {
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

        save_auth_session(&session).expect("failed to save auth session");
        let loaded = load_auth_session().expect("failed to load auth session");
        assert_eq!(loaded, Some(session));

        clear_auth_session().expect("failed to clear auth session");
        let cleared = load_auth_session().expect("failed to load cleared auth session");
        assert_eq!(cleared, None);
    }

    #[test]
    fn test_audio_roundtrip() {
        let _env = TestEnv::new();

        let data1 = b"audio1-content";

        save_audio("test1.ogg", data1).expect("failed to save audio 1");

        let read = read_editor_music_bytes("test1.ogg").expect("failed to read audio");

        assert_eq!(read, Some(data1.to_vec()));
    }

    #[test]
    fn test_read_music_bytes_builtin_fallback() {
        let _env = TestEnv::new();

        // Should return None for unknown builtin and unknown user audio
        let read = read_editor_music_bytes("missing.ogg").expect("unexpected error");

        assert!(read.is_none());

        // Save to user audio and check fallback
        let data = b"user-audio";
        save_audio("user.ogg", data).unwrap();
        let read = read_editor_music_bytes("user.ogg").unwrap();
        assert_eq!(read, Some(data.to_vec()));
    }
}
