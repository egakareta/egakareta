/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn get_egb_path() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_egb"))
}

#[test]
fn test_egb_help() {
    let output = Command::new(get_egb_path())
        .arg("--help")
        .output()
        .expect("Failed to execute egb");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("egb - level metadata conversion helper"));
}

#[test]
fn test_egb_roundtrip() {
    let egb_bin = get_egb_path();
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");

    let json_path = temp_dir.path().join("test.json");
    let egb_path = temp_dir.path().join("test.egb");

    let sample_json = r#"{
  "version": 1,
  "name": "Test Level",
  "music": {
    "source": "audio.mp3",
    "title": "Test Title",
    "author": "Test Author",
    "extra": {}
  },
  "spawn": {
    "position": [0.0, 1.0, 0.0],
    "direction": "right"
  },
  "tap_times": [1.0, 2.0],
  "timing_points": [],
  "timeline_time_seconds": 0.0,
  "timeline_duration_seconds": 10.0,
  "triggers": [],
  "simulate_trigger_hitboxes": true,
  "objects": [
    {
      "position": [0.0, 0.0, 0.0],
      "size": [1.0, 1.0, 1.0],
      "rotation_degrees": [0.0, 0.0, 0.0],
      "roundness": 0.0,
      "block_id": "core/stone",
      "color_tint": [1.0, 1.0, 1.0]
    }
  ],
  "extra": {}
}"#;

    fs::write(&json_path, sample_json).expect("Failed to write sample json");

    // Encode
    let encode_output = Command::new(&egb_bin)
        .arg("encode")
        .arg("--input")
        .arg(&json_path)
        .arg("--output")
        .arg(&egb_path)
        .output()
        .expect("Failed to execute egb encode");

    assert!(
        encode_output.status.success(),
        "Encode failed: {}",
        String::from_utf8_lossy(&encode_output.stderr)
    );
    assert!(egb_path.exists());

    // Decode
    let decode_output = Command::new(&egb_bin)
        .arg("decode")
        .arg("--input")
        .arg(&egb_path)
        .output()
        .expect("Failed to execute egb decode");

    assert!(
        decode_output.status.success(),
        "Decode failed: {}",
        String::from_utf8_lossy(&decode_output.stderr)
    );

    let decoded_json = String::from_utf8_lossy(&decode_output.stdout);

    // Check if valid JSON
    let _: serde_json::Value =
        serde_json::from_str(&decoded_json).expect("Decoded output is not valid JSON");

    // We don't necessarily expect exact string match because of pretty printing differences or field order,
    // but we can check if the parsed values are same.
    let original: serde_json::Value = serde_json::from_str(sample_json).unwrap();
    let decoded: serde_json::Value = serde_json::from_str(&decoded_json).unwrap();

    assert_eq!(original["name"], decoded["name"]);
    assert_eq!(original["music"]["source"], decoded["music"]["source"]);
    assert_eq!(
        original["objects"].as_array().unwrap().len(),
        decoded["objects"].as_array().unwrap().len()
    );
}

#[test]
fn test_egb_encode_stdin() {
    let egb_bin = get_egb_path();
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let egb_path = temp_dir.path().join("test_stdin.egb");

    let sample_json = r#"{
  "version": 1,
  "name": "Stdin Test",
  "music": {
    "source": "audio.mp3",
    "title": "Stdin Track",
    "author": "Stdin Composer",
    "extra": {}
  },
  "spawn": {
    "position": [0.0, 0.0, 0.0],
    "direction": "forward"
  },
  "tap_times": [],
  "timing_points": [],
  "timeline_time_seconds": 0.0,
  "timeline_duration_seconds": 16.0,
  "triggers": [],
  "simulate_trigger_hitboxes": true,
  "objects": [],
  "extra": {}
}"#;

    use std::io::Write;
    let mut child = Command::new(&egb_bin)
        .arg("encode")
        .arg("--output")
        .arg(&egb_path)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .expect("Failed to spawn egb");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    stdin
        .write_all(sample_json.as_bytes())
        .expect("Failed to write to stdin");
    drop(stdin);

    let status = child.wait().expect("Failed to wait for child");
    assert!(status.success());
    assert!(egb_path.exists());

    // Verify it can be decoded back
    let decode_output = Command::new(&egb_bin)
        .arg("decode")
        .arg("--input")
        .arg(&egb_path)
        .output()
        .expect("Failed to execute egb decode");

    assert!(decode_output.status.success());
    let decoded_json = String::from_utf8_lossy(&decode_output.stdout);
    let decoded: serde_json::Value = serde_json::from_str(&decoded_json).unwrap();
    assert_eq!(decoded["name"], "Stdin Test");
}

#[test]
fn test_egb_short_flags() {
    let egb_bin = get_egb_path();
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");

    let json_path = temp_dir.path().join("test.json");
    let egb_path = temp_dir.path().join("test.egb");
    let output_json_path = temp_dir.path().join("output.json");

    let sample_json = r#"{
  "version": 1,
  "name": "Short Flag Test",
  "music": {
    "source": "audio.mp3",
    "title": "Title",
    "author": "Author",
    "extra": {}
  },
  "spawn": {
    "position": [0.0, 0.0, 0.0],
    "direction": "forward"
  },
  "tap_times": [],
  "timing_points": [],
  "timeline_time_seconds": 0.0,
  "timeline_duration_seconds": 16.0,
  "triggers": [],
  "simulate_trigger_hitboxes": true,
  "objects": [],
  "extra": {}
}"#;

    fs::write(&json_path, sample_json).unwrap();

    // Encode with -i and -o
    let status = Command::new(&egb_bin)
        .arg("encode")
        .arg("-i")
        .arg(&json_path)
        .arg("-o")
        .arg(&egb_path)
        .status()
        .unwrap();
    assert!(status.success());

    // Decode with -i and -o
    let status = Command::new(&egb_bin)
        .arg("decode")
        .arg("-i")
        .arg(&egb_path)
        .arg("-o")
        .arg(&output_json_path)
        .status()
        .unwrap();
    assert!(status.success());

    let decoded_json = fs::read_to_string(&output_json_path).unwrap();
    let decoded: serde_json::Value = serde_json::from_str(&decoded_json).unwrap();
    assert_eq!(decoded["name"], "Short Flag Test");
}

#[test]
fn test_egb_missing_input() {
    let egb_bin = get_egb_path();
    let output = Command::new(&egb_bin)
        .arg("decode")
        .arg("--input")
        .arg("non_existent_file.egb")
        .output()
        .expect("Failed to execute egb");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Failed to read input file"));
}
