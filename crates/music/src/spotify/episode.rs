//! Reads podcast episode metadata, which Spotify serves apart from songs, into song rows.

use std::collections::HashMap;
use std::time::Duration;

use anyhow::{Context as _, Result};
use librespot_core::Session;
use librespot_protocol::extension_kind::ExtensionKind;
use librespot_protocol::metadata::{Episode as EpisodeMessage, ImageGroup};
use protobuf::Message as _;

use crate::Track;
use crate::spotify::{collection, identity, wire};

const EPISODE_URI: &str = "spotify:episode:";

/// Splits metadata URIs into episodes and everything else, keeping each side's order.
pub(super) fn split(uris: &[String]) -> (Vec<String>, Vec<String>) {
    uris.iter()
        .cloned()
        .partition(|uri| uri.starts_with(EPISODE_URI))
}

/// Reads episodes by URI. Episodes Spotify no longer supplies are left out.
pub(super) async fn metadata(session: &Session, uris: &[String]) -> Result<HashMap<String, Track>> {
    let mut tracks = HashMap::new();
    if uris.is_empty() {
        return Ok(tracks);
    }
    let entities = collection::extended(session, uris, ExtensionKind::EPISODE_V4)
        .await
        .context("cannot read episode metadata")?;
    for entity in entities {
        let Ok(message) = EpisodeMessage::parse_from_bytes(&entity.extension_data.value) else {
            continue;
        };
        // Spotify also returns empty extensions for episodes it no longer supplies. An empty
        // protobuf decodes successfully, but it is not a row the user can identify or play.
        if message.name().is_empty() {
            continue;
        }
        let track = track_from(&entity.entity_uri, &message);
        tracks.insert(entity.entity_uri, track);
    }
    Ok(tracks)
}

/// Maps episode audio onto the existing row, without creating artist or album routes for a show.
fn track_from(uri: &str, episode: &EpisodeMessage) -> Track {
    let show = episode.show.as_ref();
    Track {
        id: identity::from_uri(uri),
        name: episode.name().to_owned(),
        playable: !episode.audio.is_empty(),
        artists: show.map(|show| show.name().to_owned()).unwrap_or_default(),
        cover: episode.cover_image.as_ref().and_then(largest).or_else(|| {
            show.and_then(|show| show.cover_image.as_ref())
                .and_then(largest)
        }),
        duration: Duration::from_millis(episode.duration().max(0) as u64),
        explicit: episode.explicit(),
        ..Default::default()
    }
}

/// The largest listed image. Episodes have no album from which fullscreen could load a better
/// cover later, so they take the biggest one now.
fn largest(group: &ImageGroup) -> Option<String> {
    let image = group
        .image
        .iter()
        .filter(|image| image.has_file_id())
        .max_by_key(|image| wire::image_width(image))?;
    wire::image_url(image.file_id())
}
