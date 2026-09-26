/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Auth + clipboard + surface-resize events. One message per auth,
//! clipboard, and surface-resize intent (`AuthSubmitSignIn`, `AuthSignOut`,
//! `AuthOpenSignup`, `CopyLatestPerfFrame`, `ResizeSurface`).

use bevy_ecs::prelude::*;
use eg_types::PhysicalSize;

use crate::{
    events::MessageReader,
    state::{
        AuthOpenSignupRequest, AuthSignOutRequest, AuthSubmitSignInRequest, PerfFrameCopyRequest,
        RenderResizeRequest,
    },
};

/// Start the browser sign-in flow.
#[derive(Debug, Clone, Copy, Default)]
pub struct AuthSubmitSignIn;

/// Sign out of the current account.
#[derive(Debug, Clone, Copy, Default)]
pub struct AuthSignOut;

/// Open the web signup page.
#[derive(Debug, Clone, Copy, Default)]
pub struct AuthOpenSignup;

/// Copy the latest profiler frame to the system clipboard.
#[derive(Debug, Clone, Copy, Default)]
pub struct CopyLatestPerfFrame;

/// Resize render surface to the specified dimensions.
#[derive(Debug, Clone, Copy)]
pub struct ResizeSurface {
    pub width: u32,
    pub height: u32,
}

// -----------------------------------------------------------------------
// Dispatch systems
// -----------------------------------------------------------------------

// `AuthSubmitSignIn` writes a typed request resource. The
// `state::auth_state::process_auth_requests_system` consumes the request in
// `StateSchedule` and mutates `AuthStateRes`.
pub fn handle_auth_submit_sign_in(
    mut events: MessageReader<AuthSubmitSignIn>,
    mut sign_in_request: ResMut<AuthSubmitSignInRequest>,
) {
    for _ in events.read() {
        sign_in_request.request();
    }
}

// `AuthSignOut` writes a typed request resource. The request is consumed by
// `state::auth_state::process_auth_requests_system`, not by Runtime.
pub fn handle_auth_sign_out(
    mut events: MessageReader<AuthSignOut>,
    mut sign_out_request: ResMut<AuthSignOutRequest>,
) {
    for _ in events.read() {
        sign_out_request.request();
    }
}

// `AuthOpenSignup` writes a typed request resource. The request is consumed by
// `state::auth_state::process_auth_requests_system`, not by Runtime.
pub fn handle_auth_open_signup(
    mut events: MessageReader<AuthOpenSignup>,
    mut open_signup_request: ResMut<AuthOpenSignupRequest>,
) {
    for _ in events.read() {
        open_signup_request.request();
    }
}

// `CopyLatestPerfFrame` writes a typed request resource. The request is
// consumed by `state::perf::process_perf_frame_copy_request_system` in
// `StateSchedule`, not by Runtime.
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
