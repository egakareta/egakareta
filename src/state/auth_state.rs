/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use super::State;
use crate::platform::services::{
    trigger_auth_refresh, trigger_auth_sign_in, trigger_auth_sign_out, AuthServiceMessage,
};
use crate::types::AuthSession;

impl State {
    pub(crate) fn auth_display_name(&self) -> Option<&str> {
        self.auth
            .session
            .as_ref()
            .and_then(|session| session.profile.as_ref())
            .and_then(|profile| profile.username.as_deref())
            .or_else(|| {
                self.auth
                    .session
                    .as_ref()
                    .and_then(|session| session.user.email.as_deref())
            })
    }

    pub(crate) fn auth_pending(&self) -> bool {
        self.auth.pending
    }

    pub(crate) fn auth_message(&self) -> Option<&str> {
        self.auth.message.as_deref()
    }

    pub(crate) fn submit_auth_sign_in(&mut self) {
        if self.auth.pending {
            return;
        }

        self.auth.pending = true;
        self.auth.message = Some("Complete sign-in in your browser.".to_string());
        trigger_auth_sign_in(self.auth.channel.0.clone());
    }

    pub(crate) fn refresh_auth_session(&mut self) {
        if self.auth.pending || self.auth.refresh_started {
            return;
        }

        let Some(refresh_token) = self
            .auth
            .session
            .as_ref()
            .map(|session| session.session.refresh_token.clone())
        else {
            return;
        };

        self.auth.refresh_started = true;
        trigger_auth_refresh(refresh_token, self.auth.channel.0.clone());
    }

    pub(crate) fn sign_out_auth_session(&mut self) {
        if self.auth.pending {
            return;
        }

        let access_token = self
            .auth
            .session
            .as_ref()
            .map(|session| session.session.access_token.clone());
        self.auth.pending = true;
        self.auth.message = None;
        trigger_auth_sign_out(access_token, self.auth.channel.0.clone());
    }

    pub(crate) fn open_auth_signup_page(&mut self) {
        crate::platform::auth::open_signup_page();
    }

    pub(crate) fn update_auth_results(&mut self) {
        if self.auth.session.is_some() && !self.auth.refresh_started {
            self.refresh_auth_session();
        }

        while let Ok(message) = self.auth.channel.1.try_recv() {
            match message {
                AuthServiceMessage::SignedIn(result) => self.complete_auth_sign_in(result),
                AuthServiceMessage::Refreshed(result) => self.complete_auth_refresh(result),
                AuthServiceMessage::SignedOut(result) => self.complete_auth_sign_out(result),
            }
        }
    }

    fn complete_auth_sign_in(&mut self, result: Result<AuthSession, String>) {
        self.auth.pending = false;
        match result {
            Ok(session) => {
                self.auth.session = Some(session);
                self.auth.refresh_started = true;
                self.auth.message = None;
            }
            Err(error) => {
                self.auth.message = Some(error);
            }
        }
    }

    fn complete_auth_refresh(&mut self, result: Result<AuthSession, String>) {
        match result {
            Ok(session) => {
                self.auth.session = Some(session);
                self.auth.refresh_started = true;
            }
            Err(error) => {
                self.auth.session = None;
                self.auth.refresh_started = false;
                self.auth.message = Some(error);
            }
        }
    }

    fn complete_auth_sign_out(&mut self, result: Result<(), String>) {
        self.auth.pending = false;
        self.auth.refresh_started = false;
        self.auth.session = None;
        match result {
            Ok(()) => {
                self.auth.message = None;
            }
            Err(error) => {
                self.auth.message = Some(error);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{AuthProfile, AuthSessionTokens, AuthUser};

    fn test_auth_session(username: Option<&str>, email: Option<&str>) -> AuthSession {
        AuthSession {
            session: AuthSessionTokens {
                access_token: "access-token".to_string(),
                refresh_token: "refresh-token".to_string(),
                expires_at: Some(123),
                token_type: "bearer".to_string(),
            },
            user: AuthUser {
                id: "user-id".to_string(),
                email: email.map(str::to_string),
            },
            profile: username.map(|name| AuthProfile {
                id: "user-id".to_string(),
                username: Some(name.to_string()),
                avatar_url: None,
                country: "UN".to_string(),
            }),
        }
    }

    fn new_state() -> State {
        pollster::block_on(State::new_test())
    }

    #[test]
    fn auth_display_name_prefers_profile_username_then_email() {
        let mut state = new_state();
        assert_eq!(state.auth_display_name(), None);

        state.auth.session = Some(test_auth_session(
            Some("player"),
            Some("player@example.com"),
        ));
        assert_eq!(state.auth_display_name(), Some("player"));

        state.auth.session = Some(test_auth_session(None, Some("player@example.com")));
        assert_eq!(state.auth_display_name(), Some("player@example.com"));

        state.auth.session = Some(test_auth_session(None, None));
        assert_eq!(state.auth_display_name(), None);
    }

    #[test]
    fn auth_actions_ignore_pending_or_missing_session_guards() {
        let mut state = new_state();

        state.auth.pending = true;
        state.submit_auth_sign_in();
        assert!(state.auth.pending);
        assert_eq!(state.auth.message, None);

        state.auth.pending = false;
        state.refresh_auth_session();
        assert!(!state.auth.refresh_started);

        state.auth.session = Some(test_auth_session(
            Some("player"),
            Some("player@example.com"),
        ));
        state.auth.pending = true;
        state.refresh_auth_session();
        assert!(!state.auth.refresh_started);

        state.auth.pending = false;
        state.auth.refresh_started = true;
        state.refresh_auth_session();
        assert!(state.auth.refresh_started);

        state.auth.pending = true;
        state.auth.message = Some("keep me".to_string());
        state.sign_out_auth_session();
        assert_eq!(state.auth.message.as_deref(), Some("keep me"));
    }

    #[test]
    fn update_auth_results_applies_sign_in_success_and_failure() {
        let mut state = new_state();
        let session = test_auth_session(Some("player"), Some("player@example.com"));

        state.auth.pending = true;
        state
            .auth
            .channel
            .0
            .send(AuthServiceMessage::SignedIn(Ok(session.clone())))
            .expect("message should send");
        state.update_auth_results();

        assert!(!state.auth.pending);
        assert_eq!(state.auth.session, Some(session));
        assert!(state.auth.refresh_started);
        assert_eq!(state.auth.message, None);

        state.auth.pending = true;
        state
            .auth
            .channel
            .0
            .send(AuthServiceMessage::SignedIn(Err(
                "sign-in failed".to_string()
            )))
            .expect("message should send");
        state.update_auth_results();

        assert!(!state.auth.pending);
        assert_eq!(state.auth.message.as_deref(), Some("sign-in failed"));
    }

    #[test]
    fn update_auth_results_applies_refresh_success_and_failure() {
        let mut state = new_state();
        let original = test_auth_session(Some("old"), Some("old@example.com"));
        let refreshed = test_auth_session(Some("new"), Some("new@example.com"));

        state.auth.session = Some(original);
        state.auth.refresh_started = true;
        state
            .auth
            .channel
            .0
            .send(AuthServiceMessage::Refreshed(Ok(refreshed.clone())))
            .expect("message should send");
        state.update_auth_results();

        assert_eq!(state.auth.session, Some(refreshed));
        assert!(state.auth.refresh_started);
        assert_eq!(state.auth.message, None);

        state
            .auth
            .channel
            .0
            .send(AuthServiceMessage::Refreshed(Err(
                "refresh failed".to_string()
            )))
            .expect("message should send");
        state.update_auth_results();

        assert_eq!(state.auth.session, None);
        assert!(!state.auth.refresh_started);
        assert_eq!(state.auth.message.as_deref(), Some("refresh failed"));
    }

    #[test]
    fn update_auth_results_applies_sign_out_success_and_failure() {
        let mut state = new_state();

        state.auth.session = Some(test_auth_session(
            Some("player"),
            Some("player@example.com"),
        ));
        state.auth.pending = true;
        state.auth.refresh_started = true;
        state
            .auth
            .channel
            .0
            .send(AuthServiceMessage::SignedOut(Ok(())))
            .expect("message should send");
        state.update_auth_results();

        assert!(!state.auth.pending);
        assert!(!state.auth.refresh_started);
        assert_eq!(state.auth.session, None);
        assert_eq!(state.auth.message, None);

        state.auth.session = Some(test_auth_session(None, Some("player@example.com")));
        state.auth.pending = true;
        state.auth.refresh_started = true;
        state
            .auth
            .channel
            .0
            .send(AuthServiceMessage::SignedOut(Err(
                "sign-out failed".to_string()
            )))
            .expect("message should send");
        state.update_auth_results();

        assert!(!state.auth.pending);
        assert!(!state.auth.refresh_started);
        assert_eq!(state.auth.session, None);
        assert_eq!(state.auth.message.as_deref(), Some("sign-out failed"));
    }
}
