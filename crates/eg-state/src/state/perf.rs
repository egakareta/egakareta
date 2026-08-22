/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::fmt::Write as _;

use bevy_ecs::prelude::*;
use puffin::{FrameView, GlobalFrameView, Reader, ScopeCollection};

use super::{PerfFrameCopyRequest, PerfRes};

#[derive(Default)]
pub struct PerfState {
    /// Whether the puffin overlay is enabled.
    pub overlay_enabled: bool,
    /// The smoothed frames per second (FPS) value, updated each frame.
    pub fps_smoothed: f32,
    /// The puffin [`GlobalFrameView`].
    pub global_frame_view: GlobalFrameView,
}

impl PerfState {
    /// Format the latest frame from a [`FrameView`] as a textual tree.
    fn format_frame(frame_view: &FrameView) -> Option<String> {
        let frame_data = frame_view.latest_frame()?;
        let unpacked = frame_data.unpacked().ok()?;
        let scope_collection = frame_view.scope_collection();

        let frame_index = unpacked.frame_index();
        let duration_ms = unpacked.duration_ns() as f64 * 1e-6;

        let mut out = String::new();
        writeln!(out, "Frame #{frame_index} ({duration_ms:.3} ms)").ok()?;

        let mut threads: Vec<_> = unpacked.thread_streams.iter().collect();
        threads.sort_by_key(|(ti, _)| ti.name.clone());

        for (thread_info, stream_info) in threads {
            let thread_ms = (stream_info.range_ns.1 - stream_info.range_ns.0) as f64 * 1e-6;
            writeln!(out, "Thread \"{}\" ({thread_ms:.3} ms)", thread_info.name).ok()?;

            let reader = Reader::from_start(&stream_info.stream);
            let top_scopes = reader.read_top_scopes().ok()?;
            for (i, scope) in top_scopes.iter().enumerate() {
                let is_last = i == top_scopes.len() - 1;
                Self::format_scope(
                    &mut out,
                    scope,
                    &stream_info.stream,
                    scope_collection,
                    "",
                    is_last,
                );
            }
        }

        Some(out)
    }

    /// Recursively format a scope and its children as a tree.
    fn format_scope(
        out: &mut String,
        scope: &puffin::Scope<'_>,
        stream: &puffin::Stream,
        scope_collection: &ScopeCollection,
        prefix: &str,
        is_last: bool,
    ) {
        let connector = if is_last { "└─ " } else { "├─ " };
        let name = scope_collection
            .fetch_by_id(&scope.id)
            .map_or_else(|| "<unknown>".to_owned(), |d| d.name().to_string());
        let duration_ms = scope.record.duration_ns as f64 * 1e-6;
        let data = if scope.record.data.is_empty() {
            String::new()
        } else {
            format!(" \"{}\"", scope.record.data)
        };

        let _ = writeln!(out, "{prefix}{connector}{name} ({duration_ms:.3} ms){data}");

        let child_prefix = format!("{prefix}{}", if is_last { "   " } else { "│  " });
        if let Ok(reader) = Reader::with_offset(stream, scope.child_begin_position)
            && let Ok(children) = reader.read_top_scopes()
        {
            for (i, child) in children.iter().enumerate() {
                let child_is_last = i == children.len() - 1;
                Self::format_scope(
                    out,
                    child,
                    stream,
                    scope_collection,
                    &child_prefix,
                    child_is_last,
                );
            }
        }
    }

    /// Format the latest profiling frame as a human-readable text tree for clipboard.
    pub fn format_latest_frame(&self) -> Option<String> {
        let frame_view = self.global_frame_view.lock();
        Self::format_frame(&frame_view)
    }
}

pub fn process_perf_frame_copy_request_system(
    mut request: ResMut<PerfFrameCopyRequest>,
    perf: Res<PerfRes>,
) {
    if !request.take() {
        return;
    }

    if let Some(text) = perf.0.format_latest_frame() {
        let _ = bevy_clipboard::Clipboard::default().set_text(text);
    }
}

#[cfg(test)]
mod tests {
    use bevy_ecs::prelude::*;

    use super::process_perf_frame_copy_request_system;
    use crate::state::{PerfFrameCopyRequest, PerfRes};

    #[test]
    fn perf_frame_copy_system_consumes_request_without_frame_data() {
        let mut world = World::new();
        world.init_resource::<PerfRes>();
        world.init_resource::<PerfFrameCopyRequest>();
        world.resource_mut::<PerfFrameCopyRequest>().request();

        let mut schedule = Schedule::default();
        schedule.add_systems(process_perf_frame_copy_request_system);
        schedule.run(&mut world);

        assert!(
            !world.resource::<PerfFrameCopyRequest>().pending,
            "perf copy request must be consumed by the scheduled system"
        );
    }
}
