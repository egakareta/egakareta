/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

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
fn audio_storage_dir() -> Result<std::path::PathBuf, String> {
    let dirs = directories::ProjectDirs::from("com", "egakareta", "egakareta")
        .ok_or_else(|| "Failed to resolve application data directory".to_string())?;
    Ok(dirs.data_local_dir().join("user_audio"))
}

#[cfg(not(target_arch = "wasm32"))]
fn settings_file_path() -> Result<std::path::PathBuf, String> {
    let dirs = directories::ProjectDirs::from("com", "egakareta", "egakareta")
        .ok_or_else(|| "Failed to resolve application data directory".to_string())?;
    Ok(dirs.data_local_dir().join("settings.json"))
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
    use super::{audio_storage_dir, settings_file_path};

    #[test]
    fn resolves_audio_storage_directory() {
        let dir = audio_storage_dir().expect("audio storage dir should resolve");
        assert!(dir.ends_with("user_audio"));
    }

    #[test]
    fn resolves_settings_file_path() {
        let path = settings_file_path().expect("settings file should resolve");
        assert!(path.ends_with("settings.json"));
    }
}
