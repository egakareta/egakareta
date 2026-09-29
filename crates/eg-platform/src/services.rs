/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::sync::mpsc::Sender;

use eg_types::AuthSession;

use crate::{
    io::{pick_audio_file, pick_level_file, save_audio_to_storage},
    task::spawn_background,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthServiceMessage {
    SignedIn(Result<AuthSession, String>),
    Refreshed(Result<AuthSession, String>),
    SignedOut(Result<(), String>),
}

pub fn trigger_audio_import(sender: Sender<(String, Vec<u8>)>) {
    spawn_background(async move {
        if let Some((filename, bytes)) = pick_audio_file().await {
            let _ = save_audio_to_storage(&filename, &bytes);
            let _ = sender.send((filename, bytes));
        }
    });
}

pub fn trigger_level_import(sender: Sender<Vec<u8>>) {
    spawn_background(async move {
        if let Some(bytes) = pick_level_file().await {
            let _ = sender.send(bytes);
        }
    });
}

pub fn trigger_auth_sign_in(sender: Sender<AuthServiceMessage>) {
    spawn_background(async move {
        #[cfg(all(test, not(target_arch = "wasm32")))]
        if let Some(result) = crate::test_hooks::auth_sign_in_result() {
            let _ = sender.send(AuthServiceMessage::SignedIn(result));
            return;
        }

        let result = persist_auth_session_result(crate::auth::sign_in().await);
        let _ = sender.send(AuthServiceMessage::SignedIn(result));
    });
}

pub fn trigger_auth_refresh(refresh_token: String, sender: Sender<AuthServiceMessage>) {
    spawn_background(async move {
        #[cfg(all(test, not(target_arch = "wasm32")))]
        if let Some(result) = crate::test_hooks::auth_refresh_result() {
            let _ = sender.send(AuthServiceMessage::Refreshed(result));
            return;
        }

        let result =
            persist_auth_session_result(crate::auth::refresh_session(&refresh_token).await);
        let _ = sender.send(AuthServiceMessage::Refreshed(result));
    });
}

fn persist_auth_session_result(result: Result<AuthSession, String>) -> Result<AuthSession, String> {
    let session = result?;
    crate::storage::save_auth_session(&session)
        .map_err(|error| format!("Session could not be saved: {error}"))?;
    Ok(session)
}

pub fn trigger_auth_sign_out(access_token: Option<String>, sender: Sender<AuthServiceMessage>) {
    spawn_background(async move {
        #[cfg(all(test, not(target_arch = "wasm32")))]
        if let Some(result) = crate::test_hooks::auth_sign_out_result() {
            let _ = sender.send(AuthServiceMessage::SignedOut(result));
            return;
        }

        let result = crate::auth::sign_out(access_token.as_deref()).await;
        let _ = crate::storage::clear_auth_session();
        let _ = sender.send(AuthServiceMessage::SignedOut(result));
    });
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use std::sync::mpsc;

    use eg_types::{AuthSession, AuthSessionTokens, AuthUser};
    use web_time::Duration;

    use super::{
        AuthServiceMessage, trigger_audio_import, trigger_auth_refresh, trigger_auth_sign_in,
        trigger_auth_sign_out, trigger_level_import,
    };
    use crate::{test_hooks, test_hooks::shared_test_lock};

    fn test_auth_session(access_token: &str, refresh_token: &str) -> AuthSession {
        AuthSession {
            session: AuthSessionTokens {
                access_token: access_token.to_string(),
                refresh_token: refresh_token.to_string(),
                expires_at: Some(123),
                token_type: "bearer".to_string(),
            },
            user: AuthUser {
                id: "user-id".to_string(),
                email: Some("player@example.com".to_string()),
            },
            profile: None,
        }
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
        let _lock = pollster::block_on(shared_test_lock().lock());
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
        let _lock = pollster::block_on(shared_test_lock().lock());
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
        let _lock = pollster::block_on(shared_test_lock().lock());
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
        let _lock = pollster::block_on(shared_test_lock().lock());
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
    fn trigger_auth_sign_in_forwards_hooked_session() {
        let _lock = pollster::block_on(shared_test_lock().lock());
        let _reset_guard = HookResetGuard::new();
        let expected = test_auth_session("access", "refresh");
        test_hooks::with_hooks_mut(|hooks| {
            hooks.auth_sign_in_result = Some(Ok(expected.clone()));
        });

        let (sender, receiver) = mpsc::channel();
        trigger_auth_sign_in(sender);

        let received = receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("sign-in result should be sent");
        assert_eq!(received, AuthServiceMessage::SignedIn(Ok(expected)));
    }

    #[test]
    fn trigger_auth_refresh_forwards_hooked_error() {
        let _lock = pollster::block_on(shared_test_lock().lock());
        let _reset_guard = HookResetGuard::new();
        test_hooks::with_hooks_mut(|hooks| {
            hooks.auth_refresh_result = Some(Err("refresh failed".to_string()));
        });

        let (sender, receiver) = mpsc::channel();
        trigger_auth_refresh("refresh-token".to_string(), sender);

        let received = receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("refresh result should be sent");
        assert_eq!(
            received,
            AuthServiceMessage::Refreshed(Err("refresh failed".to_string()))
        );
    }

    #[test]
    fn trigger_auth_sign_out_forwards_hooked_result() {
        let _lock = pollster::block_on(shared_test_lock().lock());
        let _reset_guard = HookResetGuard::new();
        test_hooks::with_hooks_mut(|hooks| {
            hooks.auth_sign_out_result = Some(Ok(()));
        });

        let (sender, receiver) = mpsc::channel();
        trigger_auth_sign_out(Some("access-token".to_string()), sender);

        let received = receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("sign-out result should be sent");
        assert_eq!(received, AuthServiceMessage::SignedOut(Ok(())));
    }
}
