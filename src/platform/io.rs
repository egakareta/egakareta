pub(crate) fn save_level_export(filename: &str, data: &[u8]) -> Result<(), String> {
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsCast;

        let window = web_sys::window().ok_or_else(|| "Window is unavailable".to_string())?;
        let document = window
            .document()
            .ok_or_else(|| "Document is unavailable".to_string())?;
        let uint8_array = unsafe { js_sys::Uint8Array::view(data) };
        let blob =
            web_sys::Blob::new_with_u8_array_sequence(&js_sys::Array::of1(&uint8_array.into()))
                .map_err(|error| format!("Failed to create blob: {:?}", error))?;
        let url = web_sys::Url::create_object_url_with_blob(&blob)
            .map_err(|error| format!("Failed to create object url: {:?}", error))?;

        let result = (|| -> Result<(), String> {
            let anchor = document
                .create_element("a")
                .map_err(|error| format!("Failed to create anchor: {:?}", error))?
                .dyn_into::<web_sys::HtmlElement>()
                .map_err(|error| format!("Failed to cast anchor: {:?}", error))?;
            anchor
                .set_attribute("href", &url)
                .map_err(|error| format!("Failed setting href: {:?}", error))?;
            anchor
                .set_attribute("download", filename)
                .map_err(|error| format!("Failed setting download filename: {:?}", error))?;
            anchor.click();
            Ok(())
        })();

        let _ = web_sys::Url::revoke_object_url(&url);
        return result;
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        std::fs::write(filename, data).map_err(|error| error.to_string())
    }
}

pub(crate) fn log_platform_error(message: &str) {
    #[cfg(target_arch = "wasm32")]
    web_sys::console::log_1(&message.into());

    #[cfg(not(target_arch = "wasm32"))]
    log::error!("{}", message);
}

pub(crate) fn read_editor_music_bytes(
    level_name: Option<&str>,
    music_source: &str,
) -> Option<Vec<u8>> {
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (level_name, music_source);
        None
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        level_name.and_then(|name| {
            let audio_path = format!("assets/levels/{}/{}", name, music_source);
            std::fs::read(audio_path).ok()
        })
    }
}
