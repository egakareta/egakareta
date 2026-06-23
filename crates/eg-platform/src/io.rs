/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/

use eg_types::AppSettings;
use eg_types::PersistentWaveformCacheEntry;

use crate::storage;

pub async fn save_level_export(filename: &str, data: &[u8]) -> Result<(), String> {
    #[cfg(test)]
    {
        return std::fs::write(filename, data)
            .map_err(|error| format!("Failed to write file: {error}"));
    }

    #[cfg(not(test))]
    {
        let dialog = rfd::AsyncFileDialog::new()
            .set_file_name(filename)
            .add_filter("Level Data", &["dat", "json", "bin"]); // Optional extension filters

        let file_handle = dialog
            .save_file()
            .await
            .ok_or_else(|| "User cancelled the save operation".to_string())?;

        file_handle
            .write(data)
            .await
            .map_err(|error| format!("Failed to write file: {:?}", error))?;

        Ok(())
    }
}

pub async fn pick_audio_file() -> Option<(String, Vec<u8>)> {
    #[cfg(test)]
    if let Some(result) = crate::test_hooks::pick_audio_result() {
        return result;
    }

    let file = rfd::AsyncFileDialog::new()
        .add_filter("Audio", &["mp3", "wav", "ogg"])
        .pick_file()
        .await?;

    let filename = file.file_name();
    let data = file.read().await;
    Some((filename, data))
}

/// Opens a platform-native file picker for level imports.
///
/// Supports `.egz` archives and `.egb` binary metadata files.
/// Returns `None` when the user cancels the dialog; otherwise returns full file bytes.
pub async fn pick_level_file() -> Option<Vec<u8>> {
    #[cfg(test)]
    if let Some(result) = crate::test_hooks::pick_level_result() {
        return result;
    }

    let file = rfd::AsyncFileDialog::new()
        .add_filter("Level Archive", &["egz"])
        .add_filter("Level Binary", &["egb"])
        .pick_file()
        .await?;

    Some(file.read().await)
}

pub async fn save_audio_to_storage(filename: &str, data: &[u8]) -> Result<(), String> {
    #[cfg(test)]
    if let Some(result) = crate::test_hooks::save_audio_result() {
        return result;
    }

    storage::save_audio(filename, data).await
}

pub async fn load_all_local_audio() -> std::collections::HashMap<String, Vec<u8>> {
    #[cfg(test)]
    if let Some(result) = crate::test_hooks::load_audio_result() {
        return result;
    }

    storage::load_all_audio().await
}

pub fn read_editor_music_bytes(music_source: &str) -> Option<Vec<u8>> {
    #[cfg(test)]
    if let Some(result) = crate::test_hooks::read_editor_music_result() {
        return result;
    }

    pollster::block_on(storage::read_editor_music_bytes(music_source))
        .ok()
        .flatten()
}

pub async fn save_waveform_to_storage(
    cache_key: &str,
    entry: &PersistentWaveformCacheEntry,
) -> Result<(), String> {
    #[cfg(test)]
    if let Some(result) = crate::test_hooks::save_waveform_result() {
        return result;
    }

    storage::save_waveform(cache_key, entry).await
}

pub async fn load_waveform_from_storage(cache_key: &str) -> Option<PersistentWaveformCacheEntry> {
    #[cfg(test)]
    if let Some(result) = crate::test_hooks::load_waveform_result() {
        return result;
    }

    storage::load_waveform(cache_key).await
}

pub async fn load_app_settings_from_storage() -> Result<AppSettings, String> {
    #[cfg(test)]
    if let Some(result) = crate::test_hooks::load_settings_result() {
        return result;
    }

    storage::load_app_settings().await
}

pub async fn save_app_settings_to_storage(settings: &AppSettings) -> Result<(), String> {
    #[cfg(test)]
    if let Some(result) = crate::test_hooks::save_settings_result() {
        return result;
    }

    storage::save_app_settings(settings).await
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs;
    use std::sync::{Mutex, OnceLock};

    use super::{
        load_all_local_audio, load_app_settings_from_storage, load_waveform_from_storage,
        save_app_settings_to_storage, save_audio_to_storage, save_level_export,
        save_waveform_to_storage,
    };
    use crate::test_hooks;
    use eg_types::AppSettings;
    use eg_types::PersistentWaveformCacheEntry;

    fn shared_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct HookResetGuard;

    impl HookResetGuard {
        fn new() -> Self {
            test_hooks::reset();
            Self
        }
    }

    impl Drop for HookResetGuard {
        fn drop(&mut self) {
            test_hooks::reset();
        }
    }

    #[tokio::test]
    async fn save_level_export_writes_bytes_to_file() {
        let temp_dir = tempfile::tempdir().expect("temp dir should be created");
        let export_path = temp_dir.path().join("level.egb");
        let payload = b"eg-level-bytes";

        save_level_export(export_path.to_string_lossy().as_ref(), payload)
            .await
            .expect("save_level_export should succeed");

        let loaded = fs::read(&export_path).expect("saved file should be readable");
        assert_eq!(loaded, payload);
    }

    #[tokio::test]
    async fn save_level_export_returns_error_when_parent_directory_is_missing() {
        let temp_dir = tempfile::tempdir().expect("temp dir should be created");
        let missing_parent = temp_dir.path().join("missing-parent").join("level.egb");

        let result = save_level_export(missing_parent.to_string_lossy().as_ref(), b"bytes").await;

        assert!(result.is_err(), "missing parent directory should fail");
    }

    #[tokio::test]
    async fn save_audio_to_storage_returns_ok_when_storage_layer_succeeds() {
        let _lock = shared_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _reset_guard = HookResetGuard::new();
        test_hooks::with_hooks_mut(|hooks| {
            hooks.save_audio_result = Some(Ok(()));
        });

        let result = save_audio_to_storage("music.ogg", b"audio-bytes").await;
        assert!(result.is_ok(), "save should return the delegated success");
    }

    #[tokio::test]
    async fn save_audio_to_storage_returns_error_when_storage_layer_fails() {
        let _lock = shared_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _reset_guard = HookResetGuard::new();
        test_hooks::with_hooks_mut(|hooks| {
            hooks.save_audio_result = Some(Err("disk full".to_string()));
        });

        let result = save_audio_to_storage("music.ogg", b"audio-bytes").await;
        assert_eq!(result, Err("disk full".to_string()));
    }

    #[tokio::test]
    async fn load_all_local_audio_returns_storage_layer_payload() {
        let _lock = shared_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _reset_guard = HookResetGuard::new();
        let mut expected_audio = HashMap::new();
        expected_audio.insert("track.ogg".to_string(), vec![1, 2, 3, 4]);
        test_hooks::with_hooks_mut(|hooks| {
            hooks.load_audio_result = Some(expected_audio.clone());
        });

        let loaded = load_all_local_audio().await;
        assert_eq!(loaded, expected_audio);
    }

    #[tokio::test]
    async fn waveform_storage_helpers_delegate_to_storage_layer() {
        let _lock = shared_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _reset_guard = HookResetGuard::new();
        let entry = PersistentWaveformCacheEntry::new(vec![0.2, 0.4], 48_000, 256);
        test_hooks::with_hooks_mut(|hooks| {
            hooks.save_waveform_result = Some(Ok(()));
            hooks.load_waveform_result = Some(Some(entry.clone()));
        });

        let save_result = save_waveform_to_storage("cache-key", &entry).await;
        assert!(save_result.is_ok(), "waveform save should delegate success");

        let loaded = load_waveform_from_storage("cache-key").await;
        assert_eq!(loaded, Some(entry));
    }

    #[tokio::test]
    async fn save_and_load_app_settings_round_trip_through_delegate_layer() {
        let _lock = shared_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _reset_guard = HookResetGuard::new();
        let mut settings = AppSettings::default();
        settings.editor_snap_to_grid = !settings.editor_snap_to_grid;
        settings.editor_snap_step = 0.5;
        settings.editor_rotation_snap = !settings.editor_rotation_snap;
        let expected = settings.clone();
        test_hooks::with_hooks_mut(|hooks| {
            hooks.save_settings_result = Some(Ok(()));
            hooks.load_settings_result = Some(Ok(expected.clone()));
        });

        let save_result = save_app_settings_to_storage(&settings).await;
        assert_eq!(save_result, Ok(()));

        let loaded = load_app_settings_from_storage().await;
        assert_eq!(loaded, Ok(settings));
    }

    #[tokio::test]
    async fn load_app_settings_from_storage_returns_error_when_storage_layer_fails() {
        let _lock = shared_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _reset_guard = HookResetGuard::new();
        test_hooks::with_hooks_mut(|hooks| {
            hooks.load_settings_result =
                Some(Err("Settings JSON parse failed: invalid JSON".into()));
        });

        let result = load_app_settings_from_storage().await;
        assert!(
            matches!(&result, Err(error) if error.contains("Settings JSON parse failed")),
            "expected delegated parse error, got: {result:?}"
        );
    }
}
