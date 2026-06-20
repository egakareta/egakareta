/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::collections::HashMap;

#[cfg(not(target_arch = "wasm32"))]
use std::sync::OnceLock;

use eg_types::AppSettings;
use eg_types::AuthSession;
use eg_types::PersistentWaveformCacheEntry;

const AUDIO_DIR_NAME: &str = "audio";
const WAVEFORM_DIR_NAME: &str = "waveforms";
const SETTINGS_FILE_NAME: &str = "settings.json";
const AUTH_SESSION_FILE_NAME: &str = "auth_session.json";
#[cfg(not(target_arch = "wasm32"))]
const NATIVE_APP_DIR_NAME: &str = "egakareta";

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait AudioStorage {
    async fn save_audio(&self, filename: &str, data: &[u8]) -> Result<(), String>;
    async fn load_all_audio(&self) -> HashMap<String, Vec<u8>>;
    fn read_editor_music_bytes(
        &self,
        level_name: Option<&str>,
        music_source: &str,
    ) -> Option<Vec<u8>>;
    async fn save_waveform(
        &self,
        cache_key: &str,
        entry: &PersistentWaveformCacheEntry,
    ) -> Result<(), String>;
    async fn load_waveform(&self, cache_key: &str) -> Option<PersistentWaveformCacheEntry>;
}

pub struct PlatformAudioStorage;

pub async fn save_audio(filename: &str, data: &[u8]) -> Result<(), String> {
    PlatformAudioStorage.save_audio(filename, data).await
}

pub async fn load_all_audio() -> HashMap<String, Vec<u8>> {
    PlatformAudioStorage.load_all_audio().await
}

pub fn read_editor_music_bytes(level_name: Option<&str>, music_source: &str) -> Option<Vec<u8>> {
    PlatformAudioStorage.read_editor_music_bytes(level_name, music_source)
}

pub async fn save_waveform(
    cache_key: &str,
    entry: &PersistentWaveformCacheEntry,
) -> Result<(), String> {
    PlatformAudioStorage.save_waveform(cache_key, entry).await
}

pub async fn load_waveform(cache_key: &str) -> Option<PersistentWaveformCacheEntry> {
    PlatformAudioStorage.load_waveform(cache_key).await
}

fn encode_waveform_entry(entry: &PersistentWaveformCacheEntry) -> Result<Vec<u8>, String> {
    serde_cbor::to_vec(entry).map_err(|error| format!("Waveform cache encode failed: {error}"))
}

fn decode_waveform_entry(bytes: &[u8]) -> Result<PersistentWaveformCacheEntry, String> {
    serde_cbor::from_slice(bytes).map_err(|error| format!("Waveform cache decode failed: {error}"))
}

pub async fn load_app_settings() -> Result<AppSettings, String> {
    load_platform_app_settings().await
}

pub async fn save_app_settings(settings: &AppSettings) -> Result<(), String> {
    save_platform_app_settings(settings).await
}

pub async fn load_auth_session() -> Result<Option<AuthSession>, String> {
    load_platform_auth_session().await
}

pub async fn save_auth_session(session: &AuthSession) -> Result<(), String> {
    save_platform_auth_session(session).await
}

pub async fn clear_auth_session() -> Result<(), String> {
    clear_platform_auth_session().await
}

fn opfs_error(context: &str, error: impl std::fmt::Debug) -> String {
    format!("OPFS {context} failed: {error:?}")
}

#[cfg(not(target_arch = "wasm32"))]
fn native_opfs_runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to create native OPFS runtime")
    })
}

#[cfg(target_arch = "wasm32")]
async fn run_opfs<T, Error>(
    future: impl std::future::Future<Output = Result<T, Error>>,
    context: &str,
) -> Result<T, String>
where
    Error: std::fmt::Debug,
{
    future.await.map_err(|error| opfs_error(context, error))
}

#[cfg(not(target_arch = "wasm32"))]
async fn run_opfs<T, Error>(
    future: impl std::future::Future<Output = Result<T, Error>>,
    context: &str,
) -> Result<T, String>
where
    Error: std::fmt::Debug,
{
    native_opfs_runtime()
        .block_on(future)
        .map_err(|error| opfs_error(context, error))
}

async fn opfs_root_dir() -> Result<opfs::persistent::DirectoryHandle, String> {
    #[cfg(target_arch = "wasm32")]
    {
        run_opfs(opfs::persistent::app_specific_dir(), "root open").await
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        use opfs::DirectoryHandle as _;

        if let Ok(root) = std::env::var("EGAKARETA_STORAGE_ROOT") {
            let root = std::path::PathBuf::from(root);
            std::fs::create_dir_all(&root).map_err(|error| opfs_error("root create", error))?;
            return Ok(opfs::persistent::DirectoryHandle::from(root));
        }

        let root = run_opfs(opfs::persistent::app_specific_dir(), "root open").await?;
        run_opfs(
            root.get_directory_handle_with_options(
                NATIVE_APP_DIR_NAME,
                &opfs::GetDirectoryHandleOptions { create: true },
            ),
            "app directory open",
        )
        .await
    }
}

async fn opfs_subdir(name: &str) -> Result<opfs::persistent::DirectoryHandle, String> {
    use opfs::DirectoryHandle as _;

    let root = opfs_root_dir().await?;
    run_opfs(
        root.get_directory_handle_with_options(
            name,
            &opfs::GetDirectoryHandleOptions { create: true },
        ),
        "directory open",
    )
    .await
}

async fn opfs_read_file(
    dir: &opfs::persistent::DirectoryHandle,
    filename: &str,
) -> Result<Option<Vec<u8>>, String> {
    use opfs::DirectoryHandle as _;
    use opfs::FileHandle as _;

    let file =
        dir.get_file_handle_with_options(filename, &opfs::GetFileHandleOptions { create: false });
    let file = match file {
        future => match run_opfs(future, "file open").await {
            Ok(file) => file,
            Err(_) => return Ok(None),
        },
    };

    run_opfs(file.read(), "file read").await.map(Some)
}

async fn opfs_write_file(
    dir: &opfs::persistent::DirectoryHandle,
    filename: &str,
    data: &[u8],
) -> Result<(), String> {
    use opfs::DirectoryHandle as _;
    use opfs::FileHandle as _;
    use opfs::WritableFileStream as _;

    let mut file = run_opfs(
        dir.get_file_handle_with_options(filename, &opfs::GetFileHandleOptions { create: true }),
        "file open",
    )
    .await?;
    let mut writer = run_opfs(
        file.create_writable_with_options(&opfs::CreateWritableOptions {
            keep_existing_data: false,
        }),
        "file writable open",
    )
    .await?;
    run_opfs(writer.write_at_cursor_pos(data), "file write").await?;
    run_opfs(writer.close(), "file close").await
}

async fn load_platform_app_settings() -> Result<AppSettings, String> {
    let root = opfs_root_dir().await?;
    let Some(settings_bytes) = opfs_read_file(&root, SETTINGS_FILE_NAME).await? else {
        return Ok(AppSettings::default());
    };
    let settings_json = String::from_utf8(settings_bytes)
        .map_err(|err| format!("Settings JSON decode failed: {err}"))?;

    serde_json::from_str(&settings_json).map_err(|err| format!("Settings JSON parse failed: {err}"))
}

async fn save_platform_app_settings(settings: &AppSettings) -> Result<(), String> {
    let settings_json = serde_json::to_string_pretty(settings)
        .map_err(|err| format!("Settings JSON encode failed: {err}"))?;
    let root = opfs_root_dir().await?;
    opfs_write_file(&root, SETTINGS_FILE_NAME, settings_json.as_bytes()).await
}

async fn load_platform_auth_session() -> Result<Option<AuthSession>, String> {
    let root = opfs_root_dir().await?;
    let Some(session_bytes) = opfs_read_file(&root, AUTH_SESSION_FILE_NAME).await? else {
        return Ok(None);
    };
    let session_json = String::from_utf8(session_bytes)
        .map_err(|err| format!("Auth session JSON decode failed: {err}"))?;

    serde_json::from_str(&session_json)
        .map(Some)
        .map_err(|err| format!("Auth session JSON parse failed: {err}"))
}

async fn save_platform_auth_session(session: &AuthSession) -> Result<(), String> {
    let session_json = serde_json::to_string_pretty(session)
        .map_err(|err| format!("Auth session JSON encode failed: {err}"))?;
    let root = opfs_root_dir().await?;
    opfs_write_file(&root, AUTH_SESSION_FILE_NAME, session_json.as_bytes()).await
}

async fn clear_platform_auth_session() -> Result<(), String> {
    use opfs::DirectoryHandle as _;

    let mut root = opfs_root_dir().await?;
    let _ = run_opfs(
        root.remove_entry(AUTH_SESSION_FILE_NAME),
        "auth session remove",
    )
    .await;
    Ok(())
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl AudioStorage for PlatformAudioStorage {
    async fn save_audio(&self, filename: &str, data: &[u8]) -> Result<(), String> {
        let audio_dir = opfs_subdir(AUDIO_DIR_NAME).await?;
        opfs_write_file(&audio_dir, filename, data).await
    }

    async fn load_all_audio(&self) -> HashMap<String, Vec<u8>> {
        use futures::StreamExt;
        use opfs::DirectoryEntry;
        use opfs::DirectoryHandle as _;
        use opfs::FileHandle as _;

        let mut audio_map = HashMap::new();
        let Ok(audio_dir) = opfs_subdir(AUDIO_DIR_NAME).await else {
            return audio_map;
        };
        let Ok(entries) = run_opfs(audio_dir.entries(), "directory entries").await else {
            return audio_map;
        };

        futures::pin_mut!(entries);
        while let Some(entry) = entries.next().await {
            let Ok((filename, DirectoryEntry::File(file))) = entry else {
                continue;
            };
            if let Ok(bytes) = run_opfs(file.read(), "file read").await {
                audio_map.insert(filename, bytes);
            }
        }

        audio_map
    }

    fn read_editor_music_bytes(
        &self,
        level_name: Option<&str>,
        music_source: &str,
    ) -> Option<Vec<u8>> {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(name) = level_name {
            if let Some(bytes) = eg_level::get_builtin_audio(name, music_source) {
                return Some(bytes.to_vec());
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            return pollster::block_on(async {
                let audio_dir = opfs_subdir(AUDIO_DIR_NAME).await.ok()?;
                opfs_read_file(&audio_dir, music_source).await.ok()?
            });
        }

        #[cfg(target_arch = "wasm32")]
        {
            let _ = (level_name, music_source);
            None
        }
    }

    async fn save_waveform(
        &self,
        cache_key: &str,
        entry: &PersistentWaveformCacheEntry,
    ) -> Result<(), String> {
        let encoded = encode_waveform_entry(entry)?;
        let waveform_dir = opfs_subdir(WAVEFORM_DIR_NAME).await?;
        opfs_write_file(&waveform_dir, &format!("{cache_key}.cbor"), &encoded).await
    }

    async fn load_waveform(&self, cache_key: &str) -> Option<PersistentWaveformCacheEntry> {
        let waveform_dir = opfs_subdir(WAVEFORM_DIR_NAME).await.ok()?;
        let bytes = opfs_read_file(&waveform_dir, &format!("{cache_key}.cbor"))
            .await
            .ok()??;
        decode_waveform_entry(&bytes).ok()
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

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
            std::env::set_var("EGAKARETA_STORAGE_ROOT", temp_dir.path());
            Self {
                _temp_dir: temp_dir,
                _lock: lock,
            }
        }
    }

    impl Drop for TestEnv {
        fn drop(&mut self) {
            std::env::remove_var("EGAKARETA_STORAGE_ROOT");
        }
    }

    #[test]
    fn opfs_root_uses_storage_override() {
        pollster::block_on(async {
            let _env = TestEnv::new();
            let root = opfs_root_dir().await.expect("opfs root should resolve");

            opfs_write_file(&root, "probe.txt", b"probe")
                .await
                .expect("failed to write probe");
            let loaded = opfs_read_file(&root, "probe.txt")
                .await
                .expect("failed to read probe")
                .expect("probe should exist");

            assert_eq!(loaded, b"probe");
        });
    }

    #[test]
    fn opfs_subdirectories_are_created() {
        pollster::block_on(async {
            let _env = TestEnv::new();
            opfs_subdir(AUDIO_DIR_NAME)
                .await
                .expect("audio directory should resolve");
            opfs_subdir(WAVEFORM_DIR_NAME)
                .await
                .expect("waveform directory should resolve");
        });
    }

    #[test]
    fn test_settings_roundtrip() {
        pollster::block_on(async {
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
        });
    }

    #[test]
    fn test_load_settings_default_when_missing() {
        pollster::block_on(async {
            let _env = TestEnv::new();
            let loaded = load_app_settings().await.expect("failed to load settings");
            assert_eq!(loaded, AppSettings::default());
        });
    }

    #[test]
    fn test_auth_session_roundtrip() {
        pollster::block_on(async {
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
        });
    }

    #[test]
    fn test_audio_roundtrip() {
        pollster::block_on(async {
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

            let read = read_editor_music_bytes(None, "test1.ogg").expect("failed to read audio");
            assert_eq!(read, data1);
        });
    }

    #[test]
    fn test_read_music_bytes_builtin_fallback() {
        pollster::block_on(async {
            let _env = TestEnv::new();

            // Should return None for unknown builtin and unknown user audio
            let read = read_editor_music_bytes(Some("UnknownLevel"), "missing.ogg");
            assert!(read.is_none());

            // Save to user audio and check fallback
            let data = b"user-audio";
            save_audio("user.ogg", data).await.unwrap();
            let read = read_editor_music_bytes(Some("UnknownLevel"), "user.ogg").unwrap();
            assert_eq!(read, data);
        });
    }

    #[test]
    fn test_waveform_cache_roundtrip() {
        pollster::block_on(async {
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
        });
    }
}
