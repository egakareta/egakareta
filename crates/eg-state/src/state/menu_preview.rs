/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! App-world menu preview selection and metadata extraction.

use bevy_ecs::prelude::*;
use bevy_state::prelude::State;
use eg_types::{
    AppPhase, DEFAULT_MENU_PREVIEW_CAMERA_POSITION, DEFAULT_MENU_PREVIEW_CAMERA_TARGET,
    LevelMetadata,
};

use super::{
    BuiltinLevelCatalogRes, MenuPreviewObjectsRes, MenuStateRes,
    level_management::menu_preview_camera_for_metadata,
};

/// Refresh the selected built-in level used by the menu preview.
///
/// The scheduled resource is authoritative for preview selection, camera,
/// sky color, and objects. Render extraction consumes the objects and emits
/// the cached preview mesh for the compatibility presenter.
pub(crate) fn refresh_menu_preview_system(
    phase: Res<State<AppPhase>>,
    catalog: Res<BuiltinLevelCatalogRes>,
    menu: Option<ResMut<MenuStateRes>>,
    preview_objects: Option<ResMut<MenuPreviewObjectsRes>>,
) {
    let Some(mut menu) = menu else {
        return;
    };
    let Some(mut preview_objects) = preview_objects else {
        return;
    };

    if *phase.get() != AppPhase::Menu || menu.0.preview_level_index == Some(menu.0.selected_level) {
        return;
    }

    menu.0.preview_level_index = Some(menu.0.selected_level);

    let metadata = menu
        .0
        .levels
        .get(menu.0.selected_level)
        .and_then(|level_name| catalog.get(level_name));

    let (objects, sky_color, camera_position, camera_target) = match metadata {
        Some(metadata) => {
            let (camera_position, camera_target) = menu_preview_camera_for_metadata(&metadata);
            (
                metadata.objects,
                metadata.sky_color,
                camera_position,
                camera_target,
            )
        }
        None => (
            Vec::new(),
            LevelMetadata::default().sky_color,
            DEFAULT_MENU_PREVIEW_CAMERA_POSITION,
            DEFAULT_MENU_PREVIEW_CAMERA_TARGET,
        ),
    };

    preview_objects.objects = objects;
    menu.0.preview_sky_color = sky_color;
    menu.0.preview_camera_position = camera_position;
    menu.0.preview_camera_target = camera_target;
}
