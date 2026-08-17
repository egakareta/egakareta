/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use eg_types::AuthSession;

use crate::io::{pick_audio_file, pick_level_file, save_audio_to_storage};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthServiceMessage {
    SignedIn(Result<AuthSession, String>),
    Refreshed(Result<AuthSession, String>),
    SignedOut(Result<(), String>),
}

/// Pick and persist a user audio file, returning the owned import payload.
///
/// The caller owns this future through a Bevy `Task`; no service-side channel
/// or detached producer is required for completion delivery.
pub async fn import_audio() -> Option<(String, Vec<u8>)> {
    let (filename, bytes) = pick_audio_file().await?;
    let _ = save_audio_to_storage(&filename, &bytes).await;
    Some((filename, bytes))
}

/// Pick a level archive or binary payload and return its owned bytes.
///
/// Like [`import_audio`], this is a result-returning future owned by the
/// App-world task component that requested it.
pub async fn import_level() -> Option<Vec<u8>> {
    pick_level_file().await
}

/// Complete the browser sign-in flow and return its typed outcome.
pub async fn sign_in() -> AuthServiceMessage {
    #[cfg(all(test, not(target_arch = "wasm32")))]
    if let Some(result) = crate::test_hooks::auth_sign_in_result() {
        return AuthServiceMessage::SignedIn(result);
    }

    AuthServiceMessage::SignedIn(persist_auth_session_result(crate::auth::sign_in().await).await)
}

/// Refresh an auth session and return its typed outcome.
pub async fn refresh(refresh_token: String) -> AuthServiceMessage {
    #[cfg(all(test, not(target_arch = "wasm32")))]
    if let Some(result) = crate::test_hooks::auth_refresh_result() {
        return AuthServiceMessage::Refreshed(result);
    }

    AuthServiceMessage::Refreshed(
        persist_auth_session_result(crate::auth::refresh_session(&refresh_token).await).await,
    )
}

async fn persist_auth_session_result(
    result: Result<AuthSession, String>,
) -> Result<AuthSession, String> {
    let session = result?;
    crate::storage::save_auth_session(&session)
        .await
        .map_err(|error| format!("Session could not be saved: {error}"))?;
    Ok(session)
}

/// Sign out and clear the persisted session, returning the typed outcome.
pub async fn sign_out(access_token: Option<String>) -> AuthServiceMessage {
    #[cfg(all(test, not(target_arch = "wasm32")))]
    if let Some(result) = crate::test_hooks::auth_sign_out_result() {
        return AuthServiceMessage::SignedOut(result);
    }

    let result = match access_token.as_deref().filter(|token| !token.is_empty()) {
        Some(token) => crate::auth::sign_out(Some(token)).await,
        None => Ok(()),
    };
    let _ = crate::storage::clear_auth_session().await;
    AuthServiceMessage::SignedOut(result)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use eg_types::{AuthSession, AuthSessionTokens, AuthUser};

    use super::{AuthServiceMessage, import_audio, import_level, refresh, sign_in, sign_out};
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
    fn import_audio_returns_selected_audio() {
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

        let received = pollster::block_on(import_audio()).expect("import should return audio");
        assert_eq!(received.0, expected_filename);
        assert_eq!(received.1, expected_bytes);
    }

    #[test]
    fn import_audio_returns_none_when_picker_returns_none() {
        let _lock = shared_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _reset_guard = HookResetGuard::new();
        test_hooks::with_hooks_mut(|hooks| {
            hooks.pick_audio_result = Some(None);
        });

        assert_eq!(pollster::block_on(import_audio()), None);
    }

    #[test]
    fn import_level_returns_selected_level_bytes() {
        let _lock = shared_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _reset_guard = HookResetGuard::new();
        let expected_bytes = vec![0xEE, 0x47, 0x5A];
        test_hooks::with_hooks_mut(|hooks| {
            hooks.pick_level_result = Some(Some(expected_bytes.clone()));
        });

        let received =
            pollster::block_on(import_level()).expect("import should return level bytes");
        assert_eq!(received, expected_bytes);
    }

    #[test]
    fn import_level_returns_none_when_picker_returns_none() {
        let _lock = shared_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _reset_guard = HookResetGuard::new();
        test_hooks::with_hooks_mut(|hooks| {
            hooks.pick_level_result = Some(None);
        });

        assert_eq!(pollster::block_on(import_level()), None);
    }

    #[test]
    fn sign_in_returns_hooked_session() {
        let _lock = shared_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _reset_guard = HookResetGuard::new();
        let expected = test_auth_session("access", "refresh");
        test_hooks::with_hooks_mut(|hooks| {
            hooks.auth_sign_in_result = Some(Ok(expected.clone()));
        });

        let received = pollster::block_on(sign_in());
        assert_eq!(received, AuthServiceMessage::SignedIn(Ok(expected)));
    }

    #[test]
    fn refresh_returns_hooked_error() {
        let _lock = shared_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _reset_guard = HookResetGuard::new();
        test_hooks::with_hooks_mut(|hooks| {
            hooks.auth_refresh_result = Some(Err("refresh failed".to_string()));
        });

        let received = pollster::block_on(refresh("refresh-token".to_string()));
        assert_eq!(
            received,
            AuthServiceMessage::Refreshed(Err("refresh failed".to_string()))
        );
    }

    #[test]
    fn sign_out_returns_hooked_result() {
        let _lock = shared_test_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _reset_guard = HookResetGuard::new();
        test_hooks::with_hooks_mut(|hooks| {
            hooks.auth_sign_out_result = Some(Ok(()));
        });

        let received = pollster::block_on(sign_out(Some("access-token".to_string())));
        assert_eq!(received, AuthServiceMessage::SignedOut(Ok(())));
    }
}
