/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Platform services and background tasks write five `mpsc` channels
//! (auth service, audio service, level importer). App-owned `StateSchedule`
//! drains those channels and emits typed result messages into the World.
//!
//! ## Why messages and not direct resource writes?
//!
//! - One reader per message type - distinct `Message<T>` types
//!   give each handler its own `MessageReader<T>`, so multiple
//!   readers of the same channel can be added without
//!   cross-reader nondeterminism.
//! - Testable in isolation - a test can write a synthetic
//!   `AudioImportResult` into the `Messages<AudioImportResultEvent>`
//!   buffer and assert the handler behavior without going through
//!   the platform layer.
//! - Unlocks async-result consumers - `bevy_tasks`-based
//!   systems can read the events without holding the channel
//!   receiver (which is `!Sync`).

use bevy_ecs::prelude::*;
use eg_audio::service::{AudioPreloadResult, WaveformResult};

/// Result of an async auth service call.
#[allow(dead_code)]
#[derive(Message, Debug, Clone)]
pub struct AuthServiceResultEvent {
    /// Original auth service message.
    pub message: eg_platform::services::AuthServiceMessage,
}

impl AuthServiceResultEvent {
    /// Construct a new event.
    pub fn new(message: eg_platform::services::AuthServiceMessage) -> Self {
        Self { message }
    }
}

/// Result of an async audio-import request.
#[allow(dead_code)]
#[derive(Message, Debug, Clone)]
pub struct AudioImportResultEvent {
    /// Original filename.
    pub filename: String,
    /// Raw audio bytes on success.
    pub bytes: Option<Vec<u8>>,
}

impl AudioImportResultEvent {
    /// Construct a new event.
    pub fn new(filename: String, bytes: Option<Vec<u8>>) -> Self {
        Self { filename, bytes }
    }
}

/// Result of an async waveform-load request.
///
/// Note: `WaveformLoadMessage` does not implement `Clone`, so this
/// event is **not** `Clone` - it's moved into the `Messages<>` buffer
/// by the poll system and consumed once by the handler system.
#[allow(dead_code)]
#[derive(Message, Debug)]
pub struct WaveformResultEvent {
    /// Source identifier.
    pub source: String,
    /// Level name.
    pub level_name: String,
    /// The waveform load message.
    pub message: eg_audio::service::WaveformLoadMessage,
}

impl WaveformResultEvent {
    /// Construct from a WaveformResult (consumes the result).
    pub fn from_result(result: WaveformResult) -> Self {
        Self {
            source: result.0,
            level_name: result.1,
            message: result.2,
        }
    }
}

/// Result of an async audio preload request.
#[allow(dead_code)]
#[derive(Message, Debug, Clone)]
pub struct AudioPreloadResultEvent {
    /// Source key.
    pub source_key: String,
    /// Preloaded bytes on success.
    pub bytes: Option<Vec<u8>>,
}

impl AudioPreloadResultEvent {
    /// Construct from an AudioPreloadResult.
    pub fn from_result(result: &AudioPreloadResult) -> Self {
        Self {
            source_key: result.0.clone(),
            bytes: result.1.clone(),
        }
    }
}

/// Result of an async level-import request.
#[allow(dead_code)]
#[derive(Message, Debug, Clone)]
pub struct LevelImportResultEvent {
    /// Imported level bytes.
    pub bytes: Vec<u8>,
}

impl LevelImportResultEvent {
    /// Construct a new event.
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_service_result_event_wraps_message() {
        use eg_types::{AuthSession, AuthSessionTokens, AuthUser};
        let session = AuthSession {
            session: AuthSessionTokens {
                access_token: String::new(),
                refresh_token: String::new(),
                expires_at: Some(0),
                token_type: String::new(),
            },
            user: AuthUser {
                id: String::new(),
                email: None,
            },
            profile: None,
        };
        let message = eg_platform::services::AuthServiceMessage::SignedIn(Ok(session));
        let event = AuthServiceResultEvent::new(message);
        assert!(matches!(
            event.message,
            eg_platform::services::AuthServiceMessage::SignedIn(_)
        ));
    }

    #[test]
    fn audio_import_result_event_carries_filename_and_bytes() {
        let event = AudioImportResultEvent::new("level1.ogg".to_string(), Some(vec![1, 2, 3]));
        assert_eq!(event.filename, "level1.ogg");
        assert_eq!(event.bytes, Some(vec![1, 2, 3]));
    }

    #[test]
    fn audio_import_result_event_supports_cancellation() {
        let event = AudioImportResultEvent::new("level1.ogg".to_string(), None);
        assert!(event.bytes.is_none());
    }

    #[test]
    fn waveform_result_event_from_result_preserves_payload() {
        let result: WaveformResult = (
            "audio.mp3".to_string(),
            "Level1".to_string(),
            eg_audio::service::WaveformLoadMessage::Failed { bytes: None },
        );
        let event = WaveformResultEvent::from_result(result);
        assert_eq!(event.source, "audio.mp3");
        assert_eq!(event.level_name, "Level1");
        assert!(matches!(
            event.message,
            eg_audio::service::WaveformLoadMessage::Failed { .. }
        ));
    }

    #[test]
    fn audio_preload_result_event_from_result_preserves_payload() {
        let result: AudioPreloadResult = ("Level1:audio.mp3".to_string(), Some(vec![9, 9, 9]));
        let event = AudioPreloadResultEvent::from_result(&result);
        assert_eq!(event.source_key, "Level1:audio.mp3");
        assert_eq!(event.bytes, Some(vec![9, 9, 9]));
    }

    #[test]
    fn audio_preload_result_event_supports_failure() {
        let result: AudioPreloadResult = ("Level1:audio.mp3".to_string(), None);
        let event = AudioPreloadResultEvent::from_result(&result);
        assert!(event.bytes.is_none());
    }

    #[test]
    fn level_import_result_event_carries_bytes() {
        let event = LevelImportResultEvent::new(vec![0xDE, 0xAD, 0xBE, 0xEF]);
        assert_eq!(event.bytes, vec![0xDE, 0xAD, 0xBE, 0xEF]);
    }
}
