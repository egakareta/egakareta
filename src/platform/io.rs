/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use crate::platform::storage;
use crate::types::AppSettings;

pub(crate) fn save_level_export(filename: &str, data: &[u8]) -> Result<(), String> {
    #[cfg(target_arch = "wasm32")]
    {
        use js_sys::Uint8Array;
        use wasm_bindgen::JsCast;
        use web_sys::HtmlAnchorElement;

        let document = gloo_utils::document();
        let uint8_array = Uint8Array::new_with_length(data.len() as u32);
        uint8_array.copy_from(data);
        let blob = gloo_file::Blob::new(uint8_array.buffer());
        let url = gloo_file::ObjectUrl::from(blob);

        let anchor = document
            .create_element("a")
            .map_err(|error| format!("Failed to create anchor: {:?}", error))?
            .dyn_into::<HtmlAnchorElement>()
            .map_err(|error| format!("Failed to cast anchor: {:?}", error))?;

        anchor.set_href(&url.to_string());
        anchor.set_download(filename);
        anchor.click();

        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        std::fs::write(filename, data).map_err(|error| error.to_string())
    }
}

pub(crate) fn log_platform_error(message: &str) {
    #[cfg(target_arch = "wasm32")]
    gloo_console::error!(message);

    #[cfg(not(target_arch = "wasm32"))]
    log::error!("{}", message);
}

pub(crate) async fn pick_audio_file() -> Option<(String, Vec<u8>)> {
    let file = rfd::AsyncFileDialog::new()
        .add_filter("Audio", &["mp3", "wav", "ogg"])
        .pick_file()
        .await?;

    let filename = file.file_name();
    let data = file.read().await;
    Some((filename, data))
}

pub(crate) async fn pick_level_file() -> Option<Vec<u8>> {
    let file = rfd::AsyncFileDialog::new()
        .add_filter("Level Archive", &["egz"])
        .add_filter("Level Binary", &["egb"])
        .pick_file()
        .await?;

    Some(file.read().await)
}

pub(crate) async fn save_audio_to_storage(filename: &str, data: &[u8]) -> Result<(), String> {
    #[cfg(all(test, not(target_arch = "wasm32")))]
    if let Some(result) = test_hooks::save_audio_result() {
        return result;
    }

    storage::save_audio(filename, data).await
}

pub(crate) async fn load_all_local_audio() -> std::collections::HashMap<String, Vec<u8>> {
    #[cfg(all(test, not(target_arch = "wasm32")))]
    if let Some(result) = test_hooks::load_audio_result() {
        return result;
    }

    storage::load_all_audio().await
}

pub(crate) fn read_editor_music_bytes(
    level_name: Option<&str>,
    music_source: &str,
) -> Option<Vec<u8>> {
    #[cfg(all(test, not(target_arch = "wasm32")))]
    if let Some(result) = test_hooks::read_editor_music_result() {
        return result;
    }

    storage::read_editor_music_bytes(level_name, music_source)
}

pub(crate) async fn load_app_settings_from_storage() -> Result<AppSettings, String> {
    #[cfg(all(test, not(target_arch = "wasm32")))]
    if let Some(result) = test_hooks::load_settings_result() {
        return result;
    }

    storage::load_app_settings().await
}

pub(crate) async fn save_app_settings_to_storage(settings: &AppSettings) -> Result<(), String> {
    #[cfg(all(test, not(target_arch = "wasm32")))]
    if let Some(result) = test_hooks::save_settings_result() {
        return result;
    }

    storage::save_app_settings(settings).await
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod test_hooks {
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};

    use crate::types::AppSettings;

    #[derive(Clone, Default)]
    pub(crate) struct IoHooks {
        pub(crate) save_audio_result: Option<Result<(), String>>,
        pub(crate) load_audio_result: Option<HashMap<String, Vec<u8>>>,
        pub(crate) read_editor_music_result: Option<Option<Vec<u8>>>,
        pub(crate) load_settings_result: Option<Result<AppSettings, String>>,
        pub(crate) save_settings_result: Option<Result<(), String>>,
    }

    fn hooks_state() -> &'static Mutex<IoHooks> {
        static STATE: OnceLock<Mutex<IoHooks>> = OnceLock::new();
        STATE.get_or_init(|| Mutex::new(IoHooks::default()))
    }

    pub(crate) fn with_hooks_mut<T>(update: impl FnOnce(&mut IoHooks) -> T) -> T {
        let mut guard = hooks_state()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        update(&mut guard)
    }

    pub(crate) fn reset() {
        with_hooks_mut(|hooks| *hooks = IoHooks::default());
    }

    pub(crate) fn save_audio_result() -> Option<Result<(), String>> {
        with_hooks_mut(|hooks| hooks.save_audio_result.clone())
    }

    pub(crate) fn load_audio_result() -> Option<HashMap<String, Vec<u8>>> {
        with_hooks_mut(|hooks| hooks.load_audio_result.clone())
    }

    pub(crate) fn read_editor_music_result() -> Option<Option<Vec<u8>>> {
        with_hooks_mut(|hooks| hooks.read_editor_music_result.clone())
    }

    pub(crate) fn load_settings_result() -> Option<Result<AppSettings, String>> {
        with_hooks_mut(|hooks| hooks.load_settings_result.clone())
    }

    pub(crate) fn save_settings_result() -> Option<Result<(), String>> {
        with_hooks_mut(|hooks| hooks.save_settings_result.clone())
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use std::collections::HashMap;
    use std::fs;
    use std::sync::{Mutex, OnceLock};

    use super::{
        load_all_local_audio, load_app_settings_from_storage, save_app_settings_to_storage,
        save_audio_to_storage, save_level_export, test_hooks,
    };
    use crate::types::AppSettings;

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

    #[test]
    fn save_level_export_writes_bytes_to_file() {
        let temp_dir = tempfile::tempdir().expect("temp dir should be created");
        let export_path = temp_dir.path().join("level.egb");
        let payload = b"egakareta-level-bytes";

        save_level_export(export_path.to_string_lossy().as_ref(), payload)
            .expect("save_level_export should succeed");

        let loaded = fs::read(&export_path).expect("saved file should be readable");
        assert_eq!(loaded, payload);
    }

    #[test]
    fn save_level_export_returns_error_when_parent_directory_is_missing() {
        let temp_dir = tempfile::tempdir().expect("temp dir should be created");
        let missing_parent = temp_dir.path().join("missing-parent").join("level.egb");

        let result = save_level_export(missing_parent.to_string_lossy().as_ref(), b"bytes");

        assert!(result.is_err(), "missing parent directory should fail");
    }

    #[test]
    fn save_audio_to_storage_returns_ok_when_storage_layer_succeeds() {
        let _lock = shared_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _reset_guard = HookResetGuard::new();
        test_hooks::with_hooks_mut(|hooks| {
            hooks.save_audio_result = Some(Ok(()));
        });

        let result = pollster::block_on(save_audio_to_storage("music.ogg", b"audio-bytes"));
        assert!(result.is_ok(), "save should return the delegated success");
    }

    #[test]
    fn save_audio_to_storage_returns_error_when_storage_layer_fails() {
        let _lock = shared_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _reset_guard = HookResetGuard::new();
        test_hooks::with_hooks_mut(|hooks| {
            hooks.save_audio_result = Some(Err("disk full".to_string()));
        });

        let result = pollster::block_on(save_audio_to_storage("music.ogg", b"audio-bytes"));
        assert_eq!(result, Err("disk full".to_string()));
    }

    #[test]
    fn load_all_local_audio_returns_storage_layer_payload() {
        let _lock = shared_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _reset_guard = HookResetGuard::new();
        let mut expected_audio = HashMap::new();
        expected_audio.insert("track.ogg".to_string(), vec![1, 2, 3, 4]);
        test_hooks::with_hooks_mut(|hooks| {
            hooks.load_audio_result = Some(expected_audio.clone());
        });

        let loaded = pollster::block_on(load_all_local_audio());
        assert_eq!(loaded, expected_audio);
    }

    #[test]
    fn save_and_load_app_settings_round_trip_through_delegate_layer() {
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

        let save_result = pollster::block_on(save_app_settings_to_storage(&settings));
        assert_eq!(save_result, Ok(()));

        let loaded = pollster::block_on(load_app_settings_from_storage());
        assert_eq!(loaded, Ok(settings));
    }

    #[test]
    fn load_app_settings_from_storage_returns_error_when_storage_layer_fails() {
        let _lock = shared_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _reset_guard = HookResetGuard::new();
        test_hooks::with_hooks_mut(|hooks| {
            hooks.load_settings_result =
                Some(Err("Settings JSON parse failed: invalid JSON".into()));
        });

        let result = pollster::block_on(load_app_settings_from_storage());
        assert!(
            matches!(&result, Err(error) if error.contains("Settings JSON parse failed")),
            "expected delegated parse error, got: {result:?}"
        );
    }
}
