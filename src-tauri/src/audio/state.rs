use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Audio recording state machine.
///
/// Tracks the lifecycle of audio recording sessions:
/// - Idle: not recording
/// - Recording: active recording session
/// - Stopping: graceful shutdown in progress
#[derive(Debug, Clone)]
pub enum AudioState {
    /// No active recording.
    Idle,

    /// Currently recording.
    Recording {
        /// Unique session ID for this recording.
        session_id: String,
        /// Device being recorded from.
        device_id: String,
        /// When recording started.
        started_at: DateTime<Utc>,
    },

    /// Recording is stopping (transitional state).
    Stopping {
        /// Session being finalized.
        session_id: String,
    },
}

impl AudioState {
    /// Create a new idle state.
    pub fn idle() -> Self {
        AudioState::Idle
    }

    /// Transition to recording state.
    ///
    /// # Arguments
    /// * `device_id` - ID of the audio input device
    ///
    /// # Returns
    /// New Recording state with generated session ID, or an error if already recording.
    pub fn start_recording(self, device_id: String) -> Result<Self, String> {
        match self {
            AudioState::Idle => {
                let session_id = Uuid::new_v4().to_string();
                Ok(AudioState::Recording {
                    session_id,
                    device_id,
                    started_at: Utc::now(),
                })
            }
            AudioState::Recording { .. } => {
                Err("Cannot start recording: already recording".to_string())
            }
            AudioState::Stopping { .. } => {
                Err("Cannot start recording: previous session is stopping".to_string())
            }
        }
    }

    /// Transition to stopping state.
    ///
    /// # Returns
    /// Stopping state with the current session ID, or error if not recording.
    pub fn stop_recording(self) -> Result<Self, String> {
        match self {
            AudioState::Recording { session_id, .. } => Ok(AudioState::Stopping { session_id }),
            AudioState::Idle => Err("Cannot stop: not recording".to_string()),
            AudioState::Stopping { .. } => Err("Cannot stop: already stopping".to_string()),
        }
    }

    /// Finalize stopping and return to idle.
    ///
    /// # Returns
    /// Idle state, or error if not in Stopping state.
    pub fn finalize(self) -> Result<Self, String> {
        match self {
            AudioState::Stopping { .. } => Ok(AudioState::Idle),
            AudioState::Idle => Err("Cannot finalize: not recording".to_string()),
            AudioState::Recording { .. } => {
                Err("Cannot finalize: must stop before finalizing".to_string())
            }
        }
    }

    /// Get the current session ID if recording.
    pub fn session_id(&self) -> Option<&str> {
        match self {
            AudioState::Recording { session_id, .. } => Some(session_id),
            AudioState::Stopping { session_id } => Some(session_id),
            AudioState::Idle => None,
        }
    }

    /// Get the current device ID if recording.
    pub fn device_id(&self) -> Option<&str> {
        match self {
            AudioState::Recording { device_id, .. } => Some(device_id),
            _ => None,
        }
    }

    /// Get the start time if recording.
    pub fn started_at(&self) -> Option<DateTime<Utc>> {
        match self {
            AudioState::Recording { started_at, .. } => Some(*started_at),
            _ => None,
        }
    }

    /// Check if currently recording.
    pub fn is_recording(&self) -> bool {
        matches!(self, AudioState::Recording { .. })
    }

    /// Check if currently stopping.
    pub fn is_stopping(&self) -> bool {
        matches!(self, AudioState::Stopping { .. })
    }

    /// Check if idle.
    pub fn is_idle(&self) -> bool {
        matches!(self, AudioState::Idle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state_is_idle() {
        let state = AudioState::idle();
        assert!(state.is_idle());
        assert!(!state.is_recording());
        assert!(!state.is_stopping());
    }

    #[test]
    fn test_start_recording_from_idle() {
        let state = AudioState::idle();
        let state = state
            .start_recording("device_1".to_string())
            .expect("should start recording");

        assert!(state.is_recording());
        assert!(state.session_id().is_some());
        assert_eq!(state.device_id(), Some("device_1"));
        assert!(state.started_at().is_some());
    }

    #[test]
    fn test_cannot_start_recording_while_recording() {
        let state = AudioState::idle();
        let state = state
            .start_recording("device_1".to_string())
            .expect("should start recording");

        let result = state.start_recording("device_2".to_string());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already recording"));
    }

    #[test]
    fn test_stop_recording() {
        let state = AudioState::idle();
        let state = state
            .start_recording("device_1".to_string())
            .expect("should start recording");
        let session_id = state.session_id().unwrap().to_string();

        let state = state.stop_recording().expect("should stop recording");
        assert!(state.is_stopping());
        assert_eq!(state.session_id(), Some(session_id.as_str()));
    }

    #[test]
    fn test_cannot_stop_when_idle() {
        let state = AudioState::idle();
        let result = state.stop_recording();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not recording"));
    }

    #[test]
    fn test_cannot_stop_twice() {
        let state = AudioState::idle();
        let state = state
            .start_recording("device_1".to_string())
            .expect("should start recording");
        let state = state.stop_recording().expect("should stop recording");

        let result = state.stop_recording();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already stopping"));
    }

    #[test]
    fn test_finalize_from_stopping() {
        let state = AudioState::idle();
        let state = state
            .start_recording("device_1".to_string())
            .expect("should start recording");
        let state = state.stop_recording().expect("should stop recording");

        let state = state.finalize().expect("should finalize");
        assert!(state.is_idle());
        assert!(state.session_id().is_none());
    }

    #[test]
    fn test_cannot_finalize_when_idle() {
        let state = AudioState::idle();
        let result = state.finalize();
        assert!(result.is_err());
    }

    #[test]
    fn test_cannot_finalize_while_recording() {
        let state = AudioState::idle();
        let state = state
            .start_recording("device_1".to_string())
            .expect("should start recording");

        let result = state.finalize();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must stop before finalizing"));
    }

    #[test]
    fn test_full_recording_cycle() {
        let mut state = AudioState::idle();
        assert!(state.is_idle());

        // Start recording
        state = state
            .start_recording("device_main".to_string())
            .expect("start");
        assert!(state.is_recording());
        let session_id = state.session_id().unwrap().to_string();

        // Stop recording
        state = state.stop_recording().expect("stop");
        assert!(state.is_stopping());
        assert_eq!(state.session_id(), Some(session_id.as_str()));

        // Finalize
        state = state.finalize().expect("finalize");
        assert!(state.is_idle());
        assert!(state.session_id().is_none());
    }

    #[test]
    fn test_session_id_persistence() {
        let state = AudioState::idle();
        let state = state
            .start_recording("device_1".to_string())
            .expect("should start recording");
        let session_id = state.session_id().unwrap().to_string();

        let state = state.stop_recording().expect("should stop recording");
        assert_eq!(state.session_id(), Some(session_id.as_str()));

        let state = state.finalize().expect("should finalize");
        assert!(state.session_id().is_none());
    }

    #[test]
    fn test_device_id_tracking() {
        let state = AudioState::idle();
        assert_eq!(state.device_id(), None);

        let state = state
            .start_recording("my_device".to_string())
            .expect("should start recording");
        assert_eq!(state.device_id(), Some("my_device"));

        let state = state.stop_recording().expect("should stop recording");
        assert_eq!(state.device_id(), None); // Device ID only available during Recording

        let state = state.finalize().expect("should finalize");
        assert_eq!(state.device_id(), None);
    }
}
