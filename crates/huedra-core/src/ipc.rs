//! Messages between the tray process and the command line or settings
//! window, sent over the per-user named pipe as one JSON line each way.
//! Feature doc: `docs/features/09-cli-ipc.md`.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::engine::Mode;

/// Protocol version; both sides must match.
pub const PROTOCOL_VERSION: u32 = 1;
/// Messages larger than this are refused before parsing.
pub const MAX_MESSAGE_BYTES: usize = 64 * 1024;

/// What a client asks the tray process to do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "request", rename_all = "kebab-case")]
pub enum Request {
    /// Report the current state.
    Status,
    /// Change the mode.
    SetMode {
        /// The new mode.
        mode: Mode,
    },
    /// Pause automatic switching.
    Pause {
        /// Length of the pause.
        minutes: u32,
    },
    /// End a pause.
    Resume,
    /// Turn HDR on or off now, as the shortcut does.
    ToggleHdr,
    /// Turn HDR on or off on one display now (the quick panel's switch).
    SetDisplayHdr {
        /// Device path, as in [`DisplayReport::id`].
        display: String,
        /// HDR on or off.
        on: bool,
    },
    /// Never change HDR for the game or player active now, as "Never use HDR for"
    /// in the tray menu does ([`StatusReport::never`] names it).
    NeverForActive,
    /// Read the settings file again.
    Reload,
    /// Turn HDR on for a program about to start through `huedra launch`,
    /// and keep it on while the launching process lives. Answered once HDR
    /// is on, or with the reason it stays off.
    Launch {
        /// Process id of `huedra launch`.
        pid: u32,
        /// The program it starts, to name the game.
        program: String,
    },
    /// A newer signed release was found and waits for the user's consent
    /// (Updates: Ask before installing). Sent by `huedra update --auto`.
    UpdateAvailable {
        /// Its version, `1.2.3`.
        version: String,
    },
    /// Close the tray process.
    Exit,
}

/// The tray process's answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "response", rename_all = "kebab-case")]
pub enum Response {
    /// Done; the message describes the result.
    Done {
        /// One sentence for the user.
        message: String,
    },
    /// The current state.
    Status(StatusReport),
    /// The request could not be carried out.
    Failed {
        /// One sentence for the user.
        message: String,
    },
}

/// State of the tray process for the command line and the Status page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusReport {
    /// Current mode.
    pub mode: Mode,
    /// Minutes left in a pause.
    pub paused_minutes: Option<u32>,
    /// Whether a managed display is in HDR.
    pub hdr_on: bool,
    /// One line, as in the tray tooltip.
    pub summary: String,
    /// Why the state is what it is, for example "Cyberpunk 2077 is running".
    pub reason: String,
    /// Ids of the games currently keeping HDR on or about to.
    pub active_games: Vec<String>,
    /// Whether a game, video or launch is running, or HDR that Huedra
    /// turned on is still on: an update must wait.
    #[serde(default)]
    pub busy: bool,
    /// Managed displays.
    pub displays: Vec<DisplayReport>,
    /// The game or player "Never use HDR for" would apply to; empty when none.
    #[serde(default)]
    pub never: String,
    /// Shortcuts Windows refused because another program uses them, for
    /// example "Win+Ctrl+M is used by another program."; empty when none.
    #[serde(default)]
    pub shortcut_note: String,
    /// Recent changes, newest first.
    pub history: Vec<HistoryEntry>,
}

/// One managed display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DisplayReport {
    /// Device path.
    pub id: String,
    /// Name and number, for example "Display 1 (MO27Q3)".
    pub label: String,
    /// Whether it is in HDR.
    pub hdr_on: bool,
    /// Where the engine stands, in words.
    pub detail: String,
}

/// Kind of history entry, used for the badge color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HistoryKind {
    /// HDR turned on.
    Hdr,
    /// HDR turned off.
    Sdr,
    /// Change outside Huedra.
    External,
    /// A switch failed.
    Failed,
}

/// One line of "Recent changes".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    /// Local time, `HH:MM`.
    pub time: String,
    /// Local day, `YYYY-MM-DD`; empty from a tray of an earlier version.
    #[serde(default)]
    pub day: String,
    /// Short title, for example "HDR turned on".
    pub title: String,
    /// Reason.
    pub detail: String,
    /// Badge.
    pub kind: HistoryKind,
}

#[derive(Serialize, Deserialize)]
struct Envelope<T> {
    v: u32,
    body: T,
}

/// A message could not be encoded or decoded.
#[derive(Debug, thiserror::Error)]
pub enum IpcError {
    /// Larger than [`MAX_MESSAGE_BYTES`].
    #[error("the message is too large")]
    TooLarge,
    /// The other side speaks another protocol version.
    #[error("the running Huedra is a different version; restart it")]
    Version,
    /// Not valid JSON for the expected type.
    #[error("the message is not valid: {0}")]
    Invalid(String),
}

/// Encodes a message as one JSON line.
///
/// # Errors
/// [`IpcError::TooLarge`] or [`IpcError::Invalid`].
pub fn encode<T: Serialize>(body: &T) -> Result<Vec<u8>, IpcError> {
    let mut bytes = serde_json::to_vec(&Envelope {
        v: PROTOCOL_VERSION,
        body,
    })
    .map_err(|e| IpcError::Invalid(e.to_string()))?;
    bytes.push(b'\n');
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(IpcError::TooLarge);
    }
    Ok(bytes)
}

/// Decodes one message, checking its size and version first.
///
/// # Errors
/// [`IpcError`] when the message is too large, from another version, or
/// malformed.
pub fn decode<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, IpcError> {
    #[derive(Deserialize)]
    struct Version {
        v: u32,
    }
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(IpcError::TooLarge);
    }
    let version: Version =
        serde_json::from_slice(bytes).map_err(|e| IpcError::Invalid(e.to_string()))?;
    if version.v != PROTOCOL_VERSION {
        return Err(IpcError::Version);
    }
    serde_json::from_slice::<Envelope<T>>(bytes)
        .map(|e| e.body)
        .map_err(|e| IpcError::Invalid(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        for request in [
            Request::Status,
            Request::SetMode {
                mode: Mode::AlwaysSdr,
            },
            Request::Pause { minutes: 60 },
            Request::Launch {
                pid: 7,
                program: r"D:\Games\Foo\foo.exe".into(),
            },
            Request::UpdateAvailable {
                version: "0.2.0".into(),
            },
            Request::Exit,
        ] {
            let bytes = encode(&request).expect("encode");
            assert_eq!(bytes.last(), Some(&b'\n'), "line framing");
            assert_eq!(decode::<Request>(&bytes).ok(), Some(request), "round trip");
        }
    }

    #[test]
    fn rejects_bad_messages() {
        assert!(
            matches!(
                decode::<Request>(br#"{"v":2,"body":{"request":"status"}}"#),
                Err(IpcError::Version)
            ),
            "version"
        );
        assert!(
            matches!(
                decode::<Request>(br#"{"v":1,"body":{"request":"format-disk"}}"#),
                Err(IpcError::Invalid(_))
            ),
            "unknown request"
        );
        let big = vec![b' '; MAX_MESSAGE_BYTES + 1];
        assert!(
            matches!(decode::<Request>(&big), Err(IpcError::TooLarge)),
            "size"
        );
    }
}
