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

use crate::commands::AppCommand;
use crate::events::{LegacyCommand, PendingCommands, ToLegacy};
use crate::state::{PerfFrameCopyRequest, RenderResizeRequest};

/// Start the browser sign-in flow.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct AuthSubmitSignIn;

impl ToLegacy for AuthSubmitSignIn {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::App(AppCommand::AuthSubmitSignIn)
    }
}

/// Sign out of the current account.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct AuthSignOut;

impl ToLegacy for AuthSignOut {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::App(AppCommand::AuthSignOut)
    }
}

/// Open the web signup page.
#[derive(Message, Debug, Clone, Copy, Default)]
pub struct AuthOpenSignup;

impl ToLegacy for AuthOpenSignup {
    fn to_legacy(self) -> LegacyCommand {
        LegacyCommand::App(AppCommand::AuthOpenSignup)
    }
}

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

pub fn handle_auth_submit_sign_in(
    mut events: MessageReader<AuthSubmitSignIn>,
    mut pending: ResMut<PendingCommands>,
) {
    for _ in events.read() {
        pending.app_commands.push(AppCommand::AuthSubmitSignIn);
    }
}

pub fn handle_auth_sign_out(
    mut events: MessageReader<AuthSignOut>,
    mut pending: ResMut<PendingCommands>,
) {
    for _ in events.read() {
        pending.app_commands.push(AppCommand::AuthSignOut);
    }
}

pub fn handle_auth_open_signup(
    mut events: MessageReader<AuthOpenSignup>,
    mut pending: ResMut<PendingCommands>,
) {
    for _ in events.read() {
        pending.app_commands.push(AppCommand::AuthOpenSignup);
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
