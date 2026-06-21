/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/

#[cfg(test)]
mod tests {
    use eg_mesh::Vertex;
    use eg_types::{
        apply_timed_triggers_to_objects, camera_trigger_eye_from_object,
        camera_trigger_eye_from_target, camera_trigger_forward,
        camera_trigger_forward_from_rotation_degrees,
        camera_trigger_rotation_pitch_from_rotation_degrees, camera_trigger_target_from_eye,
        camera_triggers_to_timed_triggers, default_camera_trigger_distance,
        default_camera_trigger_pitch, default_camera_trigger_rotation,
        default_camera_trigger_transition_interval_seconds, timed_triggers_to_camera_triggers,
        triggers_from_objects, CameraTrigger, CameraTriggerMode, EditorStateParams, GameCursor,
        LevelCreatorMetadata, LevelMetadata, LevelObject, LevelPreviewCameraMetadata,
        MusicMetadata, SpawnDirection, SpawnMetadata, TimedTrigger, TimedTriggerAction,
        TimedTriggerEasing, TimedTriggerTarget, CAMERA_TRIGGER_BLOCK_ID,
        TRANSFORM_TRIGGER_BLOCK_ID,
    };
    use serde_json::json;

    /// Asserts that two f32 values are approximately equal within 1e-5 epsilon.
    fn assert_approx_eq(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= 1e-5,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn game_cursor_reports_when_system_cursor_should_be_hidden() {
        assert!(!GameCursor::Default.hides_system_cursor());
        assert!(GameCursor::Hidden.hides_system_cursor());
    }

    #[test]
    fn level_object_rotation_defaults_when_missing() {
        let json = r#"{
            "position":[1.0,2.0,3.0],
            "size":[4.0,5.0,6.0],
            "block_id":"core/stone"
        }"#;

        let object: LevelObject = serde_json::from_str(json).expect("valid level object");
        assert_eq!(object.rotation_degrees, [0.0, 0.0, 0.0]);
        assert_eq!(object.block_id, "core/stone");
        assert_eq!(object.color_tint, [1.0, 1.0, 1.0]);
    }

    #[test]
    fn level_metadata_parses_objects_without_rotation_field() {
        let json = r#"{
            "name":"Current",
            "music":{"source":"music.mp3"},
            "spawn":{"position":[0.0,0.0,0.0],"direction":"forward"},
            "objects":[
                {"position":[0.0,0.0,0.0],"size":[1.0,1.0,1.0],"block_id":"core/grass"}
            ]
        }"#;

        let metadata: LevelMetadata = serde_json::from_str(json).expect("valid metadata");
        assert_eq!(metadata.objects.len(), 1);
        assert_eq!(metadata.objects[0].rotation_degrees, [0.0, 0.0, 0.0]);
        assert!(matches!(metadata.spawn.direction, SpawnDirection::Forward));
    }

    #[test]
    fn level_object_serialization_omits_default_rotation() {
        let object = LevelObject {
            position: [1.0, 2.0, 3.0],
            size: [4.0, 5.0, 6.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            block_id: "core/grass".to_string(),
            color_tint: [1.0, 1.0, 1.0],
            trigger: None,
            ..LevelObject::default()
        };

        let value = serde_json::to_value(&object).expect("serialize object");
        let expected = json!({
            "position": [1.0, 2.0, 3.0],
            "size": [4.0, 5.0, 6.0],
            "block_id": "core/grass"
        });

        assert_eq!(value, expected);
    }

    #[test]
    fn level_object_is_transform_trigger_matches_block_id_or_trigger_action() {
        // A plain block is not a transform trigger.
        let plain = LevelObject {
            position: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            block_id: "core/grass".to_string(),
            color_tint: [1.0, 1.0, 1.0],
            trigger: None,
            ..LevelObject::default()
        };
        assert!(!plain.is_transform_trigger());

        // A transform trigger block (matched by block_id) is detected even
        // without a stored trigger.
        let mut by_block_id = plain.clone();
        by_block_id.block_id = TRANSFORM_TRIGGER_BLOCK_ID.to_string();
        assert!(by_block_id.is_transform_trigger());

        // A non-transform-trigger block carrying a TransformObjects action is
        // also detected (covers deserialized levels that omit block_id).
        let by_action = LevelObject {
            id: "trigger".to_string(),
            position: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            block_id: "core/grass".to_string(),
            color_tint: [1.0, 1.0, 1.0],
            trigger: Some(TimedTrigger {
                time_seconds: 1.0,
                duration_seconds: 1.0,
                easing: TimedTriggerEasing::EaseInOut,
                target: TimedTriggerTarget::Objects {
                    object_ids: vec!["target".to_string()],
                },
                action: TimedTriggerAction::TransformObjects {
                    position: [2.0, 0.0, 0.0],
                    rotation_degrees: [0.0, 0.0, 0.0],
                    size: [1.0, 1.0, 1.0],
                },
            }),
            ..LevelObject::default()
        };
        assert!(by_action.is_transform_trigger());

        // A non-TransformObjects trigger (e.g. camera) should not be detected.
        let camera_block = LevelObject {
            block_id: "core/camera_trigger".to_string(),
            trigger: Some(TimedTrigger {
                time_seconds: 0.0,
                duration_seconds: 0.0,
                easing: TimedTriggerEasing::EaseInOut,
                target: TimedTriggerTarget::Camera,
                action: TimedTriggerAction::CameraPose {
                    transition_interval_seconds: 1.0,
                    use_full_segment_transition: false,
                    target_position: [0.0, 0.0, 0.0],
                    rotation: 0.0,
                    pitch: 0.0,
                },
            }),
            ..plain.clone()
        };
        assert!(!camera_block.is_transform_trigger());
    }

    #[test]
    fn level_metadata_serialization_omits_default_fields() {
        let metadata = LevelMetadata::from_editor_state(EditorStateParams {
            name: "Minimal".to_string(),
            creator_metadata: LevelCreatorMetadata::default(),
            music: MusicMetadata::default(),
            spawn: SpawnMetadata::default(),
            sky_color: eg_types::default_sky_color(),
            tap_times: Vec::new(),
            timing_points: Vec::new(),
            timeline_time_seconds: 0.0,
            timeline_duration_seconds: 16.0,
            simulate_trigger_hitboxes: false,
            menu_preview_camera: None,
            objects: vec![LevelObject {
                position: [0.0, 0.0, 0.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: [0.0, 0.0, 0.0],
                block_id: "core/stone".to_string(),
                color_tint: [1.0, 1.0, 1.0],
                trigger: None,
                ..LevelObject::default()
            }],
        });

        let value = serde_json::to_value(&metadata).expect("serialize metadata");
        assert_eq!(value["name"], "Minimal");
        assert!(value.get("format_version").is_none());
        assert!(value.get("creator").is_none());
        assert!(value.get("description").is_none());
        assert!(value.get("stars").is_none());
        assert!(value.get("tags").is_none());
        assert!(value.get("version").is_none());
        assert!(value.get("music").is_none());
        assert!(value.get("spawn").is_none());
        assert!(value.get("sky_color").is_none());
        assert!(value.get("tap_times").is_none());
        assert!(value.get("timeline_time_seconds").is_none());
        assert!(value.get("timeline_duration_seconds").is_none());
        assert!(value.get("simulate_trigger_hitboxes").is_none());
        assert!(value.get("camera_triggers").is_none());
        assert!(value.get("taps").is_none());
        assert!(value.get("timeline_step").is_none());
        assert_eq!(value["objects"].as_array().map(|v| v.len()), Some(1));
        let object = &value["objects"][0];
        assert!(object.get("position").is_none());
        assert!(object.get("size").is_none());
        assert!(object.get("rotation_degrees").is_none());
        assert!(object.get("block_id").is_none());
    }

    #[test]
    fn level_metadata_serialization_includes_simulate_trigger_hitboxes_when_enabled() {
        let metadata = LevelMetadata::from_editor_state(EditorStateParams {
            name: "HitboxPolicy".to_string(),
            creator_metadata: LevelCreatorMetadata::default(),
            music: MusicMetadata::default(),
            spawn: SpawnMetadata::default(),
            sky_color: eg_types::default_sky_color(),
            tap_times: Vec::new(),
            timing_points: Vec::new(),
            timeline_time_seconds: 0.0,
            timeline_duration_seconds: 16.0,
            simulate_trigger_hitboxes: true,
            menu_preview_camera: None,
            objects: Vec::new(),
        });

        let value = serde_json::to_value(&metadata).expect("serialize metadata");
        assert_eq!(value.get("simulate_trigger_hitboxes"), Some(&json!(true)));
    }

    #[test]
    fn level_metadata_serialization_includes_custom_sky_color() {
        let metadata = LevelMetadata::from_editor_state(EditorStateParams {
            name: "CustomSky".to_string(),
            creator_metadata: LevelCreatorMetadata::default(),
            music: MusicMetadata::default(),
            spawn: SpawnMetadata::default(),
            sky_color: [0.2, 0.4, 0.8],
            tap_times: Vec::new(),
            timing_points: Vec::new(),
            timeline_time_seconds: 0.0,
            timeline_duration_seconds: 16.0,
            simulate_trigger_hitboxes: false,
            menu_preview_camera: None,
            objects: Vec::new(),
        });

        let value = serde_json::to_value(&metadata).expect("serialize metadata");
        let sky_color = value["sky_color"].as_array().expect("sky_color array");
        assert_approx_eq(sky_color[0].as_f64().expect("sky red") as f32, 0.2);
        assert_approx_eq(sky_color[1].as_f64().expect("sky green") as f32, 0.4);
        assert_approx_eq(sky_color[2].as_f64().expect("sky blue") as f32, 0.8);
    }

    #[test]
    fn level_metadata_serialization_includes_menu_preview_camera_when_present() {
        let metadata = LevelMetadata::from_editor_state(EditorStateParams {
            name: "MenuPreview".to_string(),
            creator_metadata: LevelCreatorMetadata::default(),
            music: MusicMetadata::default(),
            spawn: SpawnMetadata::default(),
            sky_color: eg_types::default_sky_color(),
            tap_times: Vec::new(),
            timing_points: Vec::new(),
            timeline_time_seconds: 0.0,
            timeline_duration_seconds: 16.0,
            simulate_trigger_hitboxes: false,
            menu_preview_camera: Some(LevelPreviewCameraMetadata {
                position: [4.0, 8.0, -3.0],
                target: [0.0, 1.0, 0.0],
            }),
            objects: Vec::new(),
        });

        let value = serde_json::to_value(&metadata).expect("serialize metadata");
        assert_eq!(
            value.get("menu_preview_camera"),
            Some(&json!({
                "position": [4.0, 8.0, -3.0],
                "target": [0.0, 1.0, 0.0]
            }))
        );
    }

    #[test]
    fn level_preview_camera_metadata_defaults_missing_vectors_and_omits_zero_vectors() {
        let camera: LevelPreviewCameraMetadata =
            serde_json::from_value(json!({})).expect("deserialize default preview camera");
        assert_eq!(camera.position, [0.0, 0.0, 0.0]);
        assert_eq!(camera.target, [0.0, 0.0, 0.0]);

        let value = serde_json::to_value(&camera).expect("serialize default preview camera");
        assert_eq!(value, json!({}));
    }

    #[test]
    fn camera_trigger_serialization_omits_default_pose_fields() {
        let camera_trigger = CameraTrigger {
            time_seconds: 2.5,
            mode: CameraTriggerMode::Follow,
            easing: TimedTriggerEasing::EaseInOut,
            transition_interval_seconds: default_camera_trigger_transition_interval_seconds(),
            use_full_segment_transition: false,
            target_position: [0.0, 0.0, 0.0],
            distance: default_camera_trigger_distance(),
            rotation: default_camera_trigger_rotation(),
            pitch: default_camera_trigger_pitch(),
        };

        let value = serde_json::to_value(&camera_trigger).expect("serialize camera trigger");
        let expected = json!({
            "time_seconds": 2.5,
            "mode": "follow",
            "easing": "ease_in_out"
        });

        assert_eq!(value, expected);
    }

    #[test]
    fn camera_follow_trigger_defaults_missing_orbit_fields() {
        let trigger: TimedTrigger = serde_json::from_value(json!({
            "time_seconds": 1.25,
            "action": {
                "kind": "camera_follow"
            }
        }))
        .expect("deserialize camera follow trigger");

        match trigger.action {
            TimedTriggerAction::CameraFollow {
                transition_interval_seconds,
                use_full_segment_transition,
                distance,
                rotation,
                pitch,
            } => {
                assert_eq!(transition_interval_seconds, 1.0);
                assert!(!use_full_segment_transition);
                assert_eq!(distance, default_camera_trigger_distance());
                assert_eq!(rotation, default_camera_trigger_rotation());
                assert_eq!(pitch, default_camera_trigger_pitch());
            }
            _ => panic!("expected camera follow action"),
        }
    }

    #[test]
    fn converts_camera_triggers_to_triggers_and_back() {
        let camera_triggers = vec![
            CameraTrigger {
                time_seconds: 0.5,
                mode: CameraTriggerMode::Follow,
                easing: TimedTriggerEasing::EaseIn,
                transition_interval_seconds: 0.4,
                use_full_segment_transition: true,
                target_position: [0.0, 0.0, 0.0],
                distance: 18.0,
                rotation: default_camera_trigger_rotation(),
                pitch: default_camera_trigger_pitch(),
            },
            CameraTrigger {
                time_seconds: 1.0,
                mode: CameraTriggerMode::Static,
                easing: TimedTriggerEasing::EaseOut,
                transition_interval_seconds: 0.25,
                use_full_segment_transition: false,
                target_position: [1.0, 2.0, 3.0],
                distance: default_camera_trigger_distance(),
                rotation: 0.8,
                pitch: 0.2,
            },
        ];

        let triggers = camera_triggers_to_timed_triggers(&camera_triggers);
        assert_eq!(triggers.len(), 2);
        assert!(matches!(triggers[0].target, TimedTriggerTarget::Camera));
        assert!(matches!(
            triggers[0].action,
            TimedTriggerAction::CameraFollow { .. }
        ));

        let restored = timed_triggers_to_camera_triggers(&triggers);
        assert_eq!(restored, camera_triggers);
    }

    #[test]
    fn resolved_triggers_return_trigger_data_for_camera_conversion() {
        let metadata = LevelMetadata {
            name: "Bridge".to_string(),
            objects: vec![LevelObject {
                position: [0.0, 0.0, 0.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: [0.0, 0.0, 0.0],
                block_id: CAMERA_TRIGGER_BLOCK_ID.to_string(),
                color_tint: [1.0, 1.0, 1.0],
                trigger: camera_triggers_to_timed_triggers(&[CameraTrigger {
                    time_seconds: 1.2,
                    mode: CameraTriggerMode::Static,
                    easing: TimedTriggerEasing::Linear,
                    transition_interval_seconds: 1.0,
                    use_full_segment_transition: false,
                    target_position: [2.0, 3.0, 4.0],
                    distance: default_camera_trigger_distance(),
                    rotation: 0.4,
                    pitch: 0.6,
                }])
                .into_iter()
                .next(),
                ..LevelObject::default()
            }],
            ..LevelMetadata::default()
        };
        let mut metadata = metadata;
        metadata.objects[0].position = [9.0, 8.0, 7.0];
        metadata.objects[0].rotation_degrees = [30.0, 60.0, 0.0];

        let resolved = timed_triggers_to_camera_triggers(&metadata.resolved_triggers());
        let expected_target = camera_trigger_target_from_eye(
            camera_trigger_eye_from_object(&metadata.objects[0]),
            60.0_f32.to_radians(),
            30.0_f32.to_radians(),
        );
        let expected_eye = camera_trigger_eye_from_object(&metadata.objects[0]);
        let resolved_eye = camera_trigger_eye_from_target(
            resolved[0].target_position,
            resolved[0].rotation,
            resolved[0].pitch,
        );
        assert_eq!(resolved.len(), 1);
        assert!((resolved[0].time_seconds - 1.2).abs() <= 1e-6);
        for (actual, expected) in resolved[0].target_position.iter().zip(expected_target) {
            assert_approx_eq(*actual, expected);
        }
        for (actual, expected) in resolved_eye.iter().zip(expected_eye) {
            assert_approx_eq(*actual, expected);
        }
        assert!((resolved[0].pitch - 30.0_f32.to_radians()).abs() <= 1e-6);
        assert!((resolved[0].rotation - 60.0_f32.to_radians()).abs() <= 1e-6);
    }

    #[test]
    fn resolved_camera_trigger_normalizes_wrapped_pitch_rotation() {
        let object = LevelObject {
            position: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            rotation_degrees: [-135.0, 0.0, 180.0],
            block_id: CAMERA_TRIGGER_BLOCK_ID.to_string(),
            color_tint: [1.0, 1.0, 1.0],
            trigger: camera_triggers_to_timed_triggers(&[CameraTrigger {
                time_seconds: 1.2,
                mode: CameraTriggerMode::Static,
                easing: TimedTriggerEasing::Linear,
                transition_interval_seconds: 1.0,
                use_full_segment_transition: false,
                target_position: [2.0, 3.0, 4.0],
                distance: default_camera_trigger_distance(),
                rotation: 0.0,
                pitch: 0.0,
            }])
            .into_iter()
            .next(),
            ..LevelObject::default()
        };
        let metadata = LevelMetadata {
            name: "Bridge".to_string(),
            objects: vec![object],
            ..LevelMetadata::default()
        };

        let resolved = timed_triggers_to_camera_triggers(&metadata.resolved_triggers());
        let (expected_rotation, expected_pitch) =
            camera_trigger_rotation_pitch_from_rotation_degrees([-135.0, 0.0, 180.0]);
        let expected_forward = camera_trigger_forward_from_rotation_degrees([-135.0, 0.0, 180.0]);
        let resolved_forward = camera_trigger_forward(resolved[0].rotation, resolved[0].pitch);
        let resolved_eye = camera_trigger_eye_from_target(
            resolved[0].target_position,
            resolved[0].rotation,
            resolved[0].pitch,
        );

        assert_eq!(resolved.len(), 1);
        assert_approx_eq(resolved[0].rotation, expected_rotation);
        assert_approx_eq(resolved[0].pitch, expected_pitch);
        assert!(resolved_forward[1] < -0.6);
        for (actual, expected) in resolved_forward.iter().zip(expected_forward) {
            assert_approx_eq(*actual, expected);
        }
        for (actual, expected) in resolved_eye
            .iter()
            .zip(camera_trigger_eye_from_object(&metadata.objects[0]))
        {
            assert_approx_eq(*actual, expected);
        }
    }

    #[test]
    fn resolved_transform_trigger_uses_trigger_object_transform() {
        let object = LevelObject {
            id: "trigger".to_string(),
            position: [4.0, 5.0, 6.0],
            size: [2.0, 3.0, 4.0],
            rotation_degrees: [10.0, 20.0, 30.0],
            block_id: TRANSFORM_TRIGGER_BLOCK_ID.to_string(),
            color_tint: [1.0, 1.0, 1.0],
            trigger: Some(TimedTrigger {
                time_seconds: 1.0,
                duration_seconds: 2.0,
                easing: TimedTriggerEasing::EaseInOut,
                target: TimedTriggerTarget::Objects {
                    object_ids: vec!["target".to_string()],
                },
                action: TimedTriggerAction::TransformObjects {
                    position: [0.0, 0.0, 0.0],
                    rotation_degrees: [0.0, 0.0, 0.0],
                    size: [1.0, 1.0, 1.0],
                },
            }),
            ..LevelObject::default()
        };

        let triggers = triggers_from_objects(&[object]);
        assert_eq!(triggers.len(), 1);
        match &triggers[0].action {
            TimedTriggerAction::TransformObjects {
                position,
                rotation_degrees,
                size,
            } => {
                assert_eq!(position, &[4.0, 5.0, 6.0]);
                assert_eq!(rotation_degrees, &[10.0, 20.0, 30.0]);
                assert_eq!(size, &[2.0, 3.0, 4.0]);
            }
            _ => panic!("expected transform trigger"),
        }
    }

    #[test]
    fn timed_triggers_apply_easing_sorting_and_scale_clamp() {
        let base = vec![LevelObject {
            id: "target".to_string(),
            position: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            block_id: "core/stone".to_string(),
            color_tint: [1.0, 1.0, 1.0],
            trigger: None,
            ..LevelObject::default()
        }];

        let triggers = vec![TimedTrigger {
            time_seconds: 2.0,
            duration_seconds: 1.0,
            easing: TimedTriggerEasing::Linear,
            target: TimedTriggerTarget::Objects {
                object_ids: vec!["target".to_string()],
            },
            action: TimedTriggerAction::TransformObjects {
                position: [10.0, 0.0, 0.0],
                rotation_degrees: [0.0, 90.0, 0.0],
                size: [2.0, 0.0, 3.0],
            },
        }];

        let applied = apply_timed_triggers_to_objects(&base, &triggers, 2.5);
        let object = &applied[0];
        assert_approx_eq(object.position[0], 5.0);
        assert_approx_eq(object.rotation_degrees[1], 45.0);
        assert_approx_eq(object.size[0], 1.5);
        assert_approx_eq(object.size[1], 0.505);
        assert_approx_eq(object.size[2], 2.0);
    }

    #[test]
    fn timed_triggers_skip_camera_and_out_of_bounds_targets_and_deduplicate() {
        let base = vec![
            LevelObject {
                id: "object_0".to_string(),
                position: [0.0, 0.0, 0.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: [0.0, 0.0, 0.0],
                block_id: "core/stone".to_string(),
                color_tint: [1.0, 1.0, 1.0],
                trigger: None,
                ..LevelObject::default()
            },
            LevelObject {
                id: "object_1".to_string(),
                position: [1.0, 0.0, 0.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: [0.0, 0.0, 0.0],
                block_id: "core/stone".to_string(),
                color_tint: [1.0, 1.0, 1.0],
                trigger: None,
                ..LevelObject::default()
            },
            LevelObject {
                id: "object_2".to_string(),
                position: [2.0, 0.0, 0.0],
                size: [1.0, 1.0, 1.0],
                rotation_degrees: [0.0, 0.0, 0.0],
                block_id: "core/stone".to_string(),
                color_tint: [1.0, 1.0, 1.0],
                trigger: None,
                ..LevelObject::default()
            },
        ];

        let triggers = vec![
            TimedTrigger {
                time_seconds: 0.0,
                duration_seconds: 0.0,
                easing: TimedTriggerEasing::Linear,
                target: TimedTriggerTarget::Objects {
                    object_ids: vec![
                        "object_2".to_string(),
                        "object_2".to_string(),
                        "missing".to_string(),
                        "object_1".to_string(),
                    ],
                },
                action: TimedTriggerAction::TransformObjects {
                    position: [9.0, 9.0, 9.0],
                    rotation_degrees: [0.0, 0.0, 0.0],
                    size: [1.0, 1.0, 1.0],
                },
            },
            TimedTrigger {
                time_seconds: 0.0,
                duration_seconds: 0.0,
                easing: TimedTriggerEasing::Linear,
                target: TimedTriggerTarget::Camera,
                action: TimedTriggerAction::TransformObjects {
                    position: [5.0, 5.0, 5.0],
                    rotation_degrees: [0.0, 0.0, 0.0],
                    size: [1.0, 1.0, 1.0],
                },
            },
        ];

        let applied = apply_timed_triggers_to_objects(&base, &triggers, 0.0);
        assert_eq!(applied[0].position, [0.0, 0.0, 0.0]);
        assert_eq!(applied[1].position, [9.0, 9.0, 9.0]);
        assert_eq!(applied[2].position, [9.0, 9.0, 9.0]);
    }

    #[test]
    fn timed_triggers_ignore_non_finite_start_time() {
        let base = vec![LevelObject {
            id: "target".to_string(),
            position: [0.0, 0.0, 0.0],
            size: [1.0, 1.0, 1.0],
            rotation_degrees: [0.0, 0.0, 0.0],
            block_id: "core/stone".to_string(),
            color_tint: [1.0, 1.0, 1.0],
            trigger: None,
            ..LevelObject::default()
        }];
        let triggers = vec![TimedTrigger {
            time_seconds: f32::NAN,
            duration_seconds: 0.0,
            easing: TimedTriggerEasing::Linear,
            target: TimedTriggerTarget::Objects {
                object_ids: vec!["target".to_string()],
            },
            action: TimedTriggerAction::TransformObjects {
                position: [99.0, 0.0, 0.0],
                rotation_degrees: [0.0, 0.0, 0.0],
                size: [1.0, 1.0, 1.0],
            },
        }];

        let applied = apply_timed_triggers_to_objects(&base, &triggers, 10.0);
        assert_eq!(applied, base);
    }

    #[test]
    fn test_essential_keybind_groups() {
        let actions = eg_types::essential_keybind_actions();
        assert!(!actions.is_empty());
        for metadata in actions {
            assert!(!metadata.group.is_empty());
            assert!(!metadata.action.is_empty());
        }
    }

    #[test]
    fn editor_tab_keybinds_are_registered_with_defaults() {
        let actions = eg_types::essential_keybind_actions();
        for action in [
            "pick_selected_block",
            "focus_camera_target",
            "tab_compose",
            "tab_timing",
            "tab_tapping",
        ] {
            assert!(
                actions.iter().any(|metadata| metadata.action == action),
                "{action} should be shown in keybind settings"
            );
        }

        let defaults = eg_types::default_essential_keybinds();
        let expected = [
            (
                "pick_selected_block",
                eg_types::KeyChord::new("b", false, false, false),
            ),
            (
                "focus_camera_target",
                eg_types::KeyChord::new("f", false, false, false),
            ),
            (
                "tab_compose",
                eg_types::KeyChord::new("1", true, false, false),
            ),
            (
                "tab_timing",
                eg_types::KeyChord::new("2", true, false, false),
            ),
            (
                "tab_tapping",
                eg_types::KeyChord::new("3", true, false, false),
            ),
        ];

        for (action, chord) in expected {
            assert!(
                defaults
                    .iter()
                    .any(|binding| binding.action == action && binding.chord == chord),
                "{action} should have the expected default chord"
            );
        }
    }

    #[test]
    fn test_app_settings_keybind_management() {
        let mut settings = eg_types::AppSettings::default();
        let action = "zoom_in"; // capacity 2
        let chord1 = eg_types::KeyChord::new("=", false, false, false);
        let chord2 = eg_types::KeyChord::new("+", false, false, false);
        let chord3 = eg_types::KeyChord::new("k", true, false, false);

        // Clear defaults first to be sure
        settings.keybinds.retain(|b| b.action != action);
        assert_eq!(settings.keybinds_for_action(action).len(), 0);

        // Test multi-slot append
        settings.set_keybind_at_slot(action, 0, chord1.clone());
        settings.set_keybind_at_slot(action, 1, chord2.clone());
        assert_eq!(settings.keybinds_for_action(action).len(), 2);

        // Test duplicate chord prevention SAME action
        settings.set_keybind_at_slot(action, 1, chord1.clone());
        assert_eq!(settings.keybinds_for_action(action).len(), 2);
        assert_eq!(settings.keybinds_for_action(action)[0], &chord1);
        assert_eq!(settings.keybinds_for_action(action)[1], &chord2);

        // Test capacity enforcement (max 2)
        settings.set_keybind_at_slot(action, 2, chord3.clone());
        assert_eq!(settings.keybinds_for_action(action).len(), 2);

        // Test slot replacement
        settings.set_keybind_at_slot(action, 0, chord3.clone());
        assert_eq!(settings.keybinds_for_action(action).len(), 2);
        assert!(settings.keybinds_for_action(action).contains(&&chord3));

        // Test single-slot replacement
        let single_action = "undo";
        let u_chord = eg_types::KeyChord::new("u", false, false, false);
        settings.set_keybind_at_slot(single_action, 0, u_chord.clone());
        assert_eq!(settings.keybinds_for_action(single_action).len(), 1);
        assert_eq!(
            settings.keybinds_for_action(single_action).first().copied(),
            Some(&u_chord)
        );

        let u_chord2 = eg_types::KeyChord::new("z", true, false, false);
        settings.set_keybind_at_slot(single_action, 0, u_chord2.clone());
        assert_eq!(settings.keybinds_for_action(single_action).len(), 1);
        assert_eq!(
            settings.keybinds_for_action(single_action).first().copied(),
            Some(&u_chord2)
        );
    }

    #[test]
    fn untextured_vertex_uses_texture_bypass_layer() {
        let vertex = Vertex::untextured([1.0, 2.0, 3.0], [0.2, 0.3, 0.4, 0.5]);
        assert!(
            vertex.texture_layer < 0.0,
            "untextured vertices should bypass texture sampling"
        );
    }

    #[test]
    fn test_reset_essential_keybinds_with_groups() {
        let mut settings = eg_types::AppSettings::default();
        let action = "toggle_settings";
        let custom_chord = eg_types::KeyChord::new("k", true, false, false);

        // Change a default keybind
        settings.set_keybind_at_slot(action, 0, custom_chord.clone());
        assert_eq!(
            settings.keybinds_for_action(action).first().copied(),
            Some(&custom_chord)
        );

        // Reset
        settings.reset_essential_keybinds();

        // Should be back to default
        let defaults = eg_types::default_essential_keybinds();
        let default_chord = defaults
            .iter()
            .find(|b| b.action == action)
            .map(|b| &b.chord);
        assert_eq!(
            settings.keybinds_for_action(action).first().copied(),
            default_chord
        );
    }

    #[test]
    fn test_reset_single_multi_slot_keybind() {
        let mut settings = eg_types::AppSettings::default();
        let action = "zoom_in"; // Capacity 2, defaults are = and +

        // Custom bindings
        settings.set_keybind_at_slot(action, 0, eg_types::KeyChord::new("1", false, false, false));
        settings.set_keybind_at_slot(action, 1, eg_types::KeyChord::new("2", false, false, false));
        assert_eq!(settings.keybinds_for_action(action).len(), 2);

        // Reset
        settings.reset_keybind(action);

        // Should be back to BOTH defaults
        let chords = settings.keybinds_for_action(action);
        assert_eq!(chords.len(), 2);

        let defaults = eg_types::default_essential_keybinds();
        let expected: Vec<_> = defaults
            .iter()
            .filter(|b| b.action == action)
            .map(|b| &b.chord)
            .collect();
        assert_eq!(chords, expected);
    }

    #[test]
    fn app_settings_ui_scale_multiplier_is_clamped() {
        assert!((eg_types::AppSettings::clamp_ui_scale_multiplier(1.0) - 1.0).abs() <= 1e-6);
        assert!((eg_types::AppSettings::clamp_ui_scale_multiplier(0.1) - 0.5).abs() <= 1e-6);
        assert!((eg_types::AppSettings::clamp_ui_scale_multiplier(6.0) - 3.0).abs() <= 1e-6);
        assert!((eg_types::AppSettings::clamp_ui_scale_multiplier(f32::NAN) - 1.0).abs() <= 1e-6);
    }

    #[test]
    fn app_settings_level_progress_keeps_best_attempt() {
        let mut settings = eg_types::AppSettings::default();

        assert!(settings.update_level_progress(
            "Flowerfield",
            eg_types::PlayerLevelProgress {
                progress_percent: 45.0,
                completed: false,
                gems_collected: 1,
                gems_max: 3,
            },
        ));
        assert!(settings.update_level_progress(
            "Flowerfield",
            eg_types::PlayerLevelProgress {
                progress_percent: 20.0,
                completed: true,
                gems_collected: 2,
                gems_max: 3,
            },
        ));

        let progress = settings.level_progress.get("Flowerfield").copied().unwrap();
        assert!((progress.progress_percent - 45.0).abs() <= 1e-6);
        assert!(progress.completed);
        assert_eq!(progress.gems_collected, 2);
        assert_eq!(progress.gems_max, 3);
    }
}
