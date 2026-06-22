/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Per-function test hook overrides shared across the platform crate.
//!
//! Each function in [`crate::io`] and [`crate::services`] checks the
//! corresponding entry below in test builds. This lets unit tests stub out
//! platform-specific side effects (file pickers, clipboard, storage, auth) on a
//! per-function basis without going through wrapper layers.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use eg_types::{AppSettings, AuthSession, PersistentWaveformCacheEntry};

#[derive(Clone, Default)]
pub struct PlatformHooks {
    pub pick_audio_result: Option<Option<(String, Vec<u8>)>>,
    pub pick_level_result: Option<Option<Vec<u8>>>,
    pub save_audio_result: Option<Result<(), String>>,
    pub load_audio_result: Option<HashMap<String, Vec<u8>>>,
    pub read_editor_music_result: Option<Option<Vec<u8>>>,
    pub save_waveform_result: Option<Result<(), String>>,
    pub load_waveform_result: Option<Option<PersistentWaveformCacheEntry>>,
    pub load_settings_result: Option<Result<AppSettings, String>>,
    pub save_settings_result: Option<Result<(), String>>,
    pub auth_sign_in_result: Option<Result<AuthSession, String>>,
    pub auth_refresh_result: Option<Result<AuthSession, String>>,
    pub auth_sign_out_result: Option<Result<(), String>>,
}

fn hooks_state() -> &'static Mutex<PlatformHooks> {
    static STATE: OnceLock<Mutex<PlatformHooks>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(PlatformHooks::default()))
}

pub fn with_hooks_mut<T>(update: impl FnOnce(&mut PlatformHooks) -> T) -> T {
    let mut guard = hooks_state()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    update(&mut guard)
}

pub fn reset() {
    with_hooks_mut(|hooks| *hooks = PlatformHooks::default());
}

pub fn pick_audio_result() -> Option<Option<(String, Vec<u8>)>> {
    with_hooks_mut(|hooks| hooks.pick_audio_result.clone())
}

pub fn pick_level_result() -> Option<Option<Vec<u8>>> {
    with_hooks_mut(|hooks| hooks.pick_level_result.clone())
}

pub fn save_audio_result() -> Option<Result<(), String>> {
    with_hooks_mut(|hooks| hooks.save_audio_result.clone())
}

pub fn load_audio_result() -> Option<HashMap<String, Vec<u8>>> {
    with_hooks_mut(|hooks| hooks.load_audio_result.clone())
}

pub fn read_editor_music_result() -> Option<Option<Vec<u8>>> {
    with_hooks_mut(|hooks| hooks.read_editor_music_result.clone())
}

pub fn save_waveform_result() -> Option<Result<(), String>> {
    with_hooks_mut(|hooks| hooks.save_waveform_result.clone())
}

pub fn load_waveform_result() -> Option<Option<PersistentWaveformCacheEntry>> {
    with_hooks_mut(|hooks| hooks.load_waveform_result.clone())
}

pub fn load_settings_result() -> Option<Result<AppSettings, String>> {
    with_hooks_mut(|hooks| hooks.load_settings_result.clone())
}

pub fn save_settings_result() -> Option<Result<(), String>> {
    with_hooks_mut(|hooks| hooks.save_settings_result.clone())
}

pub fn auth_sign_in_result() -> Option<Result<AuthSession, String>> {
    with_hooks_mut(|hooks| hooks.auth_sign_in_result.clone())
}

pub fn auth_refresh_result() -> Option<Result<AuthSession, String>> {
    with_hooks_mut(|hooks| hooks.auth_refresh_result.clone())
}

pub fn auth_sign_out_result() -> Option<Result<(), String>> {
    with_hooks_mut(|hooks| hooks.auth_sign_out_result.clone())
}
