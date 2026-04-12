/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::collections::HashMap;

use crate::types::AppSettings;

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

pub(crate) async fn load_app_settings() -> Result<AppSettings, String> {
    load_platform_app_settings().await
}

pub(crate) async fn save_app_settings(settings: &AppSettings) -> Result<(), String> {
    save_platform_app_settings(settings).await
}

#[cfg(target_arch = "wasm32")]
async fn open_audio_db() -> Result<rexie::Rexie, String> {
    use rexie::{ObjectStore, Rexie};

    Rexie::builder(AUDIO_DB_NAME)
        .version(1)
        .add_object_store(ObjectStore::new(AUDIO_STORE_NAME))
        .build()
        .await
        .map_err(|err| format!("IndexedDB open failed: {:?}", err))
}

#[cfg(target_arch = "wasm32")]
async fn open_settings_db() -> Result<rexie::Rexie, String> {
    use rexie::{ObjectStore, Rexie};

    Rexie::builder(SETTINGS_DB_NAME)
        .version(1)
        .add_object_store(ObjectStore::new(SETTINGS_STORE_NAME))
        .build()
        .await
        .map_err(|err| format!("IndexedDB settings open failed: {:?}", err))
}

#[cfg(target_arch = "wasm32")]
async fn load_platform_app_settings() -> Result<AppSettings, String> {
    use rexie::TransactionMode;
    use wasm_bindgen::JsValue;

    let db = open_settings_db().await?;
    let tx = db
        .transaction(&[SETTINGS_STORE_NAME], TransactionMode::ReadOnly)
        .map_err(|err| format!("IndexedDB settings transaction failed: {:?}", err))?;
    let store = tx
        .store(SETTINGS_STORE_NAME)
        .map_err(|err| format!("IndexedDB settings store open failed: {:?}", err))?;

    let value = store
        .get(JsValue::from_str(SETTINGS_KEY))
        .await
        .map_err(|err| format!("IndexedDB settings read failed: {:?}", err))?;

    tx.done().await.map_err(|err| {
        format!(
            "IndexedDB settings transaction completion failed: {:?}",
            err
        )
    })?;

    let Some(value) = value else {
        return Ok(AppSettings::default());
    };

    if value.is_undefined() || value.is_null() {
        return Ok(AppSettings::default());
    }

    let Some(settings_json) = value.as_string() else {
        return Ok(AppSettings::default());
    };

    serde_json::from_str(&settings_json).map_err(|err| format!("Settings JSON parse failed: {err}"))
}

#[cfg(target_arch = "wasm32")]
async fn save_platform_app_settings(settings: &AppSettings) -> Result<(), String> {
    use rexie::TransactionMode;
    use wasm_bindgen::JsValue;

    let settings_json = serde_json::to_string_pretty(settings)
        .map_err(|err| format!("Settings JSON encode failed: {err}"))?;

    let db = open_settings_db().await?;
    let tx = db
        .transaction(&[SETTINGS_STORE_NAME], TransactionMode::ReadWrite)
        .map_err(|err| format!("IndexedDB settings transaction failed: {:?}", err))?;
    let store = tx
        .store(SETTINGS_STORE_NAME)
        .map_err(|err| format!("IndexedDB settings store open failed: {:?}", err))?;

    store
        .put(
            &JsValue::from_str(&settings_json),
            Some(&JsValue::from_str(SETTINGS_KEY)),
        )
        .await
        .map_err(|err| format!("IndexedDB settings write failed: {:?}", err))?;

    tx.done()
        .await
        .map_err(|err| format!("IndexedDB settings commit failed: {:?}", err))?;

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
            .map_err(|err| format!("IndexedDB transaction failed: {:?}", err))?;
        let store = tx
            .store(AUDIO_STORE_NAME)
            .map_err(|err| format!("IndexedDB store open failed: {:?}", err))?;

        let value = js_sys::Uint8Array::from(data);
        store
            .put(&value.into(), Some(&JsValue::from_str(filename)))
            .await
            .map_err(|err| format!("IndexedDB write failed: {:?}", err))?;
        tx.done()
            .await
            .map_err(|err| format!("IndexedDB commit failed: {:?}", err))?;

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
fn settings_file_path() -> Result<std::path::PathBuf, String> {
    Ok(storage_root_dir()?.join("settings.json"))
}

#[cfg(not(target_arch = "wasm32"))]
async fn load_platform_app_settings() -> Result<AppSettings, String> {
    let path = settings_file_path()?;
    if !path.exists() {
        return Ok(AppSettings::default());
    }

    let settings_json = std::fs::read_to_string(path).map_err(|err| err.to_string())?;
    serde_json::from_str(&settings_json).map_err(|err| format!("Settings JSON parse failed: {err}"))
}

#[cfg(not(target_arch = "wasm32"))]
async fn save_platform_app_settings(settings: &AppSettings) -> Result<(), String> {
    let path = settings_file_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }

    let settings_json = serde_json::to_string_pretty(settings)
        .map_err(|err| format!("Settings JSON encode failed: {err}"))?;
    std::fs::write(path, settings_json).map_err(|err| err.to_string())
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
}
