/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, Mutex},
};

use bevy::{
    ecs::{prelude::*, world::World},
    tasks::{AsyncComputeTaskPool, Task, TaskPool, futures::check_ready},
};
use eg_audio::{
    audio::{AudioBackend, runtime_asset_source_key},
    service::{AudioByteCache, EditorAudioCache, WaveformCacheEntry, WaveformResult},
};
use eg_types::PersistentWaveformCacheEntry;

use super::EditorTimingState;

/// Authoritative platform audio backend shared with the App world while the
/// remaining playback callers are removed from the compatibility `State`.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Resource, Clone)]
pub struct AudioBackendRes(Arc<Mutex<AudioBackend>>);

#[cfg(target_arch = "wasm32")]
#[derive(Clone)]
pub struct AudioBackendRes(Arc<Mutex<AudioBackend>>);

#[cfg(not(target_arch = "wasm32"))]
pub type AudioBackendSystemParam<'w> = bevy::ecs::change_detection::Res<'w, AudioBackendRes>;

#[cfg(target_arch = "wasm32")]
pub type AudioBackendSystemParam<'w> = bevy::ecs::change_detection::NonSend<'w, AudioBackendRes>;

pub(crate) fn with_audio_backend<R>(world: &World, f: impl FnOnce(&AudioBackendRes) -> R) -> R {
    #[cfg(not(target_arch = "wasm32"))]
    {
        f(world.resource::<AudioBackendRes>())
    }
    #[cfg(target_arch = "wasm32")]
    {
        f(world.non_send::<AudioBackendRes>())
    }
}

impl Default for AudioBackendRes {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(AudioBackend::new())))
    }
}

impl AudioBackendRes {
    fn with_mut<R>(&self, f: impl FnOnce(&mut AudioBackend) -> R) -> R {
        f(&mut self.0.lock().expect("audio backend mutex poisoned"))
    }

    pub fn set_preferred_backend_name(&self, backend_name: &str) -> bool {
        self.with_mut(|backend| backend.set_preferred_backend_name(backend_name))
    }

    pub fn stop(&self) {
        self.with_mut(AudioBackend::stop);
    }

    pub fn pause(&self) {
        self.with_mut(AudioBackend::pause);
    }

    pub fn resume(&self) {
        self.with_mut(AudioBackend::resume);
    }

    pub fn ensure_device(&self) {
        self.with_mut(|backend| {
            backend.ensure_device();
        });
    }

    pub fn playback_time_seconds(&self) -> Option<f32> {
        self.with_mut(|backend| backend.playback_time_seconds())
    }

    pub fn is_playing(&self) -> bool {
        self.with_mut(|backend| backend.is_playing())
    }

    pub fn set_speed(&self, speed: f32) {
        self.with_mut(|backend| backend.set_speed(speed));
    }

    pub fn play_sfx(&self, asset_bytes: &'static [u8]) {
        self.with_mut(|backend| backend.play_sfx(asset_bytes));
    }

    pub fn start_with_bytes_at(&self, bytes: &[u8], start_seconds: f32) {
        self.with_mut(|backend| backend.start_with_bytes_at(bytes, start_seconds));
    }

    pub fn warmup_with_bytes_at(&self, bytes: &[u8], start_seconds: f32) {
        self.with_mut(|backend| backend.warmup_with_bytes_at(bytes, start_seconds));
    }

    pub fn start_preloaded_asset_at(&self, level_name: &str, bytes: &[u8], start_seconds: f32) {
        self.with_mut(|backend| {
            backend.start_preloaded_asset_at(level_name, bytes, start_seconds);
        });
    }

    pub fn warmup_preloaded_asset_at(&self, level_name: &str, bytes: &[u8], start_seconds: f32) {
        self.with_mut(|backend| {
            backend.warmup_preloaded_asset_at(level_name, bytes, start_seconds);
        });
    }

    pub fn start_at(&self, level_name: &str, start_seconds: f32) {
        self.with_mut(|backend| backend.start_at(level_name, start_seconds));
    }

    pub fn warmup_at(&self, level_name: &str, start_seconds: f32) {
        self.with_mut(|backend| backend.warmup_at(level_name, start_seconds));
    }
}

pub(crate) fn start_audio_source_at_seconds_for_resources(
    is_editor: bool,
    playtesting_editor: bool,
    level_name: &str,
    start_seconds: f32,
    audio: &AudioBackendRes,
    editor_audio: &EditorAudioCacheRes,
    runtime_preload: &RuntimeAudioPreloadRes,
) {
    let source_key = runtime_asset_source_key(level_name, "audio.mp3");

    if (is_editor || playtesting_editor)
        && let Some(bytes) = editor_audio.bytes_for_level_audio(level_name)
    {
        audio.start_with_bytes_at(&bytes, start_seconds);
        return;
    }

    if let Some(bytes) = runtime_preload.bytes(&source_key) {
        audio.start_preloaded_asset_at(level_name, &bytes, start_seconds);
    } else {
        audio.start_at(level_name, start_seconds);
    }
}

#[derive(Default)]
struct RuntimeAudioPreloadCache {
    preloaded_audio: AudioByteCache,
    preloading_source_keys: HashSet<String>,
}

#[derive(Resource, Clone, Default)]
pub struct RuntimeAudioPreloadRes(Arc<Mutex<RuntimeAudioPreloadCache>>);

impl RuntimeAudioPreloadRes {
    pub fn contains_preloaded(&self, source_key: &str) -> bool {
        self.0
            .lock()
            .expect("runtime audio preload cache mutex poisoned")
            .preloaded_audio
            .contains_key(source_key)
    }

    pub fn contains_preloading(&self, source_key: &str) -> bool {
        self.0
            .lock()
            .expect("runtime audio preload cache mutex poisoned")
            .preloading_source_keys
            .contains(source_key)
    }

    pub fn mark_preloading(&self, source_key: String) {
        self.0
            .lock()
            .expect("runtime audio preload cache mutex poisoned")
            .preloading_source_keys
            .insert(source_key);
    }

    pub fn apply_result(&self, source_key: String, bytes: Option<Vec<u8>>) {
        let mut cache = self
            .0
            .lock()
            .expect("runtime audio preload cache mutex poisoned");
        cache.preloading_source_keys.remove(&source_key);
        if let Some(bytes) = bytes {
            cache.preloaded_audio.insert(source_key, bytes);
        }
    }

    pub fn insert_preloaded(&self, source_key: String, bytes: Vec<u8>) {
        self.0
            .lock()
            .expect("runtime audio preload cache mutex poisoned")
            .preloaded_audio
            .insert(source_key, bytes);
    }

    pub fn bytes(&self, source_key: &str) -> Option<Vec<u8>> {
        self.0
            .lock()
            .expect("runtime audio preload cache mutex poisoned")
            .preloaded_audio
            .get(source_key)
            .cloned()
    }

    #[cfg(test)]
    pub fn preloading_len(&self) -> usize {
        self.0
            .lock()
            .expect("runtime audio preload cache mutex poisoned")
            .preloading_source_keys
            .len()
    }
}

/// App-owned background load for one runtime audio source.
///
/// The source key is both the destination cache key and the request identity:
/// callers mark it preloading before spawning, so duplicate requests cannot
/// create unbounded work. The task entity owns cancellation automatically.
#[derive(Component)]
pub(crate) struct RuntimeAudioPreloadTask {
    source_key: String,
    task: Task<Option<Vec<u8>>>,
}

/// Queue a deduplicated built-in-audio preload as a Bevy-owned task.
pub(crate) fn queue_runtime_audio_preload_task(
    commands: &mut Commands,
    cache: &RuntimeAudioPreloadRes,
    level_name: &str,
) {
    let source_key = runtime_asset_source_key(level_name, "audio.mp3");
    if cache.contains_preloaded(&source_key) || cache.contains_preloading(&source_key) {
        return;
    }

    cache.mark_preloading(source_key.clone());
    let level_name = level_name.to_owned();
    let task = AsyncComputeTaskPool::get_or_init(TaskPool::new)
        .spawn(async move { eg_audio::service::load_audio_preload(level_name).await });
    commands.spawn(RuntimeAudioPreloadTask { source_key, task });
}

/// World-level equivalent for transition hooks that already own `&mut World`.
pub(crate) fn queue_runtime_audio_preload_task_world(world: &mut World, level_name: &str) {
    let cache = world.resource::<RuntimeAudioPreloadRes>().clone();
    let source_key = runtime_asset_source_key(level_name, "audio.mp3");
    if cache.contains_preloaded(&source_key) || cache.contains_preloading(&source_key) {
        return;
    }

    cache.mark_preloading(source_key.clone());
    let level_name = level_name.to_owned();
    let task = AsyncComputeTaskPool::get_or_init(TaskPool::new)
        .spawn(async move { eg_audio::service::load_audio_preload(level_name).await });
    world.spawn(RuntimeAudioPreloadTask { source_key, task });
}

/// Apply completed runtime-audio preload tasks without blocking the frame.
pub(crate) fn poll_runtime_audio_preload_tasks_system(
    mut commands: Commands,
    cache: Res<RuntimeAudioPreloadRes>,
    mut tasks: Query<(Entity, &mut RuntimeAudioPreloadTask)>,
) {
    for (entity, mut task) in &mut tasks {
        let Some(bytes) = check_ready(&mut task.task) else {
            continue;
        };
        apply_runtime_audio_preload_result(&cache, task.source_key.clone(), bytes);
        commands.entity(entity).despawn();
    }
}

/// Apply a completed preload result after its task identity has been accepted.
pub(crate) fn apply_runtime_audio_preload_result(
    cache: &RuntimeAudioPreloadRes,
    source_key: String,
    bytes: Option<Vec<u8>>,
) {
    cache.apply_result(source_key, bytes);
}

#[derive(Resource, Clone, Default)]
pub struct EditorAudioCacheRes(Arc<Mutex<EditorAudioCache>>);

impl EditorAudioCacheRes {
    #[cfg(test)]
    pub fn from_local_files(local_file_bytes: HashMap<String, Vec<u8>>) -> Self {
        Self(Arc::new(Mutex::new(EditorAudioCache::from_local_files(
            local_file_bytes,
        ))))
    }

    pub(crate) fn with_mut<R>(&self, f: impl FnOnce(&mut EditorAudioCache) -> R) -> R {
        f(&mut self.0.lock().expect("editor audio cache mutex poisoned"))
    }

    /// Replace the persisted local-file fallback cache after startup storage
    /// loading completes without replacing the App-owned resource itself.
    pub fn replace_local_files(&self, local_file_bytes: HashMap<String, Vec<u8>>) {
        self.with_mut(|cache| cache.replace_local_files(local_file_bytes));
    }

    pub fn insert_level_audio(&self, level_name: &str, bytes: Vec<u8>) -> String {
        self.with_mut(|cache| cache.insert_level_audio(level_name, bytes))
    }

    pub fn bytes_for_level_audio(&self, level_name: &str) -> Option<Vec<u8>> {
        self.with_mut(|cache| cache.bytes_for_level_audio(level_name).cloned())
    }

    pub fn bytes_for_source(&self, source_key: &str) -> Option<Vec<u8>> {
        self.with_mut(|cache| cache.bytes_for_source(source_key).cloned())
    }

    pub fn waveform(&self, source_key: &str) -> Option<WaveformCacheEntry> {
        self.with_mut(|cache| cache.waveform(source_key).cloned())
    }

    pub fn insert_waveform(&self, source_key: String, entry: WaveformCacheEntry) {
        self.with_mut(|cache| cache.insert_waveform(source_key, entry));
    }

    pub fn remove_waveform(&self, source_key: &str) {
        self.with_mut(|cache| cache.remove_waveform(source_key));
    }

    pub fn contains_waveform(&self, source_key: &str) -> bool {
        self.with_mut(|cache| cache.contains_waveform(source_key))
    }

    pub fn loading_source(&self) -> Option<String> {
        self.with_mut(|cache| cache.loading_source().map(str::to_owned))
    }

    pub fn set_loading_source(&self, source_key: Option<String>) {
        self.with_mut(|cache| cache.set_loading_source(source_key));
    }

    pub fn clear_loading_source_if(&self, source_key: &str) {
        self.with_mut(|cache| cache.clear_loading_source_if(source_key));
    }
}

/// Bevy-owned background waveform decoding task.
///
/// The task retains its completed progress records and publishes a bounded
/// number of them per frame, preventing a long waveform from monopolizing an
/// `Update` pass while preserving ordered `Started`/`Chunk`/terminal output.
#[derive(Component)]
pub(crate) struct WaveformLoadTask {
    task: Option<Task<Vec<WaveformResult>>>,
    pending_results: VecDeque<WaveformResult>,
}

const WAVEFORM_RESULTS_PER_FRAME: usize = 8;

pub(crate) fn queue_waveform_load_task(
    commands: &mut Commands,
    level_name: String,
    editor_cache: &EditorAudioCacheRes,
    runtime_preload: &RuntimeAudioPreloadRes,
    timing: &mut EditorTimingState,
) {
    let Some(cached_bytes) =
        prepare_waveform_load(&level_name, editor_cache, runtime_preload, timing)
    else {
        return;
    };
    commands.spawn(new_waveform_load_task(level_name, cached_bytes));
}

/// World-level equivalent for exclusive handlers that already own the App
/// world and therefore do not need a deferred `Commands` buffer.
pub(crate) fn queue_waveform_load_task_world(world: &mut World, level_name: String) {
    let editor_cache = world.resource::<EditorAudioCacheRes>().clone();
    let runtime_preload = world.resource::<RuntimeAudioPreloadRes>().clone();
    let cached_bytes = {
        let mut timing = world.resource_mut::<super::EditorTimingRes>();
        prepare_waveform_load(&level_name, &editor_cache, &runtime_preload, &mut timing.0)
    };
    if let Some(cached_bytes) = cached_bytes {
        world.spawn(new_waveform_load_task(level_name, cached_bytes));
    }
}

fn prepare_waveform_load(
    level_name: &str,
    editor_cache: &EditorAudioCacheRes,
    runtime_preload: &RuntimeAudioPreloadRes,
    timing: &mut EditorTimingState,
) -> Option<Option<Vec<u8>>> {
    let source_key = runtime_asset_source_key(level_name, "audio.mp3");

    if let Some(entry) = editor_cache.waveform(&source_key) {
        timing.waveform_samples = entry.samples;
        timing.waveform_sample_rate = entry.sample_rate;
        timing.waveform_window_size = entry.window_size;
        timing.waveform_loading = !entry.complete;
        timing.waveform_complete = entry.complete;
        if entry.complete {
            editor_cache.clear_loading_source_if(&source_key);
        }
        return None;
    }

    if editor_cache.loading_source().as_deref() == Some(source_key.as_str()) {
        return None;
    }

    editor_cache.set_loading_source(Some(source_key.clone()));
    timing.waveform_samples.clear();
    timing.waveform_sample_rate = 0;
    timing.waveform_window_size = eg_audio::service::WAVEFORM_WINDOW;
    timing.waveform_loading = true;
    timing.waveform_complete = false;

    let cached_bytes = editor_cache
        .bytes_for_level_audio(level_name)
        .or_else(|| runtime_preload.bytes(&source_key));
    Some(cached_bytes)
}

fn new_waveform_load_task(level_name: String, cached_bytes: Option<Vec<u8>>) -> WaveformLoadTask {
    let task = AsyncComputeTaskPool::get_or_init(TaskPool::new)
        .spawn(async move { eg_audio::service::load_waveform(level_name, cached_bytes).await });
    WaveformLoadTask {
        task: Some(task),
        pending_results: VecDeque::new(),
    }
}

/// Publish completed waveform task results in bounded frame-sized batches.
pub(crate) fn poll_waveform_load_tasks_system(
    mut commands: Commands,
    mut tasks: Query<(Entity, &mut WaveformLoadTask)>,
    mut results: MessageWriter<crate::events::async_results::WaveformResultEvent>,
) {
    for (entity, mut task) in &mut tasks {
        if let Some(compute) = &mut task.task
            && let Some(completed) = check_ready(compute)
        {
            task.pending_results = completed.into();
            task.task = None;
        }

        for _ in 0..WAVEFORM_RESULTS_PER_FRAME {
            let Some(result) = task.pending_results.pop_front() else {
                break;
            };
            results.write(crate::events::async_results::WaveformResultEvent::from_result(result));
        }

        if task.task.is_none() && task.pending_results.is_empty() {
            commands.entity(entity).despawn();
        }
    }
}

/// Convert a completed waveform into the owned persistent payload.
pub(crate) fn persistent_waveform_cache_entry(
    entry: &WaveformCacheEntry,
) -> PersistentWaveformCacheEntry {
    PersistentWaveformCacheEntry::new(entry.samples.clone(), entry.sample_rate, entry.window_size)
}

#[cfg(test)]
mod tests {
    use eg_audio::audio::runtime_asset_source_key;

    use super::{
        EditorAudioCacheRes, EditorTimingState, RuntimeAudioPreloadRes, WaveformCacheEntry,
        apply_runtime_audio_preload_result, prepare_waveform_load,
    };

    #[test]
    fn waveform_load_prefers_cache_and_short_circuits_inflight_requests() {
        let editor_cache = EditorAudioCacheRes::default();
        let runtime_preload = RuntimeAudioPreloadRes::default();
        let mut timing = EditorTimingState::default();
        let source_key = runtime_asset_source_key("Test Level", "audio.mp3");

        editor_cache.insert_waveform(
            source_key.clone(),
            WaveformCacheEntry {
                samples: vec![0.3, 0.9],
                sample_rate: 32_000,
                window_size: eg_audio::service::WAVEFORM_WINDOW,
                complete: true,
            },
        );
        editor_cache.set_loading_source(Some("other:key".to_string()));
        let request =
            prepare_waveform_load("Test Level", &editor_cache, &runtime_preload, &mut timing);
        assert!(request.is_none());
        assert_eq!(timing.waveform_samples, vec![0.3, 0.9]);
        assert_eq!(timing.waveform_sample_rate, 32_000);
        assert_eq!(editor_cache.loading_source(), Some("other:key".to_string()));

        editor_cache.remove_waveform(&source_key);
        timing.waveform_samples = vec![7.0];
        timing.waveform_sample_rate = 7;
        editor_cache.set_loading_source(Some(source_key.clone()));
        let request =
            prepare_waveform_load("Test Level", &editor_cache, &runtime_preload, &mut timing);
        assert!(request.is_none());
        assert_eq!(timing.waveform_samples, vec![7.0]);
        assert_eq!(timing.waveform_sample_rate, 7);
    }

    #[test]
    fn uncached_waveform_load_marks_loading_and_clears_timing() {
        let editor_cache = EditorAudioCacheRes::default();
        let runtime_preload = RuntimeAudioPreloadRes::default();
        let mut timing = EditorTimingState {
            waveform_samples: vec![1.0, 2.0],
            waveform_sample_rate: 11_025,
            ..EditorTimingState::default()
        };
        let source_key = runtime_asset_source_key("Test Level", "audio.mp3");

        let request =
            prepare_waveform_load("Test Level", &editor_cache, &runtime_preload, &mut timing);
        assert_eq!(request, Some(None));

        assert!(timing.waveform_samples.is_empty());
        assert_eq!(timing.waveform_sample_rate, 0);
        assert_eq!(editor_cache.loading_source(), Some(source_key));
    }

    #[test]
    fn completed_runtime_preload_result_updates_and_unmarks_the_source() {
        let cache = RuntimeAudioPreloadRes::default();
        let source_key = runtime_asset_source_key("Test Level", "audio.mp3");
        cache.mark_preloading(source_key.clone());

        apply_runtime_audio_preload_result(&cache, source_key.clone(), Some(vec![1, 2, 3]));

        assert!(cache.contains_preloaded(&source_key));
        assert!(!cache.contains_preloading(&source_key));
    }
}
