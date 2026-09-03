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

use std::collections::{HashMap, hash_map::Entry};

use bevy_ecs::prelude::*;
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

/// Monotonic revision of the authored editor-object set.
#[derive(Resource, Default)]
pub struct EditorObjectRevision(pub u64);

/// Observe authored-object lifecycle changes after deferred command application.
pub(crate) fn observe_editor_object_changes_system(
    added: Query<Entity, (Added<ObjectData>, With<EditorAuthoredObject>)>,
    changed: Query<Entity, (Changed<ObjectData>, With<EditorAuthoredObject>)>,
    reordered: Query<Entity, (Changed<EditorObjectOrder>, With<EditorAuthoredObject>)>,
    mut removed: RemovedComponents<EditorAuthoredObject>,
    mut revision: ResMut<EditorObjectRevision>,
) {
    if added.iter().next().is_some()
        || changed.iter().next().is_some()
        || reordered.iter().next().is_some()
        || removed.read().next().is_some()
    {
        revision.0 = revision.0.wrapping_add(1);
    }
}

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
    existing: &Query<
        (Entity, &ObjectId, &ObjectData, &EditorObjectOrder),
        With<EditorAuthoredObject>,
    >,
    objects: &[LevelObject],
) {
    let mut existing_by_id = HashMap::new();
    let mut existing_by_order = HashMap::new();
    let mut duplicate_entities = Vec::new();
    for (entity, id, data, order) in existing.iter() {
        if id.0.is_empty() {
            existing_by_order.insert(order.0, (entity, data, order));
        } else if let Entry::Vacant(entry) = existing_by_id.entry(id.0.clone()) {
            entry.insert((entity, data, order));
        } else {
            duplicate_entities.push(entity);
        }
    }

    for (order, object) in objects.iter().cloned().enumerate() {
        let existing_object = if object.id.is_empty() {
            existing_by_order.remove(&order)
        } else {
            existing_by_id.remove(&object.id)
        };
        let Some((entity, current, current_order)) = existing_object else {
            spawn_editor_object_commands(commands, order, object);
            continue;
        };

        let mut entity_commands = commands.entity(entity);
        if current.0 != object {
            entity_commands.insert((
                ObjectData(object.clone()),
                Position(object.position),
                Rotation(object.rotation),
                Size(object.size),
                BlockId(object.block_id.clone()),
                ColorTint(object.color_tint),
                CachedBlockBehavior::from_block_id(&object.block_id),
            ));
        }
        if current_order.0 != order {
            entity_commands.insert(EditorObjectOrder(order));
        }
    }

    for entity in duplicate_entities {
        commands.entity(entity).despawn();
    }
    for (entity, _, _) in existing_by_id
        .into_values()
        .chain(existing_by_order.into_values())
    {
        commands.entity(entity).despawn();
    }
}

/// Reconcile the authored editor-object entity set from ordered level data.
///
/// Unchanged IDs retain their entities. This keeps ECS change detection useful
/// to render extraction and avoids invalidating downstream caches merely
/// because an editor command produced a new ordered vector.
pub fn replace_editor_objects(world: &mut World, objects: Vec<LevelObject>) {
    let existing = {
        let mut query = world.query_filtered::<
            (Entity, &ObjectId, &ObjectData, &EditorObjectOrder),
            With<EditorAuthoredObject>,
        >();
        query
            .iter(world)
            .map(|(entity, id, data, order)| (entity, id.0.clone(), data.0.clone(), order.0))
            .collect::<Vec<_>>()
    };
    let mut existing_by_id = HashMap::new();
    let mut existing_by_order = HashMap::new();
    let mut duplicate_entities = Vec::new();
    for (entity, id, data, order) in existing {
        if id.is_empty() {
            existing_by_order.insert(order, (entity, data, order));
        } else if let Entry::Vacant(entry) = existing_by_id.entry(id) {
            entry.insert((entity, data, order));
        } else {
            duplicate_entities.push(entity);
        }
    }

    for (order, object) in objects.iter().cloned().enumerate() {
        let existing_object = if object.id.is_empty() {
            existing_by_order.remove(&order)
        } else {
            existing_by_id.remove(&object.id)
        };
        let Some((entity, current, current_order)) = existing_object else {
            spawn_editor_object(world, order, object);
            continue;
        };

        let mut entity_mut = world.entity_mut(entity);
        if current != object {
            entity_mut.insert((
                ObjectData(object.clone()),
                Position(object.position),
                Rotation(object.rotation),
                Size(object.size),
                BlockId(object.block_id.clone()),
                ColorTint(object.color_tint),
                CachedBlockBehavior::from_block_id(&object.block_id),
            ));
        }
        if current_order != order {
            entity_mut.insert(EditorObjectOrder(order));
        }
    }

    for entity in duplicate_entities {
        world.despawn(entity);
    }
    for (entity, _, _) in existing_by_id
        .into_values()
        .chain(existing_by_order.into_values())
    {
        world.despawn(entity);
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

    #[test]
    fn reconciliation_retains_entities_for_unchanged_ids() {
        let mut world = World::new();
        let first = LevelObject {
            id: "first".to_string(),
            ..LevelObject::new("core/stone")
        };
        let second = LevelObject {
            id: "second".to_string(),
            ..LevelObject::new("core/gem")
        };
        spawn_editor_object(&mut world, 0, first.clone());
        spawn_editor_object(&mut world, 1, second.clone());

        let first_entity = {
            let mut query = world.query::<(Entity, &ObjectId)>();
            query
                .iter(&world)
                .find(|(_, id)| id.0 == "first")
                .map(|(entity, _)| entity)
                .expect("first entity")
        };

        let updated_first = LevelObject {
            position: [4.0, 0.0, 0.0],
            ..first
        };
        replace_editor_objects(&mut world, vec![second, updated_first]);

        assert!(world.get_entity(first_entity).is_ok());
        assert_eq!(
            collect_editor_objects(&mut world)[1].position,
            [4.0, 0.0, 0.0]
        );
    }

    #[test]
    fn reconciliation_drops_duplicate_existing_ids_without_dropping_reused_entity() {
        let mut world = World::new();
        spawn_editor_object(
            &mut world,
            0,
            LevelObject {
                id: "shared".to_string(),
                position: [1.0, 0.0, 0.0],
                ..LevelObject::new("core/stone")
            },
        );
        spawn_editor_object(
            &mut world,
            1,
            LevelObject {
                id: "shared".to_string(),
                position: [2.0, 0.0, 0.0],
                ..LevelObject::new("core/stone")
            },
        );

        replace_editor_objects(
            &mut world,
            vec![LevelObject {
                id: "shared".to_string(),
                position: [3.0, 0.0, 0.0],
                ..LevelObject::new("core/stone")
            }],
        );

        assert_eq!(collect_editor_objects(&mut world).len(), 1);
        assert_eq!(
            collect_editor_objects(&mut world)[0].position,
            [3.0, 0.0, 0.0]
        );
    }
}
