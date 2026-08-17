/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use bevy::ecs::prelude::*;
use eg_platform::services::{trigger_auth_refresh, trigger_auth_sign_in, trigger_auth_sign_out};
use eg_types::AuthSession;

use super::{AuthOpenSignupRequest, AuthSignOutRequest, AuthStateRes, AuthSubmitSignInRequest};

impl AuthStateRes {
    pub fn display_name(&self) -> Option<&str> {
        self.session
            .as_ref()
            .and_then(|session| session.profile.as_ref())
            .and_then(|profile| profile.username.as_deref())
            .or_else(|| {
                self.session
                    .as_ref()
                    .and_then(|session| session.user.email.as_deref())
            })
    }

    pub fn submit_auth_sign_in(&mut self) {
        if self.pending {
            return;
        }

        self.pending = true;
        self.message = Some("Complete sign-in in your browser.".to_string());
        trigger_auth_sign_in(self.sender.clone());
    }

    pub fn refresh_auth_session(&mut self) {
        if self.pending || self.refresh_started {
            return;
        }

        let Some(refresh_token) = self
            .session
            .as_ref()
            .map(|session| session.session.refresh_token.clone())
        else {
            return;
        };

        self.refresh_started = true;
        trigger_auth_refresh(refresh_token, self.sender.clone());
    }

    pub(crate) fn refresh_auth_session_if_needed(&mut self) {
        if self.session.is_some() && !self.refresh_started {
            self.refresh_auth_session();
        }
    }

    pub fn sign_out_auth_session(&mut self) {
        if self.pending {
            return;
        }

        let access_token = self
            .session
            .as_ref()
            .map(|session| session.session.access_token.clone());
        self.pending = true;
        self.message = None;
        trigger_auth_sign_out(access_token, self.sender.clone());
    }

    pub fn open_auth_signup_page(&mut self) {
        if !self.external_navigation_enabled {
            return;
        }

        if let Err(error) = eg_platform::auth::open_signup_page() {
            self.message = Some(error);
        }
    }

    pub(crate) fn complete_auth_sign_in(&mut self, result: Result<AuthSession, String>) {
        self.pending = false;
        match result {
            Ok(session) => {
                self.session = Some(session);
                self.refresh_started = true;
                self.message = None;
            }
            Err(error) => {
                self.message = Some(error);
            }
        }
    }

    pub(crate) fn complete_auth_refresh(&mut self, result: Result<AuthSession, String>) {
        match result {
            Ok(session) => {
                self.session = Some(session);
                self.refresh_started = true;
            }
            Err(error) => {
                self.session = None;
                self.refresh_started = false;
                self.message = Some(error);
            }
        }
    }

    pub(crate) fn complete_auth_sign_out(&mut self, result: Result<(), String>) {
        self.pending = false;
        self.refresh_started = false;
        self.session = None;
        match result {
            Ok(()) => {
                self.message = None;
            }
            Err(error) => {
                self.message = Some(error);
            }
        }
    }
}

pub fn process_auth_requests_system(
    mut auth: ResMut<AuthStateRes>,
    mut sign_in_request: ResMut<AuthSubmitSignInRequest>,
    mut sign_out_request: ResMut<AuthSignOutRequest>,
    mut open_signup_request: ResMut<AuthOpenSignupRequest>,
) {
    if sign_in_request.take() {
        auth.submit_auth_sign_in();
    }
    if sign_out_request.take() {
        auth.sign_out_auth_session();
    }
    if open_signup_request.take() {
        auth.open_auth_signup_page();
    }
    auth.refresh_auth_session_if_needed();
}

#[cfg(test)]
mod tests {
    use eg_types::{AuthProfile, AuthSessionTokens, AuthUser};

    use super::*;

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

    #[test]
    fn process_auth_requests_system_consumes_requests_in_schedule() {
        let mut world = World::new();
        world.insert_resource(AuthStateRes {
            pending: true,
            ..AuthStateRes::default()
        });
        world.insert_resource(AuthSubmitSignInRequest { pending: true });
        world.insert_resource(AuthSignOutRequest { pending: true });
        world.insert_resource(AuthOpenSignupRequest { pending: true });

        let mut schedule = bevy::ecs::schedule::Schedule::default();
        schedule.add_systems(process_auth_requests_system);
        schedule.run(&mut world);

        assert!(!world.resource::<AuthSubmitSignInRequest>().pending);
        assert!(!world.resource::<AuthSignOutRequest>().pending);
        assert!(!world.resource::<AuthOpenSignupRequest>().pending);
        assert!(
            world.resource::<AuthStateRes>().pending,
            "pending guard should keep this test side-effect free"
        );
    }

    #[test]
    fn auth_resource_display_name_prefers_profile_username_then_email() {
        let mut auth = AuthStateRes::default();
        assert_eq!(auth.display_name(), None);

        auth.session = Some(test_auth_session(
            Some("player"),
            Some("player@example.com"),
        ));
        assert_eq!(auth.display_name(), Some("player"));

        auth.session = Some(test_auth_session(None, Some("player@example.com")));
        assert_eq!(auth.display_name(), Some("player@example.com"));

        auth.session = Some(test_auth_session(None, None));
        assert_eq!(auth.display_name(), None);
    }
}
