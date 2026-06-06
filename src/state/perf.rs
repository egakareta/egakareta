/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use std::fmt::Write as _;

use puffin::{FrameView, GlobalFrameView, Reader, ScopeCollection};

pub(crate) struct EditorPerfState {
    pub(crate) overlay_enabled: bool,
    pub(crate) fps_smoothed: f32,
    pub(crate) frame_view: GlobalFrameView,
}

impl EditorPerfState {
    pub(crate) fn new() -> Self {
        Self {
            overlay_enabled: false,
            fps_smoothed: 0.0,
            frame_view: GlobalFrameView::default(),
        }
    }
}

/// Format the latest frame from a [`FrameView`] as a text tree.
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
            format_scope(
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
    if let Ok(reader) = Reader::with_offset(stream, scope.child_begin_position) {
        if let Ok(children) = reader.read_top_scopes() {
            for (i, child) in children.iter().enumerate() {
                let child_is_last = i == children.len() - 1;
                format_scope(
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
}

impl super::State {
    pub(crate) fn toggle_perf_overlay(&mut self) {
        self.editor.perf.overlay_enabled = !self.editor.perf.overlay_enabled;
        puffin::set_scopes_on(self.editor.perf.overlay_enabled);
    }

    pub(crate) fn perf_overlay_enabled(&self) -> bool {
        self.editor.perf.overlay_enabled
    }

    pub(crate) fn perf_fps(&self) -> f32 {
        self.editor.perf.fps_smoothed
    }

    pub(crate) fn perf_graphics_backend(&self) -> String {
        format!("{:?}", self.render.gpu.adapter_info.backend)
    }

    pub(crate) fn perf_audio_backend(&self) -> String {
        self.audio.state.runtime.backend_name()
    }

    /// Format the latest profiling frame as a human-readable text tree for clipboard.
    pub(crate) fn format_latest_frame(&self) -> Option<String> {
        let frame_view = self.editor.perf.frame_view.lock();
        format_frame(&frame_view)
    }
}
