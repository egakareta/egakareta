/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use bevy::{
    ecs::prelude::*,
    tasks::{IoTaskPool, Task, TaskPool, futures::check_ready},
};
use eg_platform::services::AuthServiceMessage;
use eg_types::AuthSession;

use super::{AuthOpenSignupRequest, AuthSignOutRequest, AuthStateRes, AuthSubmitSignInRequest};
use crate::events::async_results::AuthServiceResultEvent;

/// One in-flight authentication operation owned by the App world.
#[derive(Component)]
pub(crate) struct AuthTask(Task<AuthServiceMessage>);

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

    fn begin_auth_sign_in(&mut self) -> bool {
        if self.pending {
            return false;
        }

        self.pending = true;
        self.message = Some("Complete sign-in in your browser.".to_string());
        true
    }

    fn begin_auth_refresh(&mut self) -> Option<String> {
        if self.pending || self.refresh_started {
            return None;
        }

        let refresh_token = self
            .session
            .as_ref()
            .map(|session| session.session.refresh_token.clone())?;

        self.refresh_started = true;
        Some(refresh_token)
    }

    fn begin_auth_refresh_if_needed(&mut self) -> Option<String> {
        if self.session.is_some() && !self.refresh_started {
            self.begin_auth_refresh()
        } else {
            None
        }
    }

    fn begin_auth_sign_out(&mut self) -> Option<Option<String>> {
        if self.pending {
            return None;
        }

        let access_token = self
            .session
            .as_ref()
            .map(|session| session.session.access_token.clone());
        self.pending = true;
        self.message = None;
        Some(access_token)
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
    mut commands: Commands,
    mut auth: ResMut<AuthStateRes>,
    mut sign_in_request: ResMut<AuthSubmitSignInRequest>,
    mut sign_out_request: ResMut<AuthSignOutRequest>,
    mut open_signup_request: ResMut<AuthOpenSignupRequest>,
) {
    if sign_in_request.take() && auth.begin_auth_sign_in() {
        spawn_auth_sign_in_task(&mut commands);
    }
    if sign_out_request.take()
        && let Some(access_token) = auth.begin_auth_sign_out()
    {
        spawn_auth_sign_out_task(&mut commands, access_token);
    }
    if open_signup_request.take() {
        auth.open_auth_signup_page();
    }
    if let Some(refresh_token) = auth.begin_auth_refresh_if_needed() {
        spawn_auth_refresh_task(&mut commands, refresh_token);
    }
}

fn spawn_auth_sign_in_task(commands: &mut Commands) {
    let task = IoTaskPool::get_or_init(TaskPool::new).spawn(eg_platform::services::sign_in());
    commands.spawn(AuthTask(task));
}

fn spawn_auth_refresh_task(commands: &mut Commands, refresh_token: String) {
    let task =
        IoTaskPool::get_or_init(TaskPool::new).spawn(eg_platform::services::refresh(refresh_token));
    commands.spawn(AuthTask(task));
}

fn spawn_auth_sign_out_task(commands: &mut Commands, access_token: Option<String>) {
    let task =
        IoTaskPool::get_or_init(TaskPool::new).spawn(eg_platform::services::sign_out(access_token));
    commands.spawn(AuthTask(task));
}

/// Write each completed task result into the established typed auth handler.
pub fn poll_auth_tasks_system(
    mut commands: Commands,
    mut tasks: Query<(Entity, &mut AuthTask)>,
    mut results: MessageWriter<AuthServiceResultEvent>,
) {
    for (entity, mut task) in &mut tasks {
        let Some(result) = check_ready(&mut task.0) else {
            continue;
        };
        results.write(AuthServiceResultEvent::new(result));
        commands.entity(entity).despawn();
    }
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
