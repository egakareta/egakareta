/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
use crate::platform::io::{
    log_platform_error, pick_audio_file, save_audio_to_storage, save_level_export,
};
use crate::platform::task::spawn_background;
use std::sync::mpsc::Sender;

pub fn trigger_audio_import(sender: Sender<(String, Vec<u8>)>) {
    spawn_background(async move {
        if let Some((filename, bytes)) = pick_audio_file().await {
            let _ = save_audio_to_storage(&filename, &bytes).await;
            let _ = sender.send((filename, bytes));
        }
    });
}

pub fn trigger_level_export(filename: &str, data: &[u8]) {
    if let Err(error) = save_level_export(filename, data) {
        log_platform_error(&format!("Export failed: {}", error));
    }
}
