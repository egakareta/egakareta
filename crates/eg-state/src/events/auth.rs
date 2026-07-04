/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Auth + clipboard + surface-resize events. One per `AppCommand` variant
//! under `AppCommand::Auth` in the legacy enum.

use bevy_app::App;
use bevy_ecs::prelude::*;
use eg_types::PhysicalSize;

use crate::state::{
    AuthOpenSignupRequest, AuthSignOutRequest, AuthSubmitSignInRequest, PerfFrameCopyRequest,
    RenderResizeRequest,
};

/// Start the browser sign-in flow.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct AuthSubmitSignIn;

/// Sign out of the current account.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct AuthSignOut;

/// Open the web signup page.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct AuthOpenSignup;

/// Copy the latest profiler frame to the system clipboard.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct CopyLatestPerfFrame;

/// Resize render surface to the specified dimensions.
#[derive(Message, Debug, Clone, Copy)]
pub struct ResizeSurface {
    pub width: u32,
    pub height: u32,
}

pub fn register_events(app: &mut App) {
    app.add_message::<AuthSubmitSignIn>()
        .add_message::<AuthSignOut>()
        .add_message::<AuthOpenSignup>()
        .add_message::<CopyLatestPerfFrame>()
        .add_message::<ResizeSurface>();
}

// -----------------------------------------------------------------------
// Dispatch systems
// -----------------------------------------------------------------------

// `AuthSubmitSignIn` writes a typed request resource (same pattern as
// `events::misc::handle_escape` and
// `events::auth::handle_resize_surface`): the handler writes
// `AuthSubmitSignInRequest.pending = true` and
// `Runtime::sync_app_auth_submit_sign_in_request` consumes the
// request during the typed request sync pass and calls
// the existing legacy `State::submit_auth_sign_in` compatibility
// method. The sign-in side effect (network IO via
// `trigger_auth_sign_in` on the auth channel) stays legacy-owned
// because the auth channel `Sender` lives on the legacy `State`.
pub fn handle_auth_submit_sign_in(
    mut events: MessageReader<AuthSubmitSignIn>,
    mut sign_in_request: ResMut<AuthSubmitSignInRequest>,
) {
    for _ in events.read() {
        sign_in_request.request();
    }
}

// `AuthSignOut` writes a typed request resource (same pattern as
// `AuthSubmitSignIn`): the handler writes
// `AuthSignOutRequest.pending = true` and
// `Runtime::sync_app_auth_sign_out_request` consumes the request
// and calls the existing legacy `State::sign_out_auth_session`
// compatibility method. The sign-out side effect (network IO via
// `trigger_auth_sign_out` on the auth channel) stays legacy-owned
// because the auth channel `Sender` lives on the legacy `State`.
pub fn handle_auth_sign_out(
    mut events: MessageReader<AuthSignOut>,
    mut sign_out_request: ResMut<AuthSignOutRequest>,
) {
    for _ in events.read() {
        sign_out_request.request();
    }
}

// `AuthOpenSignup` writes a typed request resource (same pattern as
// `AuthSubmitSignIn`): the handler writes
// `AuthOpenSignupRequest.pending = true` and
// `Runtime::sync_app_auth_open_signup_request` consumes the
// request and calls the existing legacy
// `State::open_auth_signup_page` compatibility method. The
// signup-page-open side effect (platform code via
// `eg_platform::auth::open_signup_page`) stays legacy-owned
// because the platform API is only available in non-wasm targets.
pub fn handle_auth_open_signup(
    mut events: MessageReader<AuthOpenSignup>,
    mut open_signup_request: ResMut<AuthOpenSignupRequest>,
) {
    for _ in events.read() {
        open_signup_request.request();
    }
}

pub fn handle_copy_latest_perf_frame(
    mut events: MessageReader<CopyLatestPerfFrame>,
    mut copy_request: ResMut<PerfFrameCopyRequest>,
) {
    for _ in events.read() {
        copy_request.request();
    }
}

pub fn handle_resize_surface(
    mut events: MessageReader<ResizeSurface>,
    mut resize: ResMut<RenderResizeRequest>,
) {
    for event in events.read() {
        resize.request(PhysicalSize::new(event.width, event.height));
    }
}
