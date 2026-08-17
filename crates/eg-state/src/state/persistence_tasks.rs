/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! App-owned persistence tasks for asynchronous platform storage writes.

use bevy::{
    ecs::prelude::*,
    tasks::{IoTaskPool, Task, TaskPool, futures::check_ready},
};
use eg_types::{AppSettings, PersistentWaveformCacheEntry};

#[derive(Component)]
pub(crate) struct PersistenceTask(Task<PersistenceResult>);

enum PersistenceResult {
    AppSettings(Result<(), String>),
    Waveform {
        cache_key: String,
        result: Result<(), String>,
    },
    #[cfg(not(test))]
    LevelExport {
        filename: String,
        result: Result<(), String>,
    },
}

pub(crate) fn queue_app_settings_persist(commands: &mut Commands, settings: AppSettings) {
    let task = IoTaskPool::get_or_init(TaskPool::new).spawn(async move {
        PersistenceResult::AppSettings(
            eg_platform::io::save_app_settings_to_storage(&settings).await,
        )
    });
    commands.spawn(PersistenceTask(task));
}

#[cfg(not(test))]
pub(crate) fn queue_app_settings_persist_world(world: &mut World, settings: AppSettings) {
    let task = IoTaskPool::get_or_init(TaskPool::new).spawn(async move {
        PersistenceResult::AppSettings(
            eg_platform::io::save_app_settings_to_storage(&settings).await,
        )
    });
    world.spawn(PersistenceTask(task));
}

pub(crate) fn queue_waveform_persist_world(
    world: &mut World,
    cache_key: String,
    entry: PersistentWaveformCacheEntry,
) {
    let task = IoTaskPool::get_or_init(TaskPool::new).spawn(async move {
        let result = eg_platform::io::save_waveform_to_storage(&cache_key, &entry).await;
        PersistenceResult::Waveform { cache_key, result }
    });
    world.spawn(PersistenceTask(task));
}

#[cfg(not(test))]
pub(crate) fn queue_level_export_persist(commands: &mut Commands, filename: String, data: Vec<u8>) {
    let task = IoTaskPool::get_or_init(TaskPool::new).spawn(async move {
        let result = eg_platform::io::save_level_export(&filename, &data).await;
        PersistenceResult::LevelExport { filename, result }
    });
    commands.spawn(PersistenceTask(task));
}

#[cfg(not(test))]
pub(crate) fn queue_pending_level_exports_system(
    mut commands: Commands,
    mut exports: ResMut<super::LevelExportServiceRes>,
) {
    for (filename, data) in exports.take_pending_exports() {
        queue_level_export_persist(&mut commands, filename, data);
    }
}

#[cfg(test)]
pub(crate) fn queue_pending_level_exports_system(_exports: ResMut<super::LevelExportServiceRes>) {
    // Test fixtures inspect the request rather than performing platform I/O.
}

/// Report completed persistence operations and release their task entities.
pub(crate) fn poll_persistence_tasks_system(
    mut commands: Commands,
    mut tasks: Query<(Entity, &mut PersistenceTask)>,
) {
    for (entity, mut task) in &mut tasks {
        let Some(result) = check_ready(&mut task.0) else {
            continue;
        };
        match result {
            PersistenceResult::AppSettings(Err(error)) => {
                log::error!("Failed to persist app settings: {error}");
            }
            PersistenceResult::Waveform {
                cache_key,
                result: Err(error),
            } => {
                log::error!("Failed to save waveform cache '{cache_key}': {error}");
            }
            #[cfg(not(test))]
            PersistenceResult::LevelExport {
                filename,
                result: Err(error),
            } => {
                log::error!("Export '{filename}' failed: {error}");
            }
            PersistenceResult::AppSettings(Ok(()))
            | PersistenceResult::Waveform { result: Ok(()), .. } => {}
            #[cfg(not(test))]
            PersistenceResult::LevelExport { result: Ok(()), .. } => {}
        }
        commands.entity(entity).despawn();
    }
}
