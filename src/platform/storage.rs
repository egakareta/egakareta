use std::collections::HashMap;

#[cfg(target_arch = "wasm32")]
const AUDIO_DB_NAME: &str = "line-dash-audio";
#[cfg(target_arch = "wasm32")]
const AUDIO_STORE_NAME: &str = "audio";

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
    let dirs = directories::ProjectDirs::from("com", "line_dash", "line_dash")
        .ok_or_else(|| "Failed to resolve application data directory".to_string())?;
    Ok(dirs.data_local_dir().join("user_audio"))
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
            let audio_path = format!("assets/levels/{}/{}", name, music_source);
            if let Ok(bytes) = std::fs::read(audio_path) {
                return Some(bytes);
            }
        }

        let mut path = audio_storage_dir().ok()?;
        path.push(music_source);
        std::fs::read(path).ok()
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::audio_storage_dir;

    #[test]
    fn resolves_audio_storage_directory() {
        let dir = audio_storage_dir().expect("audio storage dir should resolve");
        assert!(dir.ends_with("user_audio"));
    }
}
