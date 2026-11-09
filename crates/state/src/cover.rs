use std::collections::HashMap;
use std::sync::Arc;

use gpui::{Context, Entity, Task};
use music::{CoverSize, MusicApi, Track};

use crate::{Io, Playback, Session, SessionEvent, join};

/// The artwork of an album or episode, with an original when the provider serves one.
#[derive(Clone)]
struct Art {
    large: String,
    full: Option<String>,
}

pub struct Cover {
    session: Entity<Session>,
    playback: Entity<Playback>,
    item: Option<String>,
    large: Option<String>,
    full: Option<String>,
    cache: HashMap<String, Art>,
    io: Io,
    task: Option<Task<()>>,
}

impl Cover {
    pub fn new(
        session: Entity<Session>,
        playback: Entity<Playback>,
        io: Io,
        cx: &mut Context<Self>,
    ) -> Self {
        cx.observe(&playback, |this, _, cx| this.follow(cx))
            .detach();
        cx.subscribe(&session, |this, _, event, cx| match event {
            SessionEvent::SignedOut => this.forget(cx),
            SessionEvent::SignedIn
            | SessionEvent::Reconnected
            | SessionEvent::WentOffline
            | SessionEvent::LocalChanged => {}
        })
        .detach();

        Self {
            session,
            playback,
            item: None,
            large: None,
            full: None,
            cache: HashMap::new(),
            io,
            task: None,
        }
    }

    pub fn large(&self) -> Option<&str> {
        self.large.as_deref()
    }

    /// The album or episode whose artwork follows this track. Episodes do not need album links.
    pub fn key(track: &Track) -> Option<&str> {
        match track.is_episode() {
            true => track.id.as_deref(),
            false => track.album_id.as_deref(),
        }
    }

    /// Returns the large artwork only when it belongs to `item`.
    pub fn large_for(&self, item: &str) -> Option<&str> {
        self.large
            .as_deref()
            .filter(|_| self.item.as_deref() == Some(item))
    }

    /// The full-resolution artwork for a cover that fills the screen, only when it belongs to
    /// `item` and the provider serves something bigger than the large one.
    pub fn full_for(&self, item: &str) -> Option<&str> {
        self.full
            .as_deref()
            .filter(|_| self.item.as_deref() == Some(item))
    }

    fn forget(&mut self, cx: &mut Context<Self>) {
        self.task = None;
        self.item = None;
        self.large = None;
        self.full = None;
        self.cache.clear();
        cx.notify();
    }

    /// Follows the shown track's album or episode. Reloading episode artwork also upgrades
    /// thumbnails kept in a queue saved by an older version of the app.
    fn follow(&mut self, cx: &mut Context<Self>) {
        let (item, thumbnail) = match self.playback.read(cx).shown_track(cx) {
            Some(track) => (Self::key(&track).map(str::to_owned), track.cover),
            None => (None, None),
        };
        if item == self.item {
            return;
        }
        self.task = None;
        self.item = item.clone();
        let cached = item.as_ref().and_then(|id| self.cache.get(id)).cloned();
        let client = item.as_deref().and_then(|id| self.client(id, cx));
        let derive = |size| client.as_ref()?.cover_at(thumbnail.as_deref()?, size);
        self.large = cached
            .as_ref()
            .map(|art| art.large.clone())
            .or_else(|| derive(CoverSize::Large));
        self.full = cached
            .and_then(|art| art.full)
            .or_else(|| derive(CoverSize::Full));
        cx.notify();

        let Some(id) = item else {
            return;
        };
        if self.large.is_some() {
            return;
        }
        self.load(id, cx);
    }

    fn load(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(client) = self.client(&id, cx) else {
            return;
        };

        let io = self.io.clone();
        let wanted = id.clone();
        self.task = Some(cx.spawn(async move |this, cx| {
            let found = join(io.spawn(async move {
                let large = match wanted.starts_with(music::EPISODE_PREFIX) {
                    true => client.track(&wanted).await?.cover,
                    false => client.album(&wanted).await?.album.cover_large,
                };
                let full = large
                    .as_deref()
                    .and_then(|cover| client.cover_at(cover, CoverSize::Full));
                anyhow::Ok(large.map(|large| Art { large, full }))
            }))
            .await;

            this.update(cx, |this, cx| {
                this.task = None;
                match found {
                    Ok(art) => {
                        let Some(art) = art else {
                            return;
                        };
                        this.cache.insert(id.clone(), art.clone());
                        if this.item.as_deref() == Some(id.as_str()) {
                            this.large = Some(art.large);
                            this.full = art.full.or(this.full.take());
                            cx.notify();
                        }
                    }
                    Err(error) => {
                        log::warn!("cover: cannot load {id}: {error:#}");
                        crate::noted(&error, cx);
                    }
                }
            })
            .ok();
        }));
    }

    /// The client that serves `id`: the local library for a local id, the signed-in
    /// provider otherwise.
    fn client(&self, id: &str, cx: &Context<Self>) -> Option<Arc<dyn MusicApi>> {
        let session = self.session.read(cx);
        match music::is_local_id(id) {
            true => session.local_client(),
            false => session.client(),
        }
    }
}
