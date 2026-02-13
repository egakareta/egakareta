#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

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
    #[cfg(target_arch = "wasm32")]
    {
        use wasm_bindgen::JsValue;
        let window = web_sys::window().ok_or("No window")?;
        let idb_factory = window
            .indexed_db()
            .map_err(|_| "IndexedDB not supported")?
            .ok_or("No IndexedDB")?;

        let request = idb_factory
            .open_with_u32("line-dash-audio", 1)
            .map_err(|e| format!("{:?}", e))?;

        let filename = filename.to_string();
        let data = data.to_vec();

        let on_upgrade = Closure::once(move |event: web_sys::Event| {
            let target = event.target().unwrap();
            let db = target
                .dyn_into::<web_sys::IdbOpenDbRequest>()
                .unwrap()
                .result()
                .unwrap()
                .dyn_into::<web_sys::IdbDatabase>()
                .unwrap();
            if !db.object_store_names().contains("audio") {
                let _ = db.create_object_store("audio");
            }
        });

        let on_success = Closure::once(move |event: web_sys::Event| {
            let db = event
                .target()
                .unwrap()
                .dyn_into::<web_sys::IdbRequest>()
                .unwrap()
                .result()
                .unwrap()
                .dyn_into::<web_sys::IdbDatabase>()
                .unwrap();
            let tx = db
                .transaction_with_str_and_mode("audio", web_sys::IdbTransactionMode::Readwrite)
                .unwrap();
            let store = tx.object_store("audio").unwrap();
            let uint8_array = unsafe { js_sys::Uint8Array::view(&data) };
            let _ = store.put_with_key(&uint8_array.into(), &JsValue::from_str(&filename));
        });

        request.set_onsuccess(Some(on_success.as_ref().unchecked_ref()));
        request.set_onupgradeneeded(Some(on_upgrade.as_ref().unchecked_ref()));
        on_success.forget();
        on_upgrade.forget();

        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let mut path = std::env::current_dir().map_err(|e| e.to_string())?;
        path.push("user_audio");
        if !path.exists() {
            std::fs::create_dir_all(&path).map_err(|e| e.to_string())?;
        }
        path.push(filename);
        std::fs::write(path, data).map_err(|e| e.to_string())?;
        Ok(())
    }
}

pub(crate) async fn load_all_local_audio() -> std::collections::HashMap<String, Vec<u8>> {
    #[allow(unused_mut)]
    let mut audio_map = std::collections::HashMap::new();

    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = std::env::current_dir().ok().map(|mut p| {
            p.push("user_audio");
            p
        });
        if let Some(path) = path {
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
        }
    }

    audio_map
}

pub(crate) fn read_editor_music_bytes(
    level_name: Option<&str>,
    music_source: &str,
) -> Option<Vec<u8>> {
    #[cfg(target_arch = "wasm32")]
    {
        // On web, we check if we have it in session memory or we'd need to fetch from IndexedDB.
        // Since this is sync, we can't easily fetch from IndexedDB here.
        // We'll rely on the State keeping a cache of imported audio.
        let _ = (level_name, music_source);
        None
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        // First check builtin levels
        if let Some(name) = level_name {
            let audio_path = format!("assets/levels/{}/{}", name, music_source);
            if let Ok(bytes) = std::fs::read(audio_path) {
                return Some(bytes);
            }
        }

        // Then check user_audio
        let mut path = std::env::current_dir().ok()?;
        path.push("user_audio");
        path.push(music_source);
        std::fs::read(path).ok()
    }
}
