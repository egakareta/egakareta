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

    pub(crate) fn auth_dialog_open(&self) -> bool {
        self.auth.dialog_open
    }

    pub(crate) fn auth_identifier(&self) -> &str {
        &self.auth.identifier
    }

    pub(crate) fn auth_password(&self) -> &str {
        &self.auth.password
    }

    pub(crate) fn auth_pending(&self) -> bool {
        self.auth.pending
    }

    pub(crate) fn auth_message(&self) -> Option<&str> {
        self.auth.message.as_deref()
    }

    pub(crate) fn set_auth_dialog_open(&mut self, open: bool) {
        self.auth.dialog_open = open;
        if open {
            self.auth.message = None;
        } else if !self.auth.pending {
            self.auth.password.clear();
        }
    }

    pub(crate) fn set_auth_identifier(&mut self, identifier: String) {
        self.auth.identifier = identifier;
    }

    pub(crate) fn set_auth_password(&mut self, password: String) {
        self.auth.password = password;
    }

    pub(crate) fn submit_auth_sign_in(&mut self) {
        let identifier = self.auth.identifier.trim().to_string();
        let password = self.auth.password.clone();

        if self.auth.pending {
            return;
        }
        if identifier.is_empty() || password.is_empty() {
            self.auth.message = Some("Enter your username or email and password.".to_string());
            return;
        }

        self.auth.pending = true;
        self.auth.message = None;
        trigger_auth_sign_in(identifier, password, self.auth.channel.0.clone());
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
                self.auth.dialog_open = false;
                self.auth.password.clear();
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
        self.auth.password.clear();
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
