/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
use eg_types::LevelObject;

super::editor_context_methods! {
    fn normalized_block_selection_indices(&self) -> Vec<usize> {
        let object_count = self.objects.len();
        let mut indices: Vec<usize> = self
            .input
            .selected_block_indices
            .iter()
            .copied()
            .filter(|index| *index < object_count)
            .collect();

        if let Some(index) = self
            .input
            .selected_block_index
            .filter(|index| *index < object_count)
        {
            indices.push(index);
        }

        indices.sort_unstable();
        indices.dedup();
        indices
    }

    pub fn selected_indices_normalized(&self) -> Vec<usize> {
        self.normalized_block_selection_indices()
    }

    pub fn normalize_block_selection(&mut self) {
        let indices = self.normalized_block_selection_indices();
        self.input.selected_block_index = self
            .input
            .selected_block_index
            .filter(|index| indices.binary_search(index).is_ok())
            .or_else(|| indices.first().copied());
        self.input.selected_block_indices = indices;

        if self
            .input
            .hovered_block_index
            .is_some_and(|index| index >= self.objects.len())
        {
            self.input.hovered_block_index = None;
        }
        self.selected_mask_cache = None;
    }

    pub fn clear_block_selection(&mut self) {
        let had_selection = self.input.selected_block_index.is_some()
            || !self.input.selected_block_indices.is_empty()
            || self.input.hovered_block_index.is_some();
        self.input.selected_block_index = None;
        self.input.selected_block_indices.clear();
        self.input.hovered_block_index = None;
        if had_selection {
            self.selected_mask_cache = None;
        }
    }

    pub fn replace_block_selection(&mut self, indices: Vec<usize>) {
        self.input.selected_block_index = indices.first().copied();
        self.input.selected_block_indices = indices;
        self.normalize_block_selection();
    }

    pub fn add_block_to_selection(&mut self, index: usize) {
        self.input.selected_block_indices.push(index);
        self.normalize_block_selection();
    }

    pub fn remove_block_from_selection(&mut self, index: usize) {
        self.input
            .selected_block_indices
            .retain(|selected| *selected != index);
        if self.input.selected_block_index == Some(index) {
            self.input.selected_block_index = None;
        }
        self.normalize_block_selection();
        if self.input.selected_block_indices.is_empty() {
            self.clear_block_selection();
        }
    }

    pub fn sync_primary_selection_from_indices(&mut self) {
        self.normalize_block_selection();
    }

    pub fn selection_contains(&self, index: usize) -> bool {
        self.input.selected_block_indices.contains(&index)
            || self.input.selected_block_index == Some(index)
    }

    pub fn selected_mask_for_len(&self, len: usize) -> Vec<bool> {
        let mut selected_mask = vec![false; len];
        for index in self.selected_indices_normalized() {
            if index < len {
                selected_mask[index] = true;
            }
        }
        selected_mask
    }

    pub fn selected_group_bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        let indices = self.selected_indices_normalized();
        if indices.is_empty() {
            return self
                .selected_transform_trigger_target()
                .map(|(_, target)| (target.position, target.size));
        }

        let first = *indices.first()?;
        let first_obj = self.objects.get(first)?;
        let mut min = first_obj.position;
        let mut max = [
            first_obj.position[0] + first_obj.size[0],
            first_obj.position[1] + first_obj.size[1],
            first_obj.position[2] + first_obj.size[2],
        ];

        for index in indices.into_iter().skip(1) {
            if let Some(obj) = self.objects.get(index) {
                min[0] = min[0].min(obj.position[0]);
                min[1] = min[1].min(obj.position[1]);
                min[2] = min[2].min(obj.position[2]);
                max[0] = max[0].max(obj.position[0] + obj.size[0]);
                max[1] = max[1].max(obj.position[1] + obj.size[1]);
                max[2] = max[2].max(obj.position[2] + obj.size[2]);
            }
        }

        Some((min, [max[0] - min[0], max[1] - min[1], max[2] - min[2]]))
    }

    pub fn selected_transform_trigger_target(&self) -> Option<(usize, LevelObject)> {
        let index = self.selected_trigger_index()?;
        let object = self.trigger_objects().get(index)?.clone();
        if !object.is_transform_trigger() {
            return None;
        }
        Some((index, object))
    }

    pub fn set_transform_trigger_target(
        &mut self,
        trigger_index: usize,
        position: [f32; 3],
        size: [f32; 3],
        rotation: [f32; 3],
    ) -> bool {
        // Find the trigger object by scanning objects for the Nth trigger
        let mut found = 0usize;
        let mut object_index = None;
        for (idx, obj) in self.objects.iter().enumerate() {
            if obj.trigger.is_some() {
                if found == trigger_index {
                    object_index = Some(idx);
                    break;
                }
                found += 1;
            }
        }
        let Some(obj_index) = object_index else {
            return false;
        };
        let Some(obj) = self.objects.get_mut(obj_index) else {
            return false;
        };

        let clamped_position = [position[0], position[1].max(0.0), position[2]];
        obj.position = clamped_position;
        obj.size = [size[0].max(0.01), size[1].max(0.01), size[2].max(0.01)];
        obj.rotation = rotation;

        self.input.cursor = [
            clamped_position[0],
            clamped_position[1].max(0.0),
            clamped_position[2],
        ];
        true
    }
}

#[cfg(test)]
mod tests {
    use eg_types::LevelObject;

    use crate::state::{
        EditorInputRes, replace_editor_objects, test_support::TestApp,
        with_editor_interaction_context,
    };

    fn block(position: [f32; 3]) -> LevelObject {
        LevelObject {
            id: format!(
                "object_{:.0}_{:.0}_{:.0}",
                position[0], position[1], position[2]
            ),
            position,
            ..LevelObject::default()
        }
    }

    #[test]
    fn normalize_block_selection_filters_invalid_indices_and_preserves_valid_primary() {
        let mut app = TestApp::minimal();
        replace_editor_objects(
            app.world_mut(),
            vec![
                block([0.0, 0.0, 0.0]),
                block([1.0, 0.0, 0.0]),
                block([2.0, 0.0, 0.0]),
            ],
        );
        with_editor_interaction_context(app.world_mut(), |editor| {
            editor.input.selected_block_index = Some(1);
            editor.input.selected_block_indices = vec![2, 99, 2];
            editor.input.hovered_block_index = Some(42);
            editor.selected_mask_cache = Some(vec![false, true, true]);

            editor.normalize_block_selection();
            assert!(editor.selected_mask_cache.is_none());
        });

        let input = &app.world().resource::<EditorInputRes>().0;
        assert_eq!(input.selected_block_index, Some(1));
        assert_eq!(input.selected_block_indices, vec![1, 2]);
        assert_eq!(input.hovered_block_index, None);
    }
}
