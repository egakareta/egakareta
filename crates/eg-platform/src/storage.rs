/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use eg_types::{AppSettings, AuthSession};
use opfs::{DirectoryHandle as _, FileHandle as _};

const AUDIO_DIR_NAME: &str = "audio";
const SETTINGS_FILE_NAME: &str = "settings.json";
const AUTH_SESSION_FILE_NAME: &str = "auth_session.json";

/// Save the given audio data to persistent storage with the specified filename.
pub async fn save_audio(filename: &str, data: &[u8]) -> Result<(), String> {
    let audio_dir = opfs_subdir(AUDIO_DIR_NAME).await?;
    opfs_write_file(&audio_dir, filename, data).await
}

/// Read the specified music file from persistent storage, returning None if it doesn't exist.
pub async fn read_editor_music_bytes(music_source: &str) -> Result<Option<Vec<u8>>, String> {
    let audio_dir = opfs_subdir(AUDIO_DIR_NAME).await?;
    opfs_read_file(&audio_dir, music_source).await
}

/// Load the app settings from persistent storage, returning default settings if not found.
pub async fn load_app_settings() -> Result<AppSettings, String> {
    let root = app_specific_dir().await?;
    let Some(settings_bytes) = opfs_read_file(&root, SETTINGS_FILE_NAME).await? else {
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
    opfs_write_file(&root, SETTINGS_FILE_NAME, settings_json.as_bytes()).await
}

/// Load the auth session from persistent storage, returning None if not found.
pub async fn load_auth_session() -> Result<Option<AuthSession>, String> {
    let root = app_specific_dir().await?;
    let Some(session_bytes) = opfs_read_file(&root, AUTH_SESSION_FILE_NAME).await? else {
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
    opfs_write_file(
        &app_specific_dir().await?,
        AUTH_SESSION_FILE_NAME,
        session_json.as_bytes(),
    )
    .await
}

/// Clear the auth session from persistent storage.
pub async fn clear_auth_session() -> Result<(), String> {
    let _ = app_specific_dir()
        .await?
        .remove_entry(AUTH_SESSION_FILE_NAME)
        .await
        .map_err(|err| format!("Failed to clear auth session: {:?}", err));
    Ok(())
}

/// The DirectoryHandle for the app-specific data storage.
/// Uses the `EGAKARETA_STORAGE_ROOT` environment variable in tests.
async fn app_specific_dir() -> Result<opfs::persistent::DirectoryHandle, String> {
    #[cfg(test)]
    if let Ok(root) = std::env::var("EGAKARETA_STORAGE_ROOT") {
        let path = std::path::PathBuf::from(root);
        std::fs::create_dir_all(&path)
            .map_err(|err| format!("Failed to create storage root: {err}"))?;
        return Ok(opfs::persistent::DirectoryHandle::from(path));
    }

    opfs::persistent::app_specific_dir()
        .await
        .map_err(|err| format!("Failed to get app specific dir: {:?}", err))
}

/// The DirectoryHandle for the specified subdirectory of the `app_specific_dir`, creating it if it doesn't exist.
async fn opfs_subdir(name: &str) -> Result<opfs::persistent::DirectoryHandle, String> {
    app_specific_dir()
        .await?
        .get_directory_handle_with_options(name, &opfs::GetDirectoryHandleOptions { create: true })
        .await
        .map_err(|err| format!("Failed to get subdirectory '{name}': {:?}", err))
}

/// Read the specified file from the given directory, returning None if it doesn't exist.
async fn opfs_read_file(
    dir: &opfs::persistent::DirectoryHandle,
    filename: &str,
) -> Result<Option<Vec<u8>>, String> {
    let file = dir
        .get_file_handle_with_options(filename, &opfs::GetFileHandleOptions { create: false })
        .await;
    let file = match file {
        Ok(file) => file,
        Err(_) => return Ok(None),
    };

    file.read()
        .await
        .map(Some)
        .map_err(|err| format!("File read failed: {:?}", err))
}

/// Write the specified data to the given file in the directory, creating it if it doesn't exist.
async fn opfs_write_file(
    dir: &opfs::persistent::DirectoryHandle,
    filename: &str,
    data: &[u8],
) -> Result<(), String> {
    use opfs::{DirectoryHandle as _, FileHandle as _, WritableFileStream as _};

    let mut file = dir
        .get_file_handle_with_options(filename, &opfs::GetFileHandleOptions { create: true })
        .await
        .map_err(|err| format!("Failed to get file handle '{filename}': {:?}", err))?;
    let mut writer = file
        .create_writable_with_options(&opfs::CreateWritableOptions {
            keep_existing_data: false,
        })
        .await
        .map_err(|err| format!("Failed to create writable for file '{filename}': {:?}", err))?;

    let _ = writer.write_at_cursor_pos(data).await;
    writer
        .close()
        .await
        .map_err(|err| format!("Failed to close writable for file '{filename}': {:?}", err))
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
    async fn opfs_root_uses_storage_override() {
        let _env = TestEnv::new();
        let root = app_specific_dir()
            .await
            .expect("failed to get app specific dir");

        opfs_write_file(&root, "probe.txt", b"probe")
            .await
            .expect("failed to write probe");
        let loaded = opfs_read_file(&root, "probe.txt")
            .await
            .expect("failed to read probe")
            .expect("probe should exist");

        assert_eq!(loaded, b"probe");
    }

    #[tokio::test]
    async fn opfs_subdirectories_are_created() {
        let _env = TestEnv::new();
        opfs_subdir(AUDIO_DIR_NAME)
            .await
            .expect("audio directory should resolve");
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

        save_audio("test1.ogg", data1)
            .await
            .expect("failed to save audio 1");

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
}
