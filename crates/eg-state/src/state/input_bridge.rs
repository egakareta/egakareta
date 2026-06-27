/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Platform input resources for the Bevy App world.

use bevy_ecs::prelude::*;

use crate::commands::InputEvent;

/// Platform input events captured for the current App world.
#[derive(Resource, Default, Debug)]
pub struct PlatformInputQueue {
    events: Vec<InputEvent>,
}

impl PlatformInputQueue {
    /// Push an input event into the queue.
    pub fn push(&mut self, event: InputEvent) {
        self.events.push(event);
    }

    /// Returns true when no input events are queued.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Number of queued input events.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Read the queued input events.
    pub fn events(&self) -> &[InputEvent] {
        &self.events
    }

    /// Drain queued input events.
    pub fn drain(&mut self) -> impl Iterator<Item = InputEvent> + '_ {
        self.events.drain(..)
    }
}

/// Register platform input resources on a World.
pub fn register_platform_input_resources(world: &mut World) {
    world.init_resource::<PlatformInputQueue>();
}

/// Queue one platform input event into a World.
pub fn enqueue_platform_input(world: &mut World, event: InputEvent) {
    world.resource_mut::<PlatformInputQueue>().push(event);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enqueue_platform_input_buffers_event() {
        let mut world = World::new();
        register_platform_input_resources(&mut world);

        enqueue_platform_input(
            &mut world,
            InputEvent::Resize {
                width: 640,
                height: 360,
            },
        );

        let queue = world.resource::<PlatformInputQueue>();
        assert_eq!(queue.len(), 1);
        assert!(matches!(
            queue.events()[0],
            InputEvent::Resize {
                width: 640,
                height: 360
            }
        ));
    }

    #[test]
    fn platform_input_queue_drain_clears_events() {
        let mut queue = PlatformInputQueue::default();
        queue.push(InputEvent::Zoom(1.5));

        let drained = queue.drain().collect::<Vec<_>>();

        assert_eq!(drained.len(), 1);
        assert!(queue.is_empty());
    }
}
