/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::collections::HashMap;

use crate::audio_service::PersistentWaveformCacheEntry;
use crate::error_utils::MapErrContext;
use crate::types::{AppSettings, AuthSession};

#[cfg(target_arch = "wasm32")]
const AUDIO_DB_NAME: &str = "egakareta-audio";
#[cfg(target_arch = "wasm32")]
const AUDIO_STORE_NAME: &str = "audio";
#[cfg(target_arch = "wasm32")]
const SETTINGS_DB_NAME: &str = "egakareta-settings";
#[cfg(target_arch = "wasm32")]
const SETTINGS_STORE_NAME: &str = "settings";
#[cfg(target_arch = "wasm32")]
const SETTINGS_KEY: &str = "app";
#[cfg(target_arch = "wasm32")]
const AUTH_SESSION_KEY: &str = "auth_session";
#[cfg(target_arch = "wasm32")]
const WAVEFORM_CACHE_KEY_PREFIX: &str = "waveform:";

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub(crate) trait AudioStorage {
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

pub(crate) struct PlatformAudioStorage;

pub(crate) async fn save_audio(filename: &str, data: &[u8]) -> Result<(), String> {
    PlatformAudioStorage.save_audio(filename, data).await
}

pub(crate) async fn load_all_audio() -> HashMap<String, Vec<u8>> {
    PlatformAudioStorage.load_all_audio().await
}

pub(crate) fn read_editor_music_bytes(
    level_name: Option<&str>,
    music_source: &str,
) -> Option<Vec<u8>> {
    PlatformAudioStorage.read_editor_music_bytes(level_name, music_source)
}

pub(crate) async fn save_waveform(
    cache_key: &str,
    entry: &PersistentWaveformCacheEntry,
) -> Result<(), String> {
    PlatformAudioStorage.save_waveform(cache_key, entry).await
}

pub(crate) async fn load_waveform(cache_key: &str) -> Option<PersistentWaveformCacheEntry> {
    PlatformAudioStorage.load_waveform(cache_key).await
}

fn encode_waveform_entry(entry: &PersistentWaveformCacheEntry) -> Result<Vec<u8>, String> {
    serde_cbor::to_vec(entry).ctx("Waveform cache encode failed")
}

fn decode_waveform_entry(bytes: &[u8]) -> Result<PersistentWaveformCacheEntry, String> {
    serde_cbor::from_slice(bytes).ctx("Waveform cache decode failed")
}

pub(crate) async fn load_app_settings() -> Result<AppSettings, String> {
    load_platform_app_settings().await
}

pub(crate) async fn save_app_settings(settings: &AppSettings) -> Result<(), String> {
    save_platform_app_settings(settings).await
}

pub(crate) async fn load_auth_session() -> Result<Option<AuthSession>, String> {
    load_platform_auth_session().await
}

pub(crate) async fn save_auth_session(session: &AuthSession) -> Result<(), String> {
    save_platform_auth_session(session).await
}

pub(crate) async fn clear_auth_session() -> Result<(), String> {
    clear_platform_auth_session().await
}

#[cfg(target_arch = "wasm32")]
async fn open_audio_db() -> Result<rexie::Rexie, String> {
    use rexie::{ObjectStore, Rexie};

    Rexie::builder(AUDIO_DB_NAME)
        .version(1)
        .add_object_store(ObjectStore::new(AUDIO_STORE_NAME))
        .build()
        .await
        .ctx("IndexedDB open failed")
}

#[cfg(target_arch = "wasm32")]
async fn open_settings_db() -> Result<rexie::Rexie, String> {
    use rexie::{ObjectStore, Rexie};

    Rexie::builder(SETTINGS_DB_NAME)
        .version(1)
        .add_object_store(ObjectStore::new(SETTINGS_STORE_NAME))
        .build()
        .await
        .ctx("IndexedDB settings open failed")
}

#[cfg(target_arch = "wasm32")]
async fn load_platform_app_settings() -> Result<AppSettings, String> {
    use rexie::TransactionMode;
    use wasm_bindgen::JsValue;

    let db = open_settings_db().await?;
    let tx = db
        .transaction(&[SETTINGS_STORE_NAME], TransactionMode::ReadOnly)
        .ctx("IndexedDB settings transaction failed")?;
    let store = tx
        .store(SETTINGS_STORE_NAME)
        .ctx("IndexedDB settings store open failed")?;

    let value = store
        .get(JsValue::from_str(SETTINGS_KEY))
        .await
        .ctx("IndexedDB settings read failed")?;

    tx.done()
        .await
        .ctx("IndexedDB settings transaction completion failed")?;

    let Some(value) = value else {
        return Ok(AppSettings::default());
    };

    if value.is_undefined() || value.is_null() {
        return Ok(AppSettings::default());
    }

    let Some(settings_json) = value.as_string() else {
        return Ok(AppSettings::default());
    };

    serde_json::from_str(&settings_json).ctx("Settings JSON parse failed")
}

#[cfg(target_arch = "wasm32")]
async fn save_platform_app_settings(settings: &AppSettings) -> Result<(), String> {
    use rexie::TransactionMode;
    use wasm_bindgen::JsValue;

    let settings_json =
        serde_json::to_string_pretty(settings).ctx("Settings JSON encode failed")?;

    let db = open_settings_db().await?;
    let tx = db
        .transaction(&[SETTINGS_STORE_NAME], TransactionMode::ReadWrite)
        .ctx("IndexedDB settings transaction failed")?;
    let store = tx
        .store(SETTINGS_STORE_NAME)
        .ctx("IndexedDB settings store open failed")?;

    store
        .put(
            &JsValue::from_str(&settings_json),
            Some(&JsValue::from_str(SETTINGS_KEY)),
        )
        .await
        .ctx("IndexedDB settings write failed")?;

    tx.done().await.ctx("IndexedDB settings commit failed")?;

    Ok(())
}

#[cfg(target_arch = "wasm32")]
async fn load_platform_auth_session() -> Result<Option<AuthSession>, String> {
    use rexie::TransactionMode;
    use wasm_bindgen::JsValue;

    let db = open_settings_db().await?;
    let tx = db
        .transaction(&[SETTINGS_STORE_NAME], TransactionMode::ReadOnly)
        .ctx("IndexedDB auth transaction failed")?;
    let store = tx
        .store(SETTINGS_STORE_NAME)
        .ctx("IndexedDB auth store open failed")?;
    let value = store
        .get(JsValue::from_str(AUTH_SESSION_KEY))
        .await
        .ctx("IndexedDB auth read failed")?;

    tx.done()
        .await
        .ctx("IndexedDB auth transaction completion failed")?;

    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_undefined() || value.is_null() {
        return Ok(None);
    }

    let Some(session_json) = value.as_string() else {
        return Ok(None);
    };

    serde_json::from_str(&session_json)
        .map(Some)
        .ctx("Auth session JSON parse failed")
}

#[cfg(target_arch = "wasm32")]
async fn save_platform_auth_session(session: &AuthSession) -> Result<(), String> {
    use rexie::TransactionMode;
    use wasm_bindgen::JsValue;

    let session_json =
        serde_json::to_string_pretty(session).ctx("Auth session JSON encode failed")?;
    let db = open_settings_db().await?;
    let tx = db
        .transaction(&[SETTINGS_STORE_NAME], TransactionMode::ReadWrite)
        .ctx("IndexedDB auth transaction failed")?;
    let store = tx
        .store(SETTINGS_STORE_NAME)
        .ctx("IndexedDB auth store open failed")?;

    store
        .put(
            &JsValue::from_str(&session_json),
            Some(&JsValue::from_str(AUTH_SESSION_KEY)),
        )
        .await
        .ctx("IndexedDB auth write failed")?;
    tx.done().await.ctx("IndexedDB auth commit failed")?;

    Ok(())
}

#[cfg(target_arch = "wasm32")]
async fn clear_platform_auth_session() -> Result<(), String> {
    use rexie::TransactionMode;
    use wasm_bindgen::JsValue;

    let db = open_settings_db().await?;
    let tx = db
        .transaction(&[SETTINGS_STORE_NAME], TransactionMode::ReadWrite)
        .ctx("IndexedDB auth transaction failed")?;
    let store = tx
        .store(SETTINGS_STORE_NAME)
        .ctx("IndexedDB auth store open failed")?;
    store
        .delete(JsValue::from_str(AUTH_SESSION_KEY))
        .await
        .ctx("IndexedDB auth delete failed")?;
    tx.done().await.ctx("IndexedDB auth commit failed")?;

    Ok(())
}

#[cfg(target_arch = "wasm32")]
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl AudioStorage for PlatformAudioStorage {
    async fn save_audio(&self, filename: &str, data: &[u8]) -> Result<(), String> {
        use rexie::TransactionMode;
        use wasm_bindgen::JsValue;

        let db = open_audio_db().await?;
        let tx = db
            .transaction(&[AUDIO_STORE_NAME], TransactionMode::ReadWrite)
            .ctx("IndexedDB transaction failed")?;
        let store = tx
            .store(AUDIO_STORE_NAME)
            .ctx("IndexedDB store open failed")?;

        let value = js_sys::Uint8Array::from(data);
        store
            .put(&value.into(), Some(&JsValue::from_str(filename)))
            .await
            .ctx("IndexedDB write failed")?;
        tx.done().await.ctx("IndexedDB commit failed")?;

        Ok(())
    }

    async fn load_all_audio(&self) -> HashMap<String, Vec<u8>> {
        HashMap::new()
    }

    fn read_editor_music_bytes(
        &self,
        level_name: Option<&str>,
        music_source: &str,
    ) -> Option<Vec<u8>> {
        let _ = (level_name, music_source);
        None
    }

    async fn save_waveform(
        &self,
        cache_key: &str,
        entry: &PersistentWaveformCacheEntry,
    ) -> Result<(), String> {
        use rexie::TransactionMode;
        use wasm_bindgen::JsValue;

        let encoded = encode_waveform_entry(entry)?;
        let db = open_audio_db().await?;
        let tx = db
            .transaction(&[AUDIO_STORE_NAME], TransactionMode::ReadWrite)
            .ctx("IndexedDB waveform transaction failed")?;
        let store = tx
            .store(AUDIO_STORE_NAME)
            .ctx("IndexedDB waveform store open failed")?;

        let value = js_sys::Uint8Array::from(encoded.as_slice());
        let storage_key = format!("{WAVEFORM_CACHE_KEY_PREFIX}{cache_key}");
        store
            .put(&value.into(), Some(&JsValue::from_str(&storage_key)))
            .await
            .ctx("IndexedDB waveform write failed")?;
        tx.done().await.ctx("IndexedDB waveform commit failed")?;

        Ok(())
    }

    async fn load_waveform(&self, cache_key: &str) -> Option<PersistentWaveformCacheEntry> {
        use rexie::TransactionMode;
        use wasm_bindgen::JsValue;

        let db = open_audio_db().await.ok()?;
        let tx = db
            .transaction(&[AUDIO_STORE_NAME], TransactionMode::ReadOnly)
            .ok()?;
        let store = tx.store(AUDIO_STORE_NAME).ok()?;
        let storage_key = format!("{WAVEFORM_CACHE_KEY_PREFIX}{cache_key}");
        let value = store.get(JsValue::from_str(&storage_key)).await.ok()??;
        let _ = tx.done().await;
        if value.is_undefined() || value.is_null() {
            return None;
        }

        let array = js_sys::Uint8Array::new(&value);
        let mut bytes = vec![0; array.length() as usize];
        array.copy_to(bytes.as_mut_slice());
        decode_waveform_entry(&bytes).ok()
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn storage_root_dir() -> Result<std::path::PathBuf, String> {
    if let Ok(root) = std::env::var("EGAKARETA_STORAGE_ROOT") {
        return Ok(std::path::PathBuf::from(root));
    }
    let dirs = directories::ProjectDirs::from("com", "egakareta", "egakareta")
        .ok_or_else(|| "Failed to resolve application data directory".to_string())?;
    Ok(dirs.data_local_dir().to_path_buf())
}

#[cfg(not(target_arch = "wasm32"))]
fn audio_storage_dir() -> Result<std::path::PathBuf, String> {
    Ok(storage_root_dir()?.join("user_audio"))
}

#[cfg(not(target_arch = "wasm32"))]
fn waveform_storage_dir() -> Result<std::path::PathBuf, String> {
    Ok(storage_root_dir()?.join("waveforms"))
}

#[cfg(not(target_arch = "wasm32"))]
fn waveform_storage_path(cache_key: &str) -> Result<std::path::PathBuf, String> {
    Ok(waveform_storage_dir()?.join(format!("{cache_key}.cbor")))
}

#[cfg(not(target_arch = "wasm32"))]
fn settings_file_path() -> Result<std::path::PathBuf, String> {
    Ok(storage_root_dir()?.join("settings.json"))
}

#[cfg(not(target_arch = "wasm32"))]
fn auth_session_file_path() -> Result<std::path::PathBuf, String> {
    Ok(storage_root_dir()?.join("auth_session.json"))
}

#[cfg(not(target_arch = "wasm32"))]
async fn load_platform_app_settings() -> Result<AppSettings, String> {
    let path = settings_file_path()?;
    if !path.exists() {
        return Ok(AppSettings::default());
    }

    let settings_json = std::fs::read_to_string(path).map_err(|err| err.to_string())?;
    serde_json::from_str(&settings_json).ctx("Settings JSON parse failed")
}

#[cfg(not(target_arch = "wasm32"))]
async fn save_platform_app_settings(settings: &AppSettings) -> Result<(), String> {
    let path = settings_file_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }

    let settings_json =
        serde_json::to_string_pretty(settings).ctx("Settings JSON encode failed")?;
    std::fs::write(path, settings_json).map_err(|err| err.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
async fn load_platform_auth_session() -> Result<Option<AuthSession>, String> {
    let path = auth_session_file_path()?;
    if !path.exists() {
        return Ok(None);
    }

    let session_json = std::fs::read_to_string(path).map_err(|err| err.to_string())?;
    serde_json::from_str(&session_json)
        .map(Some)
        .ctx("Auth session JSON parse failed")
}

#[cfg(not(target_arch = "wasm32"))]
async fn save_platform_auth_session(session: &AuthSession) -> Result<(), String> {
    let path = auth_session_file_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }

    let session_json =
        serde_json::to_string_pretty(session).ctx("Auth session JSON encode failed")?;
    std::fs::write(path, session_json).map_err(|err| err.to_string())
}

#[cfg(not(target_arch = "wasm32"))]
async fn clear_platform_auth_session() -> Result<(), String> {
    let path = auth_session_file_path()?;
    if path.exists() {
        std::fs::remove_file(path).map_err(|err| err.to_string())?;
    }
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl AudioStorage for PlatformAudioStorage {
    async fn save_audio(&self, filename: &str, data: &[u8]) -> Result<(), String> {
        let mut path = audio_storage_dir()?;
        std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
        path.push(filename);
        std::fs::write(path, data).map_err(|e| e.to_string())?;
        Ok(())
    }

    async fn load_all_audio(&self) -> HashMap<String, Vec<u8>> {
        let mut audio_map = HashMap::new();
        let path = match audio_storage_dir() {
            Ok(path) => path,
            Err(_) => return audio_map,
        };

        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type() {
                    if file_type.is_file() {
                        let filename = entry.file_name().to_string_lossy().to_string();
                        if let Ok(bytes) = std::fs::read(entry.path()) {
                            audio_map.insert(filename, bytes);
                        }
                    }
                }
            }
        }

        audio_map
    }

    fn read_editor_music_bytes(
        &self,
        level_name: Option<&str>,
        music_source: &str,
    ) -> Option<Vec<u8>> {
        if let Some(name) = level_name {
            if let Some(bytes) = crate::level_repository::get_builtin_audio(name, music_source) {
                return Some(bytes.to_vec());
            }
        }

        let mut path = audio_storage_dir().ok()?;
        path.push(music_source);
        std::fs::read(path).ok()
    }

    async fn save_waveform(
        &self,
        cache_key: &str,
        entry: &PersistentWaveformCacheEntry,
    ) -> Result<(), String> {
        let path = waveform_storage_path(cache_key)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let encoded = encode_waveform_entry(entry)?;
        std::fs::write(path, encoded).map_err(|error| error.to_string())
    }

    async fn load_waveform(&self, cache_key: &str) -> Option<PersistentWaveformCacheEntry> {
        let path = waveform_storage_path(cache_key).ok()?;
        let bytes = std::fs::read(path).ok()?;
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
    fn resolves_audio_storage_directory() {
        let _env = TestEnv::new();
        let dir = audio_storage_dir().expect("audio storage dir should resolve");
        assert!(dir.to_string_lossy().contains("user_audio"));
    }

    #[test]
    fn resolves_settings_file_path() {
        let _env = TestEnv::new();
        let path = settings_file_path().expect("settings file should resolve");
        assert!(path.to_string_lossy().contains("settings.json"));
    }

    #[test]
    fn resolves_auth_session_file_path() {
        let _env = TestEnv::new();
        let path = auth_session_file_path().expect("auth session file should resolve");
        assert!(path.to_string_lossy().contains("auth_session.json"));
    }

    #[test]
    fn resolves_waveform_storage_directory() {
        let _env = TestEnv::new();
        let dir = waveform_storage_dir().expect("waveform storage dir should resolve");
        assert!(dir.to_string_lossy().contains("waveforms"));
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
            let session = AuthSession {
                session: crate::types::AuthSessionTokens {
                    access_token: "access".to_string(),
                    refresh_token: "refresh".to_string(),
                    expires_at: Some(123),
                    token_type: "bearer".to_string(),
                },
                user: crate::types::AuthUser {
                    id: "user-id".to_string(),
                    email: Some("player@example.com".to_string()),
                },
                profile: Some(crate::types::AuthProfile {
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
