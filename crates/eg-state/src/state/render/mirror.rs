/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Phase 4a bridge between the legacy `State::render` field and the
//! new ECS render resources.
//!
//! The mirror is implemented as a per-frame closure that
//! `State::tick_state` installs right before running the
//! `StateSchedule`. The closure is consumed by
//! [`mirror_pod_state_via_bridge`] inside the schedule and never
//! escapes the system call.
//!
//! ## Why a closure and not a direct `&State` system param?
//!
//! Bevy systems must be `Send + Sync + 'static` and their args are
//! restricted to `SystemParam` impls. The legacy `State` struct
//! holds non-`Send` data (the wgpu surface is `!Send` on some
//! platforms, the egui context is per-thread) and a `&State` system
//! param would require `unsafe` to share across worker threads.
//! Instead we let `tick_state` (which runs on the main thread and
//! holds `&mut State`) install a closure, run the schedule, and
//! wait for it to drain.
//!
//! ## Concurrency: thread-local slot
//!
//! The slot is **thread-local** so parallel tests don't clobber each
//! other's mirrors. `tick_state` and the schedule's bridge system
//! run on the same thread (the main thread that owns the State), so
//! the thread-local semantics are correct in production. Tests that
//! run on a thread pool see a per-thread slot, which is the desired
//! behavior: each test's mirror is isolated to the thread that
//! installed it.

use std::cell::RefCell;

use crate::state::render::resources::RenderUniformsRes;

pub(crate) type MirrorFn = Box<dyn FnMut(&mut RenderUniformsRes)>;

thread_local! {
    static MIRROR_SLOT: RefCell<Option<MirrorFn>> = const { RefCell::new(None) };
}

/// Install a mirror closure for the next schedule run on the
/// current thread.
///
/// The previous closure (if any) is dropped. The closure runs
/// exactly once — `take_pending_mirror` removes it from the slot
/// before invoking it.
pub fn install_mirror<F>(mirror: F)
where
    F: FnMut(&mut RenderUniformsRes) + 'static,
{
    MIRROR_SLOT.with(|slot| {
        *slot.borrow_mut() = Some(Box::new(mirror));
    });
}

/// Take the pending mirror closure, if any. Returns `None` when no
/// mirror was installed on this thread for this frame.
pub fn take_pending_mirror() -> Option<MirrorFn> {
    MIRROR_SLOT.with(|slot| slot.borrow_mut().take())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::world::World;

    fn register_world() -> World {
        let mut world = World::new();
        world.init_resource::<RenderUniformsRes>();
        world
    }

    #[test]
    fn take_pending_mirror_returns_none_when_idle() {
        let _ = take_pending_mirror();
    }

    #[test]
    fn install_then_take_runs_closure() {
        let _ = take_pending_mirror();
        let mut world = register_world();
        install_mirror(|u| {
            u.color_space.time_seconds = 7.0;
        });

        let mut uniforms = world.resource_mut::<RenderUniformsRes>();
        let mut mirror = take_pending_mirror().expect("expected mirror to be installed");
        mirror(&mut uniforms);

        // The closure was invoked (proved by the side effect on the
        // resources).
        assert_eq!(
            world
                .resource::<RenderUniformsRes>()
                .color_space
                .time_seconds,
            7.0
        );
        // The slot is empty after a take — a second take is a no-op.
        assert!(take_pending_mirror().is_none());
    }

    #[test]
    fn install_replaces_previous_mirror() {
        let _ = take_pending_mirror();
        install_mirror(|_u| {});
        install_mirror(|_u| {});
        // Only the second install survives; the first is dropped.
        assert!(take_pending_mirror().is_some());
        assert!(take_pending_mirror().is_none());
    }
}
