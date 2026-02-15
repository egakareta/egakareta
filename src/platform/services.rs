use crate::platform::io::{
    log_platform_error, pick_audio_file, save_audio_to_storage, save_level_export,
};
use std::sync::mpsc::Sender;

pub fn trigger_audio_import(sender: Sender<(String, Vec<u8>)>) {
    #[cfg(target_arch = "wasm32")]
    {
        wasm_bindgen_futures::spawn_local(async move {
            if let Some((filename, bytes)) = pick_audio_file().await {
                let _ = save_audio_to_storage(&filename, &bytes).await;
                let _ = sender.send((filename, bytes));
            }
        });
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::thread::spawn(move || {
            pollster::block_on(async {
                if let Some((filename, bytes)) = pick_audio_file().await {
                    let _ = save_audio_to_storage(&filename, &bytes).await;
                    let _ = sender.send((filename, bytes));
                }
            });
        });
    }
}

pub fn trigger_level_export(filename: &str, data: &[u8]) {
    if let Err(error) = save_level_export(filename, data) {
        log_platform_error(&format!("Export failed: {}", error));
    }
}
