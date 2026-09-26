/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
//! Platform services and background tasks write three `mpsc` channels
//! (auth service, audio service, level importer). App-owned `StateSchedule`
//! drains those channels and emits typed result messages into the World.
//!
//! ## Why messages and not direct resource writes?
//!
//! - Independent readers - each handler keeps a filtered cursor over the
//!   shared app message buffer, so multiple consumers remain independent.
//! - Testable in isolation - a test can write a synthetic
//!   `AudioImportResult` into the shared app message buffer and assert the
//!   handler behavior without going through
//!   the platform layer.
//! - Unlocks async-result consumers - `bevy_tasks`-based
//!   systems can read the events without holding the channel
//!   receiver (which is `!Sync`).

/// Result of an async auth service call.
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
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

/// Result of an async level-import request.
#[derive(Debug, Clone)]
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
    fn level_import_result_event_carries_bytes() {
        let event = LevelImportResultEvent::new(vec![0xDE, 0xAD, 0xBE, 0xEF]);
        assert_eq!(event.bytes, vec![0xDE, 0xAD, 0xBE, 0xEF]);
    }
}
