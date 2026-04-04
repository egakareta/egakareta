/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
use std::collections::HashSet;

use super::{EditorDirtyFlags, EditorSubsystem, State};
use crate::block_repository::resolve_block_definition;
use crate::types::AppPhase;

impl EditorSubsystem {
    fn normalized_group_path(path: &[String]) -> Vec<String> {
        path.iter()
            .flat_map(|segment| segment.split('/'))
            .map(str::trim)
            .filter(|segment| !segment.is_empty())
            .map(|segment| segment.to_string())
            .collect()
    }

    fn object_name_seed(block_id: &str) -> String {
        let label = resolve_block_definition(block_id).display_name.trim();
        if label.is_empty() {
            "Block".to_string()
        } else {
            label.to_string()
        }
    }

    fn next_unique_object_name(&self, base: &str) -> String {
        let used: HashSet<String> = self
            .objects
            .iter()
            .filter_map(|object| {
                let trimmed = object.name.trim();
                (!trimmed.is_empty()).then(|| trimmed.to_ascii_lowercase())
            })
            .collect();

        let mut index = 1usize;
        loop {
            let candidate = format!("{base} {index}");
            if !used.contains(&candidate.to_ascii_lowercase()) {
                return candidate;
            }
            index += 1;
        }
    }

    pub(crate) fn assign_default_names_for_indices(&mut self, indices: &[usize]) {
        for &index in indices {
            if index >= self.objects.len() {
                continue;
            }

            let trimmed = self.objects[index].name.trim().to_string();
            if !trimmed.is_empty() {
                self.objects[index].name = trimmed;
                continue;
            }

            let base = Self::object_name_seed(&self.objects[index].block_id);
            self.objects[index].name = self.next_unique_object_name(&base);
        }
    }

    fn unique_group_name_under(&self, parent: &[String]) -> String {
        let existing: HashSet<String> = self
            .objects
            .iter()
            .filter(|object| object.group_path.len() > parent.len())
            .filter(|object| object.group_path.starts_with(parent))
            .map(|object| object.group_path[parent.len()].to_ascii_lowercase())
            .collect();

        let mut index = 1usize;
        loop {
            let candidate = format!("Group {index}");
            if !existing.contains(&candidate.to_ascii_lowercase()) {
                return candidate;
            }
            index += 1;
        }
    }
}

impl State {
    pub(crate) fn editor_objects(&self) -> &[crate::types::LevelObject] {
        &self.editor.objects
    }

    pub(crate) fn editor_selected_indices(&self) -> Vec<usize> {
        self.editor.selected_indices_normalized()
    }

    pub(crate) fn editor_select_objects_from_explorer(
        &mut self,
        indices: Vec<usize>,
        additive: bool,
    ) {
        if self.phase != AppPhase::Editor {
            return;
        }

        let mut next = if additive {
            self.editor.selected_indices_normalized()
        } else {
            Vec::new()
        };

        for index in indices {
            if index < self.editor.objects.len() && !next.contains(&index) {
                next.push(index);
            }
        }

        next.sort_unstable();
        next.dedup();

        self.editor.ui.selected_block_indices = next;
        self.sync_primary_selection_from_indices();
        self.editor.ui.hovered_block_index = self.editor.ui.selected_block_index;
        self.editor.runtime.interaction.gizmo_drag = None;
        self.editor.runtime.interaction.block_drag = None;

        if let Some(index) = self.editor.ui.selected_block_index {
            if let Some(object) = self.editor.objects.get(index) {
                self.editor.ui.cursor =
                    [object.position[0], object.position[1], object.position[2]];
            }
        }

        self.editor.selected_mask_cache = None;
        self.mark_editor_dirty(EditorDirtyFlags {
            rebuild_block_mesh: true,
            rebuild_selection_overlays: true,
            rebuild_cursor: self.editor.ui.selected_block_index.is_some(),
            ..EditorDirtyFlags::default()
        });
    }

    pub(crate) fn editor_rename_object_from_explorer(&mut self, index: usize, name: String) {
        if self.phase != AppPhase::Editor || index >= self.editor.objects.len() {
            return;
        }

        let trimmed = name.trim().to_string();
        if self.editor.objects[index].name == trimmed {
            return;
        }

        self.record_editor_history_state();
        self.editor.objects[index].name = trimmed;
    }

    pub(crate) fn editor_create_group_from_selection(&mut self) {
        if self.phase != AppPhase::Editor {
            return;
        }

        let selected_indices = self.editor.selected_indices_normalized();
        if selected_indices.is_empty() {
            return;
        }

        let mut parent_path = self.editor.objects[selected_indices[0]].group_path.clone();
        for index in selected_indices.iter().copied().skip(1) {
            if let Some(object) = self.editor.objects.get(index) {
                while !object.group_path.starts_with(&parent_path) {
                    if parent_path.is_empty() {
                        break;
                    }
                    parent_path.pop();
                }
            }
        }

        let group_name = self.editor.unique_group_name_under(&parent_path);
        self.record_editor_history_state();

        for index in selected_indices {
            if let Some(object) = self.editor.objects.get_mut(index) {
                let mut next_path = parent_path.clone();
                next_path.push(group_name.clone());
                if object.group_path.len() > parent_path.len()
                    && object.group_path.starts_with(&parent_path)
                {
                    next_path.extend_from_slice(&object.group_path[parent_path.len()..]);
                }
                object.group_path = next_path;
            }
        }
    }

    pub(crate) fn editor_rename_group(&mut self, path: Vec<String>, new_name: String) {
        if self.phase != AppPhase::Editor {
            return;
        }

        let target_path = EditorSubsystem::normalized_group_path(&path);
        if target_path.is_empty() {
            return;
        }

        let trimmed = new_name.trim();
        if trimmed.is_empty() {
            return;
        }

        let mut replacement_path = target_path[..target_path.len() - 1].to_vec();
        replacement_path.push(trimmed.to_string());
        if replacement_path == target_path {
            return;
        }

        let mut changed = false;
        for object in &self.editor.objects {
            if object.group_path.starts_with(&target_path) {
                changed = true;
                break;
            }
        }
        if !changed {
            return;
        }

        self.record_editor_history_state();

        for object in &mut self.editor.objects {
            if object.group_path.starts_with(&target_path) {
                let mut next_path = replacement_path.clone();
                next_path.extend_from_slice(&object.group_path[target_path.len()..]);
                object.group_path = next_path;
            }
        }
    }

    pub(crate) fn editor_move_selected_to_group(&mut self, group_path: Option<Vec<String>>) {
        if self.phase != AppPhase::Editor {
            return;
        }

        let selected_indices = self.editor.selected_indices_normalized();
        if selected_indices.is_empty() {
            return;
        }

        let normalized = group_path
            .as_deref()
            .map(EditorSubsystem::normalized_group_path)
            .unwrap_or_default();

        let mut changed = false;
        for index in &selected_indices {
            if self.editor.objects[*index].group_path != normalized {
                changed = true;
                break;
            }
        }
        if !changed {
            return;
        }

        self.record_editor_history_state();
        for index in selected_indices {
            if let Some(object) = self.editor.objects.get_mut(index) {
                object.group_path = normalized.clone();
            }
        }
    }
}
