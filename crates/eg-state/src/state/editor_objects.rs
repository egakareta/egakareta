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
#[allow(clippy::type_complexity)]
pub(crate) fn observe_editor_object_changes_system(
    changed: Query<
        Entity,
        (
            With<EditorAuthoredObject>,
            Or<(
                Added<EditorAuthoredObject>,
                Changed<ObjectData>,
                Changed<EditorObjectOrder>,
            )>,
        ),
    >,
    mut removed: RemovedComponents<EditorAuthoredObject>,
    mut revision: ResMut<EditorObjectRevision>,
) {
    if changed.iter().next().is_some() || removed.read().next().is_some() {
        revision.0 = revision.0.wrapping_add(1);
    }
}

fn authored_object_bundle(order: usize, object: LevelObject) -> impl Bundle {
    let cached_behavior = CachedBlockBehavior::from_block_id_ref(&object.block_id);
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
            cached_behavior,
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
///
/// Borrowed-ID fast path: the lookup maps borrow `&str` IDs from the live
/// query instead of cloning 19k `String` keys, and the reconcile loop
/// iterates `objects` by reference, cloning a `LevelObject` only when an
/// entity is spawned or its data actually changed. Unchanged IDs keep their
/// entities so downstream change detection stays useful.
pub(crate) fn replace_editor_objects_commands(
    commands: &mut Commands,
    existing: &Query<
        (Entity, &ObjectId, &ObjectData, &EditorObjectOrder),
        With<EditorAuthoredObject>,
    >,
    objects: &[LevelObject],
) {
    let mut existing_by_id: HashMap<&str, (Entity, &ObjectData, &EditorObjectOrder)> =
        HashMap::with_capacity(existing.iter().size_hint().0);
    let mut existing_by_order: HashMap<usize, (Entity, &ObjectData, &EditorObjectOrder)> =
        HashMap::new();
    let mut duplicate_entities = Vec::new();
    for (entity, id, data, order) in existing.iter() {
        if id.0.is_empty() {
            existing_by_order.insert(order.0, (entity, data, order));
        } else if let Entry::Vacant(entry) = existing_by_id.entry(id.0.as_str()) {
            entry.insert((entity, data, order));
        } else {
            duplicate_entities.push(entity);
        }
    }

    for (order, object) in objects.iter().enumerate() {
        let existing_object = if object.id.is_empty() {
            existing_by_order.remove(&order)
        } else {
            existing_by_id.remove(object.id.as_str())
        };
        let Some((entity, current, current_order)) = existing_object else {
            spawn_editor_object_commands(commands, order, object.clone());
            continue;
        };

        let mut entity_commands = commands.entity(entity);
        if current.0 != *object {
            entity_commands.insert((
                ObjectData(object.clone()),
                Position(object.position),
                Rotation(object.rotation),
                Size(object.size),
                BlockId(object.block_id.clone()),
                ColorTint(object.color_tint),
                CachedBlockBehavior::from_block_id_ref(&object.block_id),
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
///
/// Fast path: only entity IDs + orders are cloned upfront (no 19k
/// `LevelObject` clones). Each incoming object is compared against the live
/// `ObjectData` via `world.get` and cloned only when spawned or changed.
pub fn replace_editor_objects(world: &mut World, objects: Vec<LevelObject>) {
    let existing: Vec<(Entity, String, usize)> = {
        let mut query = world
            .query_filtered::<(Entity, &ObjectId, &EditorObjectOrder), With<EditorAuthoredObject>>(
            );
        query
            .iter(world)
            .map(|(entity, id, order)| (entity, id.0.clone(), order.0))
            .collect()
    };
    let mut existing_by_id: HashMap<String, (Entity, usize)> =
        HashMap::with_capacity(existing.len());
    let mut existing_by_order: HashMap<usize, (Entity, usize)> = HashMap::new();
    let mut duplicate_entities = Vec::new();
    for (entity, id, order) in existing {
        if id.is_empty() {
            // Empty IDs are order-keyed; keep first occurrence, despawn dupes.
            if let Entry::Vacant(entry) = existing_by_order.entry(order) {
                entry.insert((entity, order));
            } else {
                duplicate_entities.push(entity);
            }
        } else if let Entry::Vacant(entry) = existing_by_id.entry(id) {
            entry.insert((entity, order));
        } else {
            duplicate_entities.push(entity);
        }
    }

    for (order, object) in objects.into_iter().enumerate() {
        let existing_object = if object.id.is_empty() {
            existing_by_order.remove(&order)
        } else {
            existing_by_id.remove(&object.id)
        };
        let Some((entity, current_order)) = existing_object else {
            spawn_editor_object(world, order, object);
            continue;
        };

        let needs_data_update = world
            .get::<ObjectData>(entity)
            .is_none_or(|current| current.0 != object);
        if needs_data_update {
            let cached = CachedBlockBehavior::from_block_id_ref(&object.block_id);
            let mut entity_mut = world.entity_mut(entity);
            entity_mut.insert((
                ObjectData(object.clone()),
                Position(object.position),
                Rotation(object.rotation),
                Size(object.size),
                BlockId(object.block_id.clone()),
                ColorTint(object.color_tint),
                cached,
            ));
        }
        if current_order != order {
            world.entity_mut(entity).insert(EditorObjectOrder(order));
        }
    }

    for entity in duplicate_entities {
        world.despawn(entity);
    }
    for (entity, _) in existing_by_id
        .into_values()
        .chain(existing_by_order.into_values())
    {
        world.despawn(entity);
    }
    if let Some(mut revision) = world.get_resource_mut::<EditorObjectRevision>() {
        revision.0 = revision.0.wrapping_add(1);
    }
}

/// Collect ordered `(order, object)` pairs without sorting when dense.
///
/// Most editor scenes keep `EditorObjectOrder` dense (`0..len`), so placing
/// directly by index avoids the O(n log n) sort on every 19k-object collect.
/// Falls back to an unstable sort only when orders are sparse/out of range.
fn collect_ordered_pairs(
    iter: impl Iterator<Item = (usize, LevelObject)>,
    len_hint: usize,
) -> Vec<LevelObject> {
    let mut placed: Vec<Option<LevelObject>> = Vec::new();
    placed.resize_with(len_hint, || None);
    let mut overflow: Vec<(usize, LevelObject)> = Vec::new();
    for (order, object) in iter {
        if order < placed.len() {
            placed[order] = Some(object);
        } else {
            overflow.push((order, object));
        }
    }
    if !overflow.is_empty() {
        let mut ordered: Vec<(usize, LevelObject)> = placed
            .into_iter()
            .enumerate()
            .filter_map(|(order, object)| object.map(|object| (order, object)))
            .chain(overflow)
            .collect();
        ordered.sort_unstable_by_key(|(order, _)| *order);
        return ordered.into_iter().map(|(_, object)| object).collect();
    }
    // If any dense slot is missing (desync), fall back to sorting what we have.
    if placed.iter().any(|slot| slot.is_none()) {
        let mut ordered: Vec<(usize, LevelObject)> = placed
            .into_iter()
            .enumerate()
            .filter_map(|(order, object)| object.map(|object| (order, object)))
            .collect();
        // Duplicate orders may exceed dense slots; keep stable output by
        // sorting the sparse remainder.
        ordered.sort_unstable_by_key(|(order, _)| *order);
        return ordered.into_iter().map(|(_, object)| object).collect();
    }
    placed
        .into_iter()
        .map(|slot| slot.expect("dense orders must fill every slot"))
        .collect()
}

/// Read authored objects from ECS in stable authoring order.
pub(crate) fn collect_editor_object_data(
    objects: &Query<(&EditorObjectOrder, &ObjectData), With<EditorAuthoredObject>>,
) -> Vec<LevelObject> {
    let len_hint = objects.iter().size_hint().0;
    collect_ordered_pairs(
        objects
            .iter()
            .map(|(order, data)| (order.0, data.0.clone())),
        len_hint,
    )
}

/// Collect authored editor objects in stable authoring order.
#[cfg(test)]
pub(crate) fn collect_editor_objects(world: &mut World) -> Vec<LevelObject> {
    let mut query =
        world.query_filtered::<(&EditorObjectOrder, &ObjectData), With<EditorAuthoredObject>>();
    let len_hint = query.iter(world).size_hint().0;
    collect_ordered_pairs(
        query
            .iter(world)
            .map(|(order, data)| (order.0, data.0.clone())),
        len_hint,
    )
}

/// Collect authored editor objects from the App world in stable authoring order.
pub fn collect_editor_objects_from_world(world: &mut World) -> Vec<LevelObject> {
    let mut query =
        world.query_filtered::<(&EditorObjectOrder, &ObjectData), With<EditorAuthoredObject>>();
    let len_hint = query.iter(world).size_hint().0;
    collect_ordered_pairs(
        query
            .iter(world)
            .map(|(order, data)| (order.0, data.0.clone())),
        len_hint,
    )
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
