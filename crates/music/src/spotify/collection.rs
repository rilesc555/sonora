use std::collections::HashMap;
use std::time::Duration;

use anyhow::{Context as _, Result};
use librespot_core::Session;
use librespot_protocol::entity_extension_data::EntityExtensionData;
use librespot_protocol::extended_metadata::{BatchedEntityRequest, EntityRequest, ExtensionQuery};
use librespot_protocol::extension_kind::ExtensionKind;
use librespot_protocol::metadata::{
    Album as AlbumMessage, Artist as ArtistMessage, Episode as EpisodeMessage, ImageGroup,
    Track as TrackMessage,
};
use protobuf::{EnumOrUnknown, Message as _};

use crate::spotify::pace::paced;
use crate::spotify::{collection2, identity, wire};
use crate::{ArtistRef, Credit, Track};

const TRACK_PREFIX: &str = "spotify:track:";
const UNKNOWN: &str = "Unknown";
const BATCH: usize = 500;

pub async fn saved_tracks(session: &Session) -> Result<Vec<Track>> {
    let items = collection2::saved_items(session, collection2::COLLECTION, TRACK_PREFIX).await?;
    if items.is_empty() {
        return Ok(Vec::new());
    }

    let uris: Vec<_> = items.iter().map(|item| item.uri.clone()).collect();
    let mut known = metadata(session, &uris).await?;
    Ok(items
        .into_iter()
        .filter_map(|item| {
            let mut track = known.remove(&item.uri)?;
            track.added_at = item.added_at;
            Some(track)
        })
        .collect())
}

pub async fn track(session: &Session, track_id: &str) -> Result<Track> {
    let uri = identity::to_uri(track_id);
    metadata(session, std::slice::from_ref(&uri))
        .await?
        .remove(&uri)
        .context("track metadata is missing")
}

pub(crate) async fn metadata(session: &Session, uris: &[String]) -> Result<HashMap<String, Track>> {
    let (episodes, songs): (Vec<String>, Vec<String>) = uris
        .iter()
        .cloned()
        .partition(|uri| uri.starts_with("spotify:episode:"));
    let entities = extended(session, &songs, ExtensionKind::TRACK_V4)
        .await
        .context("cannot read track metadata")?;

    let mut tracks = HashMap::new();
    for entity in entities {
        let Ok(message) = TrackMessage::parse_from_bytes(&entity.extension_data.value) else {
            continue;
        };
        let track = track_from(&entity.entity_uri, &message);
        tracks.insert(entity.entity_uri, track);
    }
    let entities = extended(session, &episodes, ExtensionKind::EPISODE_V4)
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
        let track = episode_from(&entity.entity_uri, &message);
        tracks.insert(entity.entity_uri, track);
    }
    Ok(tracks)
}

pub(crate) async fn extended(
    session: &Session,
    uris: &[String],
    kind: ExtensionKind,
) -> Result<Vec<EntityExtensionData>> {
    let mut entities = Vec::with_capacity(uris.len());
    batched(session, uris, kind, BATCH, |batch| {
        entities.extend(batch);
        Ok(())
    })
    .await?;

    Ok(entities)
}

/// Reads extended metadata a batch at a time and hands each batch to `take` before asking
/// for the next one. A caller that turns the entities into its own models here never holds
/// more than one batch of protobufs, which is what keeps a thousand-release discography from
/// costing hundreds of megabytes.
pub(crate) async fn batched(
    session: &Session,
    uris: &[String],
    kind: ExtensionKind,
    batch: usize,
    mut take: impl FnMut(Vec<EntityExtensionData>) -> Result<()>,
) -> Result<()> {
    for uris in uris.chunks(batch.max(1)) {
        let request = BatchedEntityRequest {
            entity_request: uris
                .iter()
                .map(|uri| EntityRequest {
                    entity_uri: uri.clone(),
                    query: vec![ExtensionQuery {
                        extension_kind: EnumOrUnknown::new(kind),
                        ..Default::default()
                    }],
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        };
        let response = paced(|| session.spclient().get_extended_metadata(request.clone())).await?;
        take(
            response
                .extended_metadata
                .into_iter()
                .flat_map(|array| array.extension_data)
                .collect(),
        )?;
    }

    Ok(())
}

fn track_from(uri: &str, track: &TrackMessage) -> Track {
    let (artists, artist_refs) = artists_from(&track.artist);

    Track {
        id: uri.strip_prefix(TRACK_PREFIX).map(str::to_owned),
        playable: !track.file.is_empty() || !track.alternative.is_empty(),
        name: non_empty(track.name.as_deref())
            .unwrap_or(UNKNOWN)
            .to_owned(),
        artists,
        artist_refs,
        album: track
            .album
            .as_ref()
            .and_then(|album| non_empty(album.name.as_deref()))
            .unwrap_or_default()
            .to_owned(),
        album_id: track.album.as_ref().and_then(|album| base62(album.gid())),
        cover: track.album.as_ref().and_then(cover_url),
        duration: Duration::from_millis(track.duration.unwrap_or_default().max(0) as u64),
        added_at: None,
        added_by: None,
        playcount: None,
        popularity: track.popularity.unwrap_or_default().clamp(0, 100) as u32,
        explicit: track.explicit.unwrap_or_default(),
        track_number: track.number.unwrap_or_default().max(0) as u32,
        disc_number: track.disc_number.unwrap_or_default().max(0) as u32,
        tags: track.tags.clone(),
        languages: track.language_of_performance.clone(),
        credits: track
            .artist_with_role
            .iter()
            .filter_map(|credit| {
                let name = non_empty(credit.artist_name.as_deref())?.to_owned();
                let role = match credit.role.map(|role| role.value()) {
                    Some(1) => "Main artist",
                    Some(2) => "Featured artist",
                    Some(3) => "Remixer",
                    Some(4) => "Actor",
                    Some(5) => "Composer",
                    Some(6) => "Conductor",
                    Some(7) => "Orchestra",
                    _ => "Performer",
                };
                Some(Credit {
                    name,
                    role: role.to_owned(),
                    id: base62(credit.artist_gid()),
                })
            })
            .collect(),
    }
}

pub(crate) fn artists_from(artists: &[ArtistMessage]) -> (String, Vec<ArtistRef>) {
    let refs: Vec<_> = artists
        .iter()
        .filter_map(|artist| {
            let name = non_empty(artist.name.as_deref())?.to_owned();
            Some(ArtistRef {
                name,
                id: base62(artist.gid()),
            })
        })
        .collect();
    let names = match refs.is_empty() {
        true => UNKNOWN.to_owned(),
        false => refs
            .iter()
            .map(|artist| artist.name.as_str())
            .collect::<Vec<_>>()
            .join(", "),
    };
    (names, refs)
}

pub(crate) fn base62(gid: &[u8]) -> Option<String> {
    librespot_core::SpotifyId::from_raw(gid)
        .ok()
        .map(|id| id.to_base62())
}

fn non_empty(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.is_empty())
}

/// Maps episode audio onto the existing row, without creating artist or album routes for a show.
fn episode_from(uri: &str, episode: &EpisodeMessage) -> Track {
    let show = episode.show.as_ref();
    Track {
        id: identity::from_uri(uri),
        name: episode.name().to_owned(),
        playable: !episode.audio.is_empty(),
        artists: show.map(|show| show.name().to_owned()).unwrap_or_default(),
        cover: episode
            .cover_image
            .as_ref()
            .and_then(|group| image_url(group, true))
            .or_else(|| {
                show.and_then(|show| show.cover_image.as_ref())
                    .and_then(|group| image_url(group, true))
            }),
        duration: Duration::from_millis(episode.duration().max(0) as u64),
        explicit: episode.explicit(),
        ..Default::default()
    }
}

fn cover_url(album: &AlbumMessage) -> Option<String> {
    image_url(album.cover_group.as_ref()?, false)
}

/// Uses a thumbnail for albums, whose covers can be upgraded later. Episodes need the largest
/// listed image because they have no album from which fullscreen can load a better cover.
fn image_url(group: &ImageGroup, largest: bool) -> Option<String> {
    let image = group
        .image
        .iter()
        .filter(|image| image.has_file_id())
        .min_by_key(|image| match largest {
            true => -wire::image_width(image),
            false => wire::image_width(image),
        })?;

    wire::image_url(image.file_id())
}
