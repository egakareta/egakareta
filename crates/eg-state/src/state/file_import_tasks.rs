/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! App-owned file-picker tasks.
//!
//! File dialog futures return owned data. The task component, rather than a
//! service-side callback, owns both the work and cancellation lifetime;
//! completed payloads are forwarded to the existing typed App-world handlers.

use bevy::{
    ecs::prelude::*,
    tasks::{IoTaskPool, Task, TaskPool, futures::check_ready},
};

use crate::events::async_results::{AudioImportResultEvent, LevelImportResultEvent};

/// One in-flight level picker request.
#[derive(Component)]
pub(crate) struct LevelImportTask(Task<Option<Vec<u8>>>);

/// One in-flight audio picker request.
#[derive(Component)]
pub(crate) struct AudioImportTask(Task<Option<(String, Vec<u8>)>>);

/// Spawn a level picker task after the caller has verified that no request is
/// already in flight.
pub(crate) fn spawn_level_import_task(commands: &mut Commands) {
    let task = IoTaskPool::get_or_init(TaskPool::new)
        .spawn(async move { eg_platform::services::import_level().await });
    commands.spawn(LevelImportTask(task));
}

/// Spawn an audio picker/persistence task after the caller has verified that
/// no request is already in flight.
pub(crate) fn spawn_audio_import_task(commands: &mut Commands) {
    let task = IoTaskPool::get_or_init(TaskPool::new)
        .spawn(async move { eg_platform::services::import_audio().await });
    commands.spawn(AudioImportTask(task));
}

/// Forward each completed level selection into the established App-world
/// import handler without blocking the frame.
pub(crate) fn poll_level_import_tasks_system(
    mut commands: Commands,
    mut tasks: Query<(Entity, &mut LevelImportTask)>,
    mut results: MessageWriter<LevelImportResultEvent>,
) {
    for (entity, mut task) in &mut tasks {
        let Some(bytes) = check_ready(&mut task.0) else {
            continue;
        };
        if let Some(bytes) = bytes {
            results.write(LevelImportResultEvent::new(bytes));
        }
        commands.entity(entity).despawn();
    }
}

/// Forward each completed audio selection into the established App-world
/// import handler without blocking the frame.
pub(crate) fn poll_audio_import_tasks_system(
    mut commands: Commands,
    mut tasks: Query<(Entity, &mut AudioImportTask)>,
    mut results: MessageWriter<AudioImportResultEvent>,
) {
    for (entity, mut task) in &mut tasks {
        let Some(result) = check_ready(&mut task.0) else {
            continue;
        };
        if let Some((filename, bytes)) = result {
            results.write(AudioImportResultEvent::new(filename, Some(bytes)));
        }
        commands.entity(entity).despawn();
    }
}
