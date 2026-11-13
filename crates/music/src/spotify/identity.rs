//! Converts opaque app ids without losing the Spotify audio entity's kind.

use crate::EPISODE_PREFIX;

const TRACK_URI: &str = "spotify:track:";
const EPISODE_URI: &str = "spotify:episode:";

/// Keeps existing song ids unchanged and gives episodes a separate namespace.
pub(super) fn from_uri(uri: &str) -> Option<String> {
    if let Some(id) = uri.strip_prefix(EPISODE_URI).filter(|id| !id.is_empty()) {
        return Some(format!("{EPISODE_PREFIX}{id}"));
    }
    uri.strip_prefix(TRACK_URI)
        .filter(|id| !id.is_empty())
        .map(str::to_owned)
}

/// Restores the URI for playback, metadata, playlist edits, and Connect.
pub(crate) fn to_uri(id: &str) -> String {
    match id.strip_prefix(EPISODE_PREFIX) {
        Some(id) => format!("{EPISODE_URI}{id}"),
        None => format!("{TRACK_URI}{id}"),
    }
}

/// Whether a playlist item names supported audio rather than a folder or local file.
pub(super) fn is_audio(uri: &str) -> bool {
    uri.starts_with(TRACK_URI) || uri.starts_with(EPISODE_URI)
}
