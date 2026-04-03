/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

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

pub(crate) async fn save_audio_to_storage(filename: &str, data: &[u8]) -> Result<(), String> {
    storage::save_audio(filename, data).await
}

pub(crate) async fn load_all_local_audio() -> std::collections::HashMap<String, Vec<u8>> {
    storage::load_all_audio().await
}

pub(crate) fn read_editor_music_bytes(
    level_name: Option<&str>,
    music_source: &str,
) -> Option<Vec<u8>> {
    storage::read_editor_music_bytes(level_name, music_source)
}

pub(crate) async fn load_app_settings_from_storage() -> Result<AppSettings, String> {
    storage::load_app_settings().await
}

pub(crate) async fn save_app_settings_to_storage(settings: &AppSettings) -> Result<(), String> {
    storage::save_app_settings(settings).await
}
