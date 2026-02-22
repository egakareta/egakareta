use crate::platform::storage;

pub(crate) fn save_level_export(filename: &str, data: &[u8]) -> Result<(), String> {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;

        let document = gloo_utils::document();
        let blob = gloo_file::Blob::new(data);
        let url = gloo_file::ObjectUrl::from(blob);

        let anchor = document
            .create_element("a")
            .map_err(|error| format!("Failed to create anchor: {:?}", error))?
            .dyn_into::<web_sys::HtmlElement>()
            .map_err(|error| format!("Failed to cast anchor: {:?}", error))?;
        anchor
            .set_attribute("href", &url.to_string())
            .map_err(|error| format!("Failed setting href: {:?}", error))?;
        anchor
            .set_attribute("download", filename)
            .map_err(|error| format!("Failed setting download filename: {:?}", error))?;
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
