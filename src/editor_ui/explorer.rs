/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERICAL.md for details.

*/
use std::collections::BTreeSet;

use crate::block_repository::resolve_block_definition;
use crate::commands::AppCommand;
use crate::state::EditorUiViewModel;

fn collect_child_groups(view: &EditorUiViewModel<'_>, parent_path: &[String]) -> Vec<String> {
    let mut groups = BTreeSet::new();
    for object in view.objects {
        if object.group_path.len() <= parent_path.len() {
            continue;
        }
        if !object.group_path.starts_with(parent_path) {
            continue;
        }
        groups.insert(object.group_path[parent_path.len()].clone());
    }
    groups.into_iter().collect()
}

fn collect_object_indices(view: &EditorUiViewModel<'_>, parent_path: &[String]) -> Vec<usize> {
    let mut indices = Vec::new();
    for (index, object) in view.objects.iter().enumerate() {
        if object.group_path == parent_path {
            indices.push(index);
        }
    }
    indices
}

fn object_label(view: &EditorUiViewModel<'_>, index: usize) -> String {
    let object = &view.objects[index];
    let trimmed = object.name.trim();
    if !trimmed.is_empty() {
        return trimmed.to_string();
    }

    let base_name = resolve_block_definition(&object.block_id)
        .display_name
        .trim();
    let base_name = if base_name.is_empty() {
        "Block"
    } else {
        base_name
    };
    format!("{base_name} {}", index + 1)
}

fn render_object_row(
    ui: &mut egui::Ui,
    view: &EditorUiViewModel<'_>,
    index: usize,
    commands: &mut Vec<AppCommand>,
) {
    let is_selected = view.selected_object_indices.contains(&index);
    let label = object_label(view, index);
    let subtitle = view.objects[index].block_id.as_str();

    ui.horizontal(|ui| {
        let response = ui.selectable_label(is_selected, label);
        ui.label(egui::RichText::new(subtitle).small().weak());

        if response.clicked() {
            let additive = ui.input(|input| input.modifiers.shift || input.modifiers.ctrl);
            commands.push(AppCommand::EditorExplorerSelectObjects {
                indices: vec![index],
                additive,
            });
        }
    });

    if is_selected {
        let mut object_name = view.objects[index].name.clone();
        ui.horizontal(|ui| {
            ui.add_space(14.0);
            ui.label("Name:");
            if ui.text_edit_singleline(&mut object_name).changed() {
                commands.push(AppCommand::EditorExplorerRenameObject {
                    index,
                    name: object_name,
                });
            }
        });
    }
}

fn render_group(
    ui: &mut egui::Ui,
    view: &EditorUiViewModel<'_>,
    parent_path: &[String],
    commands: &mut Vec<AppCommand>,
) {
    let object_indices = collect_object_indices(view, parent_path);
    for object_index in object_indices {
        render_object_row(ui, view, object_index, commands);
    }

    let child_groups = collect_child_groups(view, parent_path);
    for group_name in child_groups {
        let mut group_path = parent_path.to_vec();
        group_path.push(group_name.clone());

        let header_text = format!("Folder: {}", group_name);
        egui::CollapsingHeader::new(header_text)
            .id_salt(format!("explorer_group:{}", group_path.join("/")))
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let mut edited_group_name = group_name.clone();
                    ui.label("Rename:");
                    if ui.text_edit_singleline(&mut edited_group_name).changed() {
                        commands.push(AppCommand::EditorExplorerRenameGroup {
                            path: group_path.clone(),
                            new_name: edited_group_name,
                        });
                    }

                    if ui.button("Move Selected Here").clicked() {
                        commands.push(AppCommand::EditorExplorerMoveSelectedToGroup(Some(
                            group_path.clone(),
                        )));
                    }
                });

                render_group(ui, view, &group_path, commands);
            });
    }
}

pub(crate) fn show_explorer_panel(
    ctx: &egui::Context,
    view: &EditorUiViewModel<'_>,
    commands: &mut Vec<AppCommand>,
) {
    if !view.show_explorer {
        return;
    }

    egui::SidePanel::right("editor_explorer_sidebar")
        .resizable(true)
        .default_width(320.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Explorer");
                if ui.button(egui_phosphor::regular::X).clicked() {
                    commands.push(AppCommand::EditorSetShowExplorer(false));
                }
            });

            ui.separator();
            ui.label(format!(
                "Objects: {} | Selected: {}",
                view.objects.len(),
                view.selected_object_indices.len()
            ));

            ui.horizontal(|ui| {
                if ui.button("Group Selected").clicked() {
                    commands.push(AppCommand::EditorExplorerCreateGroupFromSelection);
                }

                if ui.button("Move Selected To Root").clicked() {
                    commands.push(AppCommand::EditorExplorerMoveSelectedToGroup(None));
                }
            });

            ui.separator();

            if view.objects.is_empty() {
                ui.label("No blocks placed yet.");
                return;
            }

            egui::ScrollArea::vertical()
                .id_salt("explorer_scroll")
                .show(ui, |ui| {
                    render_group(ui, view, &[], commands);
                });
        });
}
