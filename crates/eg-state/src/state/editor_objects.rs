/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! ECS vocabulary for authored editor objects.
//!
//! An [`EditorAuthoredObject`] is distinct from a gameplay object: it remains
//! in the App world while the editor is active and carries its stable authored
//! order separately from the world-local entity id. Gameplay playtest setup
//! can therefore query authored objects without making a `Vec<LevelObject>`
//! resource authoritative.

use bevy::ecs::prelude::*;
use eg_game::{
    BlockId, CachedBlockBehavior, ColorTint, Object, ObjectBundle, ObjectData, ObjectId, Position,
    Rotation, Size,
};
use eg_types::LevelObject;

/// Marks an `eg_game::ObjectBundle` entity as an authored editor object.
#[derive(Component, Default)]
pub struct EditorAuthoredObject;

/// Stable authored ordering for editor export, selection, and UI lists.
///
/// Entity identifiers are intentionally not used for this purpose because
/// they are world-local and change when objects are rebuilt.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct EditorObjectOrder(pub usize);

fn authored_object_bundle(order: usize, object: LevelObject) -> impl Bundle {
    (
        ObjectBundle {
            object: Object,
            id: ObjectId(object.id.clone()),
            data: ObjectData(object.clone()),
            position: Position(object.position),
            rotation: Rotation(object.rotation),
            size: Size(object.size),
            block_id: BlockId(object.block_id.clone()),
            color_tint: ColorTint(object.color_tint),
            cached_behavior: CachedBlockBehavior::from_block_id(&object.block_id),
        },
        EditorAuthoredObject,
        EditorObjectOrder(order),
    )
}

/// Spawn one authored editor object with its ECS projection kept in lockstep.
pub(crate) fn spawn_editor_object(world: &mut World, order: usize, object: LevelObject) -> Entity {
    world.spawn(authored_object_bundle(order, object)).id()
}

/// Queue one authored object from a normal ECS system.
pub(crate) fn spawn_editor_object_commands(
    commands: &mut Commands,
    order: usize,
    object: LevelObject,
) {
    commands.spawn(authored_object_bundle(order, object));
}

/// Replace the ordered authored entity set from a normal ECS command handler.
pub(crate) fn replace_editor_objects_commands(
    commands: &mut Commands,
    existing: &Query<Entity, With<EditorAuthoredObject>>,
    objects: &[LevelObject],
) {
    for entity in existing.iter() {
        commands.entity(entity).despawn();
    }
    for (order, object) in objects.iter().cloned().enumerate() {
        spawn_editor_object_commands(commands, order, object);
    }
}

/// Replace the authored editor-object entity set from ordered level data.
///
/// This is deliberately an exclusive-world helper: structural changes are
/// applied immediately and a later same-frame query observes the complete
/// replacement without relying on a vector mirror.
pub fn replace_editor_objects(world: &mut World, objects: Vec<LevelObject>) {
    let entities = {
        let mut query = world.query_filtered::<Entity, With<EditorAuthoredObject>>();
        query.iter(world).collect::<Vec<_>>()
    };
    for entity in entities {
        world.despawn(entity);
    }
    for (order, object) in objects.iter().cloned().enumerate() {
        spawn_editor_object(world, order, object);
    }
}

/// Read authored objects from ECS in stable authoring order.
pub(crate) fn collect_editor_object_data(
    objects: &Query<(&EditorObjectOrder, &ObjectData), With<EditorAuthoredObject>>,
) -> Vec<LevelObject> {
    let mut ordered = objects
        .iter()
        .map(|(order, data)| (order.0, data.0.clone()))
        .collect::<Vec<_>>();
    ordered.sort_by_key(|(order, _)| *order);
    ordered.into_iter().map(|(_, object)| object).collect()
}

/// Collect authored editor objects in stable authoring order.
#[cfg(test)]
pub(crate) fn collect_editor_objects(world: &mut World) -> Vec<LevelObject> {
    let mut query =
        world.query_filtered::<(&EditorObjectOrder, &ObjectData), With<EditorAuthoredObject>>();
    let mut objects = query
        .iter(world)
        .map(|(order, data)| (order.0, data.0.clone()))
        .collect::<Vec<_>>();
    objects.sort_by_key(|(order, _)| *order);
    objects.into_iter().map(|(_, object)| object).collect()
}

/// Collect authored editor objects from the App world in stable authoring order.
pub fn collect_editor_objects_from_world(world: &mut World) -> Vec<LevelObject> {
    let mut query =
        world.query_filtered::<(&EditorObjectOrder, &ObjectData), With<EditorAuthoredObject>>();
    let mut objects = query
        .iter(world)
        .map(|(order, data)| (order.0, data.0.clone()))
        .collect::<Vec<_>>();
    objects.sort_by_key(|(order, _)| *order);
    objects.into_iter().map(|(_, object)| object).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authored_objects_are_entities_with_stable_export_order() {
        let mut world = World::new();
        let first = LevelObject {
            id: "first".to_string(),
            ..LevelObject::new("core/stone")
        };
        let second = LevelObject {
            id: "second".to_string(),
            ..LevelObject::new("core/gem")
        };

        spawn_editor_object(&mut world, 1, second.clone());
        spawn_editor_object(&mut world, 0, first.clone());

        assert_eq!(collect_editor_objects(&mut world), vec![first, second]);
    }
}
