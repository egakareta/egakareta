/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::sync::mpsc::Sender;

use crate::platform::io::{
    log_platform_error, pick_audio_file, pick_level_file, save_audio_to_storage, save_level_export,
};
use crate::platform::task::spawn_background;

async fn pick_audio_file_for_import() -> Option<(String, Vec<u8>)> {
    #[cfg(all(test, not(target_arch = "wasm32")))]
    if let Some(result) = test_hooks::pick_audio_result() {
        return result;
    }

    pick_audio_file().await
}

async fn save_audio_to_storage_for_import(filename: &str, bytes: &[u8]) -> Result<(), String> {
    #[cfg(all(test, not(target_arch = "wasm32")))]
    if let Some(result) = test_hooks::save_audio_result() {
        return result;
    }

    save_audio_to_storage(filename, bytes).await
}

async fn pick_level_file_for_import() -> Option<Vec<u8>> {
    #[cfg(all(test, not(target_arch = "wasm32")))]
    if let Some(result) = test_hooks::pick_level_result() {
        return result;
    }

    pick_level_file().await
}

pub fn trigger_audio_import(sender: Sender<(String, Vec<u8>)>) {
    spawn_background(async move {
        if let Some((filename, bytes)) = pick_audio_file_for_import().await {
            let _ = save_audio_to_storage_for_import(&filename, &bytes).await;
            let _ = sender.send((filename, bytes));
        }
    });
}

pub fn trigger_level_import(sender: Sender<Vec<u8>>) {
    spawn_background(async move {
        if let Some(bytes) = pick_level_file_for_import().await {
            let _ = sender.send(bytes);
        }
    });
}

pub fn trigger_level_export(filename: &str, data: &[u8]) {
    if let Err(error) = save_level_export(filename, data) {
        log_platform_error(&format!("Export failed: {}", error));
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod test_hooks {
    use std::sync::{Mutex, OnceLock};

    #[derive(Clone, Default)]
    pub(crate) struct ServiceHooks {
        pub(crate) pick_audio_result: Option<Option<(String, Vec<u8>)>>,
        pub(crate) pick_level_result: Option<Option<Vec<u8>>>,
        pub(crate) save_audio_result: Option<Result<(), String>>,
    }

    fn hooks_state() -> &'static Mutex<ServiceHooks> {
        static STATE: OnceLock<Mutex<ServiceHooks>> = OnceLock::new();
        STATE.get_or_init(|| Mutex::new(ServiceHooks::default()))
    }

    pub(crate) fn with_hooks_mut<T>(update: impl FnOnce(&mut ServiceHooks) -> T) -> T {
        let mut guard = hooks_state()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        update(&mut guard)
    }

    pub(crate) fn reset() {
        with_hooks_mut(|hooks| *hooks = ServiceHooks::default());
    }

    pub(crate) fn pick_audio_result() -> Option<Option<(String, Vec<u8>)>> {
        with_hooks_mut(|hooks| hooks.pick_audio_result.clone())
    }

    pub(crate) fn save_audio_result() -> Option<Result<(), String>> {
        with_hooks_mut(|hooks| hooks.save_audio_result.clone())
    }

    pub(crate) fn pick_level_result() -> Option<Option<Vec<u8>>> {
        with_hooks_mut(|hooks| hooks.pick_level_result.clone())
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use std::fs;
    use std::sync::mpsc;
    use std::sync::{Mutex, OnceLock};

    use web_time::Duration;

    use super::{test_hooks, trigger_audio_import, trigger_level_export, trigger_level_import};

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
    fn trigger_audio_import_forwards_selected_audio() {
        let _lock = shared_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _reset_guard = HookResetGuard::new();
        let expected_filename = "song.ogg".to_string();
        let expected_bytes = vec![0x11, 0x22, 0x33];
        test_hooks::with_hooks_mut(|hooks| {
            hooks.pick_audio_result =
                Some(Some((expected_filename.clone(), expected_bytes.clone())));
            hooks.save_audio_result = Some(Ok(()));
        });

        let (sender, receiver) = mpsc::channel();
        trigger_audio_import(sender);

        let received = receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("import should send selected audio");
        assert_eq!(received.0, expected_filename);
        assert_eq!(received.1, expected_bytes);
    }

    #[test]
    fn trigger_audio_import_sends_nothing_when_picker_returns_none() {
        let _lock = shared_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _reset_guard = HookResetGuard::new();
        test_hooks::with_hooks_mut(|hooks| {
            hooks.pick_audio_result = Some(None);
        });

        let (sender, receiver) = mpsc::channel();
        trigger_audio_import(sender);

        let received = receiver.recv_timeout(Duration::from_millis(150));
        assert!(
            received.is_err(),
            "no selection should produce no import event"
        );
    }

    #[test]
    fn trigger_level_import_forwards_selected_level_bytes() {
        let _lock = shared_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _reset_guard = HookResetGuard::new();
        let expected_bytes = vec![0xEE, 0x47, 0x5A];
        test_hooks::with_hooks_mut(|hooks| {
            hooks.pick_level_result = Some(Some(expected_bytes.clone()));
        });

        let (sender, receiver) = mpsc::channel();
        trigger_level_import(sender);

        let received = receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("import should send selected level bytes");
        assert_eq!(received, expected_bytes);
    }

    #[test]
    fn trigger_level_import_sends_nothing_when_picker_returns_none() {
        let _lock = shared_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _reset_guard = HookResetGuard::new();
        test_hooks::with_hooks_mut(|hooks| {
            hooks.pick_level_result = Some(None);
        });

        let (sender, receiver) = mpsc::channel();
        trigger_level_import(sender);

        let received = receiver.recv_timeout(Duration::from_millis(150));
        assert!(
            received.is_err(),
            "no selection should produce no import event"
        );
    }

    #[test]
    fn trigger_level_export_writes_level_bytes_to_requested_file() {
        let temp_dir = tempfile::tempdir().expect("temp dir should be created");
        let export_path = temp_dir.path().join("level.egb");
        let payload = b"serialized level bytes";

        trigger_level_export(export_path.to_string_lossy().as_ref(), payload);

        let loaded = fs::read(&export_path).expect("exported file should exist");
        assert_eq!(loaded, payload);
    }
}
